//! 仅替换仍使用已知旧内置抛接图的记录；不重建动作或覆盖自定义图片。
use super::{AppPaths,AppResult,Connection,PRESETS};
use crate::error::AppError;
use rusqlite::{params,TransactionBehavior};
use sha2::{Digest,Sha256};
use std::{collections::BTreeMap,fs,path::Path};
const KEY:&str=super::TOSS_PROPORTIONS_SEED_KEY;
pub(super) fn install(c:&mut Connection,paths:&AppPaths)->AppResult<()> {
    if seeded(c)?{return Ok(())}
    let Ok(resources)=paths.persona_resources()else{return Ok(())};
    let hashes:BTreeMap<String,String>=serde_json::from_str(include_str!("../../../resources/personas/toss-proportion-replacements.json")).map_err(|_|AppError::storage())?;
    install_from(c,paths,&resources,&hashes)
}
fn seeded(c:&Connection)->AppResult<bool>{Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key=?1)",[KEY],|r|r.get(0))?)}
fn install_from(c:&mut Connection,paths:&AppPaths,resources:&Path,hashes:&BTreeMap<String,String>)->AppResult<()> {
    if seeded(c)?{return Ok(())}
    let candidates={let mut q=c.prepare("SELECT p.id,p.initial_prompt,a.image_file FROM personas p JOIN pet_artworks a ON a.persona_id=p.id AND a.kind='base'")?;
        let rows=q.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?)))?.collect::<Result<Vec<_>,_>>()?;rows};
    let mut prepared=Vec::new();
    for preset in PRESETS {
        let matching=candidates.iter().filter(|(_,prompt,_)|preset.matches_prompt(prompt)).collect::<Vec<_>>();
        if matching.is_empty()||!hashes.keys().any(|key|key.starts_with(&format!("{}/",preset.key))){continue}
        let Ok(base)=fs::read(resources.join(preset.key).join("base.png"))else{return Ok(())};let base=crate::pet_creation::normalize(&base)?;
        for (persona,_,file) in matching {
            let Some(file)=file else{continue};let Ok(path)=crate::accessories::images::managed_file(paths,file)else{continue};
            if !fs::read(path).is_ok_and(|bytes|bytes==base){continue}
            for category in ["toss_up","toss_down"] {
                let Some(expected)=hashes.get(&format!("{}/{category}",preset.key))else{continue};
                let rows={let mut q=c.prepare("SELECT id,image_file,revision FROM pet_artworks WHERE persona_id=?1 AND kind='expression' AND category=?2")?;
                    let rows=q.query_map(params![persona,category],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,i64>(2)?)))?.collect::<Result<Vec<_>,_>>()?;rows};
                for (id,file,revision) in rows {
                    let Some(file)=file else{continue};let Ok(path)=crate::accessories::images::managed_file(paths,&file)else{continue};let Ok(bytes)=fs::read(path)else{continue};
                    if format!("{:x}",Sha256::digest(bytes))!=*expected{continue}
                    let Ok(next)=fs::read(resources.join(preset.key).join(format!("{category}.png")))else{return Ok(())};
                    prepared.push((id,*persona,file,revision,crate::pet_creation::normalize(&next)?));
                }
            }
        }
    }
    let mut published=Vec::new();
    let result=(||->AppResult<()> {
        let tx=c.transaction_with_behavior(TransactionBehavior::Immediate)?;if seeded(&tx)?{return Ok(())}
        let now=chrono::Utc::now().to_rfc3339();
        for (id,persona,old,revision,bytes) in prepared {
            let file=crate::accessories::images::publish(paths,&bytes)?;published.push(file.clone());
            if tx.execute("UPDATE pet_artworks SET image_file=?1,revision=revision+1,updated_at=?2 WHERE id=?3 AND image_file=?4 AND revision=?5",params![file,now,id,old,revision])?>0 {
                tx.execute("UPDATE personas SET pet_image_revision=pet_image_revision+1 WHERE id=?1",[persona])?;
            }
        }
        tx.execute("INSERT INTO settings(key,value) VALUES(?1,'1')",[KEY])?;tx.commit()?;Ok(())
    })();
    if result.is_err(){for file in published {if let Ok(path)=crate::accessories::images::managed_file(paths,&file){let _=fs::remove_file(path);}}}
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accessories::images;
    struct Fixture {root:std::path::PathBuf,paths:AppPaths,c:Connection,hashes:BTreeMap<String,String>,old:String,custom:String}
    fn png(color:[u8;4])->Vec<u8>{let mut image=image::RgbaImage::new(16,16);for y in 3..13{for x in 3..13{image.put_pixel(x,y,image::Rgba(color));}}let mut out=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image).write_to(&mut out,image::ImageFormat::Png).unwrap();out.into_inner()}
    impl Fixture {
        fn new()->Self {
            let root=Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(".test-temp").join(format!("145-toss-refresh-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            let paths=AppPaths::portable(root.join("app"));fs::create_dir_all(&paths.data_root).unwrap();let resources=root.join("resources/gpt");fs::create_dir_all(&resources).unwrap();
            let base=png([100,80,200,255]);fs::write(resources.join("base.png"),&base).unwrap();fs::write(resources.join("toss_up.png"),png([80,180,200,255])).unwrap();
            let old_bytes=crate::pet_creation::normalize(&png([200,80,80,255])).unwrap();let old=images::publish(&paths,&old_bytes).unwrap();let custom=images::publish(&paths,&crate::pet_creation::normalize(&png([40,200,90,255])).unwrap()).unwrap();
            let base=images::publish(&paths,&crate::pet_creation::normalize(&base).unwrap()).unwrap();let c=crate::db::test_database();
            c.execute("UPDATE personas SET initial_prompt=?1 WHERE id=1",[PRESETS[0].prompt]).unwrap();
            c.execute("DELETE FROM pet_artworks WHERE persona_id=1",[]).unwrap();
            c.execute("INSERT INTO pet_artworks(persona_id,kind,name,category,image_file,created_at,updated_at) VALUES(1,'base','初始图','normal',?1,'a','a')",[base]).unwrap();
            c.execute("INSERT INTO pet_artworks(persona_id,kind,name,prompt,description,category,enabled,image_file,revision,created_at,updated_at) VALUES(1,'expression','我的上抛名称','我的提示','我的说明','toss_up',0,?1,5,'a','b'),(1,'expression','自定义下落','','','toss_down',1,?2,9,'a','b')",params![old,custom]).unwrap();
            let hashes=[("gpt/toss_up".into(),format!("{:x}",Sha256::digest(&old_bytes))),("gpt/toss_down".into(),format!("{:x}",Sha256::digest(old_bytes)))].into_iter().collect();
            Self{root,paths,c,hashes,old,custom}
        }
        fn run(&mut self)->AppResult<()>{install_from(&mut self.c,&self.paths,&self.root.join("resources"),&self.hashes)}
    }
    impl Drop for Fixture {fn drop(&mut self){let expected=Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join(".test-temp");assert_eq!(self.root.parent(),Some(expected.as_path()));let _=fs::remove_dir_all(&self.root);}}
    #[test]
    fn replaces_only_official_pixels_preserves_metadata_and_does_not_recreate_deleted_actions(){
        let mut f=Fixture::new();f.run().unwrap();
        let(name,prompt,description,enabled,file,revision):(String,String,String,bool,String,i64)=f.c.query_row("SELECT name,prompt,description,enabled,image_file,revision FROM pet_artworks WHERE category='toss_up'",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
        assert_eq!((name.as_str(),prompt.as_str(),description.as_str(),enabled,revision),("我的上抛名称","我的提示","我的说明",false,6));assert_ne!(file,f.old);assert_eq!(f.c.query_row("SELECT image_file FROM pet_artworks WHERE category='toss_down'",[],|r|r.get::<_,String>(0)).unwrap(),f.custom);
        f.run().unwrap();assert_eq!(f.c.query_row("SELECT revision FROM pet_artworks WHERE category='toss_up'",[],|r|r.get::<_,i64>(0)).unwrap(),6);
        f.c.execute("DELETE FROM pet_artworks WHERE category='toss_up'",[]).unwrap();f.c.execute("DELETE FROM settings WHERE key=?1",[KEY]).unwrap();f.run().unwrap();assert_eq!(f.c.query_row("SELECT count(*) FROM pet_artworks WHERE category='toss_up'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    #[test]
    fn custom_base_and_failed_update_leave_existing_artwork_untouched(){
        let mut f=Fixture::new();f.c.execute("CREATE TRIGGER reject_pose BEFORE UPDATE ON pet_artworks BEGIN SELECT RAISE(ABORT,'test');END;",[]).unwrap();assert!(f.run().is_err());assert!(!seeded(&f.c).unwrap());assert_eq!(f.c.query_row("SELECT image_file FROM pet_artworks WHERE category='toss_up'",[],|r|r.get::<_,String>(0)).unwrap(),f.old);
        f.c.execute_batch("DROP TRIGGER reject_pose").unwrap();f.c.execute("UPDATE pet_artworks SET image_file=?1 WHERE kind='base'",[&f.custom]).unwrap();f.run().unwrap();assert_eq!(f.c.query_row("SELECT image_file FROM pet_artworks WHERE category='toss_up'",[],|r|r.get::<_,String>(0)).unwrap(),f.old);
    }
}
