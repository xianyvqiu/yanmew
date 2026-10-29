//! 全局聊天表情包库。绑定只授予使用权，不搬动资料；已发送图片保存独立消息副本。
use crate::{error::{parse_id, AppError, AppResult}, platform::contracts::AppPaths};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Query {
    pub entity: String,
    #[serde(default)] pub query: String,
    #[serde(default="first_page")] pub page: u32,
    #[serde(default)] pub library_id: Option<String>,
    #[serde(default)] pub persona_id: Option<String>,
    #[serde(default)] pub tag_ids: Vec<String>,
}
fn first_page()->u32 {1}
fn ids(values:&[String])->AppResult<Vec<i64>> {values.iter().map(|s|parse_id(s)).collect()}
fn title(value:&str)->AppResult<&str> {let value=value.trim();if value.is_empty(){Err(AppError::validation("title","请填写名称"))}else{Ok(value)}}
fn exists(c:&Connection,table:&str,id:i64)->AppResult<()> {
    if c.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),[id],|r|r.get::<_,bool>(0))? {Ok(())}else{Err(AppError::not_found())}
}
fn expected(c:&Connection,table:&str,id:i64,revision:i64)->AppResult<()> {
    let found:Option<i64>=c.query_row(&format!("SELECT revision FROM {table} WHERE id=?1"),[id],|r|r.get(0)).optional()?;
    match found {None=>Err(AppError::not_found()),Some(n) if n!=revision=>Err(AppError::new("CONFLICT","资料已更新，请重新打开后再修改")),_=>Ok(())}
}
pub fn query(c:&Connection,q:Query)->AppResult<Value> {
    let library=q.library_id.as_deref().map(parse_id).transpose()?;
    let persona=q.persona_id.as_deref().map(parse_id).transpose()?;
    let tags=serde_json::to_string(&ids(&q.tag_ids)?).map_err(|_|AppError::storage())?;
    let (table,columns,filter)=match q.entity.as_str() {
        "libraries"=>("sticker_libraries l","l.id,l.title,l.description,l.revision,l.created_at,l.updated_at",
            "instr(lower(l.title||' '||l.description),lower(?1))>0 AND (?2 IS NULL OR EXISTS(SELECT 1 FROM persona_sticker_libraries b WHERE b.library_id=l.id AND b.persona_id=?2)) AND (?3 IS NULL OR l.id=?3) AND ?4 IS NOT NULL"),
        "tags"=>("sticker_tags t","t.id,t.name,t.revision,t.created_at,t.updated_at",
            "instr(lower(t.name),lower(?1))>0 AND (?2 IS NULL OR EXISTS(SELECT 1 FROM sticker_tag_links x JOIN chat_stickers s ON s.id=x.sticker_id JOIN persona_sticker_libraries b ON b.library_id=s.library_id WHERE x.tag_id=t.id AND b.persona_id=?2)) AND (?3 IS NULL OR EXISTS(SELECT 1 FROM sticker_tag_links x JOIN chat_stickers s ON s.id=x.sticker_id WHERE x.tag_id=t.id AND s.library_id=?3)) AND ?4 IS NOT NULL"),
        "stickers"=>("chat_stickers s","s.id,s.title,s.description,s.revision,s.created_at,s.updated_at,s.library_id,s.filename,s.media_type",
            "(instr(lower(s.title||' '||s.description),lower(?1))>0 OR EXISTS(SELECT 1 FROM sticker_tag_links x JOIN sticker_tags t ON t.id=x.tag_id WHERE x.sticker_id=s.id AND instr(lower(t.name),lower(?1))>0)) AND (?2 IS NULL OR EXISTS(SELECT 1 FROM persona_sticker_libraries b WHERE b.library_id=s.library_id AND b.persona_id=?2)) AND (?3 IS NULL OR s.library_id=?3) AND NOT EXISTS(SELECT 1 FROM json_each(?4) j WHERE NOT EXISTS(SELECT 1 FROM sticker_tag_links x WHERE x.sticker_id=s.id AND x.tag_id=j.value))"),
        _=>return Err(AppError::validation("entity","无效的表情包查询")),
    };
    let total:i64=c.query_row(&format!("SELECT count(*) FROM {table} WHERE {filter}"),params![q.query.trim(),persona,library,tags],|r|r.get(0))?;
    let pages=((total+11)/12).max(1);let page=i64::from(q.page.max(1)).min(pages);
    let mut statement=c.prepare(&format!("SELECT {columns} FROM {table} WHERE {filter} ORDER BY 1 DESC LIMIT 12 OFFSET ?5"))?;
    let rows=statement.query_map(params![q.query.trim(),persona,library,tags,(page-1)*12],|r|{
        if q.entity=="tags" {return Ok(json!({"id":r.get::<_,i64>(0)?.to_string(),"name":r.get::<_,String>(1)?,"revision":r.get::<_,i64>(2)?,"createdAt":r.get::<_,String>(3)?,"updatedAt":r.get::<_,String>(4)?}));}
        let mut value=json!({"id":r.get::<_,i64>(0)?.to_string(),"title":r.get::<_,String>(1)?,"description":r.get::<_,String>(2)?,"revision":r.get::<_,i64>(3)?,"createdAt":r.get::<_,String>(4)?,"updatedAt":r.get::<_,String>(5)?});
        if q.entity=="stickers" {value["libraryId"]=json!(r.get::<_,i64>(6)?.to_string());value["filename"]=json!(r.get::<_,String>(7)?);value["mediaType"]=json!(r.get::<_,String>(8)?);}
        Ok(value)
    })?.collect::<Result<Vec<_>,_>>()?;
    let mut items=Vec::new();
    for mut row in rows {
        let id=parse_id(row["id"].as_str().ok_or_else(AppError::storage)?)?;
        if q.entity=="libraries" {
            row["count"]=json!(c.query_row("SELECT count(*) FROM chat_stickers WHERE library_id=?1",[id],|r|r.get::<_,i64>(0))?);
            let mut s=c.prepare("SELECT persona_id FROM persona_sticker_libraries WHERE library_id=?1 ORDER BY persona_id")?;
            row["personaIds"]=json!(s.query_map([id],|r|Ok(r.get::<_,i64>(0)?.to_string()))?.collect::<Result<Vec<_>,_>>()?);
        } else if q.entity=="stickers" {
            let mut s=c.prepare("SELECT t.id,t.name FROM sticker_tags t JOIN sticker_tag_links x ON x.tag_id=t.id WHERE x.sticker_id=?1 ORDER BY t.name,t.id")?;
            row["tags"]=json!(s.query_map([id],|r|Ok(json!({"id":r.get::<_,i64>(0)?.to_string(),"name":r.get::<_,String>(1)?})))?.collect::<Result<Vec<_>,_>>()?);
        }
        items.push(row);
    }
    Ok(json!({"items":items,"page":page,"pages":pages,"total":total}))
}

