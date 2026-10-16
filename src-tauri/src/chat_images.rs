//! 消息图片：受管文件、消息外键及可追溯理解文本。不接受任意本机路径。
use crate::{error::{parse_id,AppError,AppResult},platform::contracts::AppPaths};
use base64::Engine;
use rusqlite::{params,Connection};
use serde::{Deserialize,Serialize};
use std::{fs,path::PathBuf,io::Write,sync::atomic::{AtomicU64,Ordering}};
static NEXT:AtomicU64=AtomicU64::new(1);
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Input {pub filename:String,pub data_url:String}
#[derive(Clone,Debug,PartialEq,Eq,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Image {pub id:String,pub filename:String,pub image_file:String,pub media_type:String,pub caption:Option<String>}
pub struct NewImage {pub filename:String,pub image_file:String,pub media_type:String,pub pending_file:PathBuf}
pub struct Staged {pub images:Vec<NewImage>,files:Vec<PathBuf>,pub committed:bool}
impl Drop for Staged {fn drop(&mut self){if !self.committed{for file in &self.files {let _=fs::remove_file(file);}}}}
fn missing()->AppError{AppError::new("CHAT_IMAGE_MISSING","聊天图片已丢失，请保留记录并重新提供图片")}
fn folder(paths:&AppPaths)->AppResult<PathBuf> {
    let root=paths.data_root.canonicalize().map_err(|_|AppError::storage())?;
    let folder=paths.data_root.join("assets/chat_images");fs::create_dir_all(&folder).map_err(|_|AppError::storage())?;
    let resolved=folder.canonicalize().map_err(|_|AppError::storage())?;
    if !resolved.starts_with(root){return Err(AppError::new("CHAT_IMAGE_INVALID","图片目录不是当前应用的受管目录"));}Ok(resolved)
}
pub fn managed_file(paths:&AppPaths,name:&str)->AppResult<PathBuf> {
    let file=name.strip_prefix("assets/chat_images/").filter(|n|!n.is_empty()&&!n.contains(['/', '\\',':','\0'])&&n.starts_with("img-")).ok_or_else(missing)?;
    let folder=folder(paths)?;let path=folder.join(file).canonicalize().map_err(|_|missing())?;
    if path.parent()!=Some(folder.as_path())||!path.is_file(){return Err(missing());}Ok(path)
}
pub fn stage(paths:&AppPaths,inputs:Vec<Input>)->AppResult<Staged> {
    let mut staged=Staged{images:vec![],files:vec![],committed:false};
    if inputs.is_empty(){return Ok(staged)}
    let folder=folder(paths)?;
    for input in inputs {
        let (_,encoded)=input.data_url.split_once(",").filter(|(p,_)|p.starts_with("data:image/")&&p.ends_with(";base64")).ok_or_else(||AppError::validation("images","请选择有效图片文件"))?;
        let bytes=base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_|AppError::validation("images","图片数据无法读取"))?;
        let format=image::guess_format(&bytes).map_err(|_|AppError::validation("images","图片格式无法识别"))?;
        let (media,extension)=match format {image::ImageFormat::Png=>("image/png","png"),image::ImageFormat::Jpeg=>("image/jpeg","jpg"),image::ImageFormat::WebP=>("image/webp","webp"),_=>return Err(AppError::validation("images","当前支持PNG、JPEG和WebP图片"))};
        image::load_from_memory_with_format(&bytes,format).map_err(|_|AppError::validation("images","图片损坏，无法解码"))?;
        let unique=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|AppError::storage())?.as_nanos();
        let name=format!("img-{unique}-{}.{}",NEXT.fetch_add(1,Ordering::Relaxed),extension);
        let path=folder.join(format!("{name}.pending"));let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|_|AppError::storage())?;
        staged.files.push(path.clone());staged.files.push(folder.join(&name));file.write_all(&bytes).and_then(|_|file.sync_all()).map_err(|_|AppError::storage())?;
        staged.images.push(NewImage{filename:input.filename.trim().to_string(),image_file:format!("assets/chat_images/{name}"),media_type:media.into(),pending_file:path});
    }Ok(staged)
}
pub fn publish(paths:&AppPaths,bytes:&[u8],filename:&str)->AppResult<String> {
    let mut staged=stage(paths,vec![Input{filename:filename.into(),data_url:format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}])?;
    publish_staged(&staged.images)?;
    let path=staged.images.first().ok_or_else(AppError::storage)?.image_file.clone();staged.committed=true;Ok(path)
}
pub fn publish_staged(images:&[NewImage])->AppResult<()> {
    for image in images {
        let folder=image.pending_file.parent().ok_or_else(AppError::storage)?;
        let name=image.image_file.strip_prefix("assets/chat_images/").ok_or_else(AppError::storage)?;
        fs::rename(&image.pending_file,folder.join(name)).map_err(|_|AppError::storage())?;
    }Ok(())
}
pub fn insert(c:&Connection,message:i64,images:&[NewImage],now:&str)->AppResult<()> {
    for image in images {c.execute("INSERT INTO message_images(message_id,filename,image_file,media_type,created_at) VALUES(?1,?2,?3,?4,?5)",params![message,image.filename,image.image_file,image.media_type,now])?;}Ok(())
}
pub fn list(c:&Connection,message:i64)->AppResult<Vec<Image>> {
    let mut q=c.prepare("SELECT id,filename,image_file,media_type,caption FROM message_images WHERE message_id=?1 ORDER BY id")?;
    let images=q.query_map([message],|r|Ok(Image{id:r.get::<_,i64>(0)?.to_string(),filename:r.get(1)?,image_file:r.get(2)?,media_type:r.get(3)?,caption:r.get(4)?}))?.collect::<Result<Vec<_>,_>>()?;Ok(images)
}
pub fn hydrate(c:&Connection,messages:&mut[crate::models::Message])->AppResult<()> {
    for message in messages {message.images=list(c,parse_id(&message.id)?)?;}Ok(())
}
pub fn data_url(paths:&AppPaths,image:&Image)->AppResult<String> {
    let bytes=fs::read(managed_file(paths,&image.image_file)?).map_err(|_|missing())?;
    Ok(format!("data:{};base64,{}",image.media_type,base64::engine::general_purpose::STANDARD.encode(bytes)))
}
pub fn caption(c:&Connection,message:i64,text:&str)->AppResult<()> {
    c.execute("UPDATE message_images SET caption=?1 WHERE message_id=?2",params![text,message])?;Ok(())
}
pub fn memory_text(c:&Connection,message:i64,content:&str)->AppResult<String> {
    let mut q=c.prepare("SELECT DISTINCT caption FROM message_images WHERE message_id=?1 AND caption IS NOT NULL ORDER BY id")?;
    let captions=q.query_map([message],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
    if captions.is_empty(){
        let count:i64=c.query_row("SELECT count(*) FROM message_images WHERE message_id=?1",[message],|r|r.get(0))?;
        if count>0{Ok(format!("{content}\n[用户在这条消息附了{count}张图片；图片内容参考本轮AI回复，不臆测尚未看清的事实]"))}else{Ok(content.into())}
    }else{Ok(format!("{content}\n图片理解资料（模型生成，可能不准确）：\n{}",captions.join("\n")))}
}
pub fn cleanup(c:&Connection,paths:&AppPaths)->AppResult<()> {
    let dir=paths.data_root.join("assets/chat_images");if !dir.try_exists().map_err(|_|AppError::storage())?{return Ok(())}
    let folder=folder(paths)?;
    for entry in fs::read_dir(&folder).map_err(|_|AppError::storage())? {
        let entry=entry.map_err(|_|AppError::storage())?;let name=entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("img-")||name.ends_with(".pending")||!entry.file_type().map_err(|_|AppError::storage())?.is_file(){continue;}
        let used=c.query_row("SELECT EXISTS(SELECT 1 FROM message_images WHERE image_file=?1)",[format!("assets/chat_images/{name}")],|r|r.get::<_,bool>(0))?;
        if !used{fs::remove_file(entry.path()).map_err(|_|AppError::storage())?;}
    }Ok(())
}
pub fn cleanup_unused(runtime:&crate::runtime::Runtime) {
    let _=runtime.read(|store,_|cleanup(&store.connection,runtime.paths.as_ref().map_err(Clone::clone)?));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_round_is_persistent_and_cleanup_keeps_live_references() {
        let folder=std::env::temp_dir().join(format!("yanmew-chat-image-test-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&folder).unwrap();let paths=AppPaths::portable(folder.clone());
        let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(2,2).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        let mut staged=stage(&paths,vec![Input{filename:"小猫.png".into(),data_url:format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes.get_ref()))}]).unwrap();
        let mut c=crate::db::test_database();let session=crate::sessions::create(&mut c,1).unwrap();
        let round=crate::sessions::start_round_with_images(&mut c,1,parse_id(&session.id).unwrap(),"",&staged.images).unwrap();staged.committed=true;
        assert_eq!(round.user_message.images.len(),1);let name=round.user_message.images[0].image_file.clone();
        cleanup(&c,&paths).unwrap();assert!(managed_file(&paths,&name).is_ok());
        let restored=crate::sessions::round_snapshot(&c,1,parse_id(&session.id).unwrap(),parse_id(&round.round.id).unwrap()).unwrap();assert_eq!(restored.user_message.images[0].filename,"小猫.png");
        c.execute("DELETE FROM messages WHERE id=?1",[parse_id(&round.user_message.id).unwrap()]).unwrap();cleanup(&c,&paths).unwrap();assert!(managed_file(&paths,&name).is_err());
        // 只删除本测试明确创建的单文件夹，路径由固定前缀独占生成。
        fs::remove_dir(folder.join("assets/chat_images")).unwrap();fs::remove_dir(folder.join("assets")).unwrap();fs::remove_dir(folder).unwrap();
    }
}
