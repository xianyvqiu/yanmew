use super::*;
use base64::Engine;
fn query_for(entity:&str)->Query {Query{entity:entity.into(),query:String::new(),page:1,library_id:None,persona_id:None,tag_ids:vec![]}}
fn library(c:&mut Connection,title:&str)->String {mutate(c,Mutation::SaveLibrary{id:None,revision:None,title:title.into(),description:"简介".into()}).unwrap()["id"].as_str().unwrap().into()}
fn tag(c:&mut Connection,name:&str)->String {mutate(c,Mutation::SaveTag{id:None,revision:None,name:name.into()}).unwrap()["id"].as_str().unwrap().into()}
#[test]
fn chat_stickers_search_pagination_and_all_tags_are_global_but_tools_are_bound(){
    let mut c=crate::db::test_database();
    let first=library(&mut c,"猫猫");let other=library(&mut c,"其他库");
    for n in 0..12{library(&mut c,&format!("库{n}"));}
    let page=query(&c,query_for("libraries")).unwrap();assert_eq!(page["total"],14);assert_eq!(page["items"].as_array().unwrap().len(),12);
    let mut q=query_for("libraries");q.page=2;assert_eq!(query(&c,q).unwrap()["items"].as_array().unwrap().len(),2);
    let happy=tag(&mut c,"开心");let cat=tag(&mut c,"猫猫");assert_eq!(tag(&mut c,"开心"),happy);
    c.execute("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(55,'另一个伙伴','设定','设定')",[]).unwrap();
    for persona in ["1","55"] {mutate(&mut c,Mutation::Bind{library_id:first.clone(),persona_id:persona.into(),bound:true}).unwrap();}
    let time=chrono::Utc::now().to_rfc3339();
    for (lib,title) in [(&first,"开心猫"),(&first,"开心狗"),(&other,"别人家的猫")] {c.execute("INSERT INTO chat_stickers(library_id,title,filename,image_file,media_type,created_at,updated_at) VALUES(?1,?2,'cat.gif','assets/chat_images/img-fixture.gif','image/gif',?3,?3)",params![lib,title,time]).unwrap();}
    for (sticker,tag) in [(1,&happy),(1,&cat),(2,&happy),(3,&cat)] {c.execute("INSERT INTO sticker_tag_links VALUES(?1,?2)",params![sticker,tag]).unwrap();}
    let mut q=query_for("stickers");q.persona_id=Some("1".into());q.tag_ids=vec![happy.clone(),cat.clone()];let result=query(&c,q).unwrap();assert_eq!(result["total"],1);assert_eq!(result["items"][0]["title"],"开心猫");
    let mut q=query_for("tags");q.persona_id=Some("55".into());assert_eq!(query(&c,q).unwrap()["total"],2);
    mutate(&mut c,Mutation::Bind{library_id:first.clone(),persona_id:"1".into(),bound:false}).unwrap();
    let mut q=query_for("stickers");q.persona_id=Some("1".into());assert_eq!(query(&c,q).unwrap()["total"],0);
    let mut q=query_for("stickers");q.persona_id=Some("55".into());assert_eq!(query(&c,q).unwrap()["total"],2);
    let mut q=query_for("libraries");q.query="猫猫".into();assert_eq!(query(&c,q).unwrap()["total"],1);
}
pub(crate) fn gif()->Vec<u8>{
    let mut bytes=Vec::new();{let mut e=image::codecs::gif::GifEncoder::new(&mut bytes);e.set_repeat(image::codecs::gif::Repeat::Infinite).unwrap();for color in [[255,0,0,255],[0,0,255,255]] {e.encode_frame(image::Frame::from_parts(image::RgbaImage::from_pixel(3,3,image::Rgba(color)),0,0,image::Delay::from_numer_denom_ms(100,1))).unwrap();}}bytes
}
#[test]
fn chat_stickers_import_keeps_gif_bytes_and_invalid_files_never_create_rows(){
    let folder=std::env::temp_dir().join(format!("yanmew-sticker-unit-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir(&folder).unwrap();let paths=AppPaths::portable(folder.clone());
    let mut c=crate::db::test_database();let lib=library(&mut c,"动图");let bytes=gif();
    let staged=crate::chat_images::stage(&paths,vec![crate::chat_images::Input{filename:"开心猫.gif".into(),data_url:format!("data:image/gif;base64,{}",base64::engine::general_purpose::STANDARD.encode(&bytes))}]).unwrap();
    import(&mut c,&paths,parse_id(&lib).unwrap(),staged).unwrap();
    let data=image(&c,&paths,1).unwrap();assert_eq!(data,format!("data:image/gif;base64,{}",base64::engine::general_purpose::STANDARD.encode(&bytes)));
    assert_eq!(query(&c,query_for("stickers")).unwrap()["items"][0]["title"],"开心猫");
    assert!(crate::chat_images::stage(&paths,vec![crate::chat_images::Input{filename:"坏.gif".into(),data_url:"data:image/gif;base64,YmFk".into()}]).is_err());
    assert!(mutate(&mut c,Mutation::SaveSticker{id:"1".into(),revision:9,title:"错误覆盖".into(),description:"".into(),tag_ids:vec![]}).is_err());
    let file:String=c.query_row("SELECT image_file FROM chat_stickers",[],|r|r.get(0)).unwrap();crate::chat_images::cleanup(&c,&paths).unwrap();assert!(crate::chat_images::managed_file(&paths,&file).is_ok());
    mutate(&mut c,Mutation::Delete{entity:"libraries".into(),id:lib,revision:0}).unwrap();crate::chat_images::cleanup(&c,&paths).unwrap();assert!(crate::chat_images::managed_file(&paths,&file).is_err());
    std::fs::remove_dir(folder.join("assets/chat_images")).unwrap();std::fs::remove_dir(folder.join("assets")).unwrap();std::fs::remove_dir(folder).unwrap();
}
#[test]
fn chat_stickers_v25_migration_preserves_messages_ids_and_high_water_mark(){
    let mut c=crate::db::test_database();let session=crate::sessions::create(&mut c,1).unwrap();
    c.execute("INSERT INTO messages(id,session_id,role,content,created_at) VALUES(201,?1,'assistant','旧消息','2026-10-28T00:00:00Z')",[&session.id]).unwrap();
    c.execute_batch("INSERT INTO message_images(id,message_id,filename,image_file,media_type,created_at) VALUES(700,201,'旧图.png','assets/chat_images/img-old.png','image/png','2026-10-28T00:00:00Z'),(999,201,'删除图.png','assets/chat_images/img-gone.png','image/png','2026-10-28T00:00:00Z');DELETE FROM message_images WHERE id=999;DROP TABLE persona_sticker_libraries;DROP TABLE sticker_tag_links;DROP TABLE chat_stickers;DROP TABLE sticker_libraries;DROP TABLE sticker_tags;ALTER TABLE message_images DROP COLUMN source_kind;PRAGMA user_version=25;").unwrap();
    crate::db::initialize(&mut c).unwrap();assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),26);
    assert_eq!(crate::chat_images::list(&c,201).unwrap()[0].id,"700");assert_eq!(crate::chat_images::list(&c,201).unwrap()[0].source_kind,"image");
    c.execute("INSERT INTO message_images(message_id,filename,image_file,media_type,created_at,source_kind) VALUES(201,'动图.gif','assets/chat_images/img-new.gif','image/gif','2026-10-28T00:00:00Z','sticker')",[]).unwrap();assert!(c.last_insert_rowid()>999);
    assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());crate::db::initialize(&mut c).unwrap();
}