#[derive(Deserialize)]
#[serde(tag="op",rename_all="camelCase",deny_unknown_fields)]
pub enum Mutation {
    SaveLibrary {id:Option<String>,revision:Option<i64>,title:String,description:String},
    SaveTag {id:Option<String>,revision:Option<i64>,name:String},
    SaveSticker {id:String,revision:i64,title:String,description:String,#[serde(rename="tagIds")]tag_ids:Vec<String>},
    Delete {entity:String,id:String,revision:i64},
    Bind {#[serde(rename="libraryId")]library_id:String,#[serde(rename="personaId")]persona_id:String,bound:bool},
}
pub fn mutate(c:&mut Connection,input:Mutation)->AppResult<Value> {
    let now=chrono::Utc::now().to_rfc3339();let tx=c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut result=json!({"saved":true});
    match input {
        Mutation::SaveLibrary{id,revision,title:value,description}=>{
            let value=title(&value)?;
            let id=if let Some(id)=id {let id=parse_id(&id)?;expected(&tx,"sticker_libraries",id,revision.ok_or_else(||AppError::validation("revision","请重新打开资料"))?)?;tx.execute("UPDATE sticker_libraries SET title=?1,description=?2,revision=revision+1,updated_at=?3 WHERE id=?4",params![value,description,now,id])?;id}
            else{tx.execute("INSERT INTO sticker_libraries(title,description,created_at,updated_at) VALUES(?1,?2,?3,?3)",params![value,description,now])?;tx.last_insert_rowid()};result["id"]=json!(id.to_string());
        },
        Mutation::SaveTag{id,revision,name}=>{
            let name=title(&name)?;
            let duplicate:Option<i64>=tx.query_row("SELECT id FROM sticker_tags WHERE name=?1",[name],|r|r.get(0)).optional()?;
            if let Some(id)=id {let id=parse_id(&id)?;expected(&tx,"sticker_tags",id,revision.ok_or_else(||AppError::validation("revision","请重新打开标签"))?)?;if duplicate.is_some_and(|other|other!=id){return Err(AppError::validation("name","已有同名标签，请选择已有标签"));}tx.execute("UPDATE sticker_tags SET name=?1,revision=revision+1,updated_at=?2 WHERE id=?3",params![name,now,id])?;result["id"]=json!(id.to_string());}
            else if let Some(id)=duplicate{result["id"]=json!(id.to_string());}
            else{tx.execute("INSERT INTO sticker_tags(name,created_at,updated_at) VALUES(?1,?2,?2)",params![name,now])?;result["id"]=json!(tx.last_insert_rowid().to_string());}
        },
        Mutation::SaveSticker{id,revision,title:value,description,tag_ids}=>{
            let id=parse_id(&id)?;expected(&tx,"chat_stickers",id,revision)?;let value=title(&value)?;
            let tags=ids(&tag_ids)?;for tag in &tags {exists(&tx,"sticker_tags",*tag)?;}
            tx.execute("UPDATE chat_stickers SET title=?1,description=?2,revision=revision+1,updated_at=?3 WHERE id=?4",params![value,description,now,id])?;
            tx.execute("DELETE FROM sticker_tag_links WHERE sticker_id=?1",[id])?;
            for tag in tags {tx.execute("INSERT OR IGNORE INTO sticker_tag_links VALUES(?1,?2)",params![id,tag])?;}
        },
        Mutation::Delete{entity,id,revision}=>{
            let table=match entity.as_str(){"libraries"=>"sticker_libraries","tags"=>"sticker_tags","stickers"=>"chat_stickers",_=>return Err(AppError::validation("entity","无效的删除目标"))};let id=parse_id(&id)?;expected(&tx,table,id,revision)?;
            tx.execute(&format!("DELETE FROM {table} WHERE id=?1"),[id])?;
        },
        Mutation::Bind{library_id,persona_id,bound}=>{
            let library=parse_id(&library_id)?;let persona=parse_id(&persona_id)?;exists(&tx,"sticker_libraries",library)?;exists(&tx,"personas",persona)?;
            if bound{tx.execute("INSERT OR IGNORE INTO persona_sticker_libraries VALUES(?1,?2,?3)",params![persona,library,now])?;}else{tx.execute("DELETE FROM persona_sticker_libraries WHERE persona_id=?1 AND library_id=?2",params![persona,library])?;}
        },
    }
    tx.commit()?;Ok(result)
}
pub fn import(c:&mut Connection,paths:&AppPaths,library:i64,mut staged:crate::chat_images::Staged)->AppResult<()> {
    let image=staged.images.first().ok_or_else(||AppError::validation("file","请选择图片"))?;
    let now=chrono::Utc::now().to_rfc3339();let tx=c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    exists(&tx,"sticker_libraries",library)?;
    let name=std::path::Path::new(&image.filename).file_stem().and_then(|s|s.to_str()).filter(|s|!s.trim().is_empty()).unwrap_or("未命名表情包");
    tx.execute("INSERT INTO chat_stickers(library_id,title,filename,image_file,media_type,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)",params![library,name,image.filename,image.image_file,image.media_type,now])?;
    let _=paths;crate::chat_images::publish_staged(&staged.images)?;tx.commit()?;staged.committed=true;Ok(())
}
pub fn image(c:&Connection,paths:&AppPaths,id:i64)->AppResult<String> {
    let (name,media):(String,String)=c.query_row("SELECT image_file,media_type FROM chat_stickers WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?.ok_or_else(AppError::not_found)?;
    use base64::Engine;let bytes=std::fs::read(crate::chat_images::managed_file(paths,&name)?).map_err(|_|AppError::storage())?;
    Ok(format!("data:{media};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[cfg(test)]
mod tests;
