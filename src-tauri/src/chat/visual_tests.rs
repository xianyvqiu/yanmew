use super::*;
use crate::ai::provider::ToolCall;
use crate::ai::tests::{client, event, finish_event, text_event, Response, Server};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
fn call(id: &str, name: &str, args: Value) -> String {
    event(
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":null}]}),
    ) + &finish_event("tool_calls")
}
#[test]
fn image_chat_uses_function_call_with_persona_scoped_picture_and_keeps_text_library_untouched() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::sse(&call(
                "emotion",
                "set_pet_expression",
                json!({"artworkId":"3"}),
            )),
            Response::sse(&(text_event("好开心！") + &finish_event("stop"))),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        runtime.read(|s,_|{s.connection.execute_batch("INSERT INTO pet_artworks(persona_id,kind,name,category,image_file,created_at,updated_at) VALUES(1,'base','初始形象','normal','base.png','a','a'),(1,'expression','自然交谈','chat','chat.png','a','a'),(1,'expression','开心','chat','happy.png','a','a');UPDATE personas SET pet_image_enabled=1 WHERE id=1;")?;Ok(())}).unwrap();
        let (prepared, _) = submit(&runtime, 1, 1, 0, "我今天中了一个小奖！".into()).unwrap();
        let saved = run_persisted(&client(), prepared, |_| {}).await.unwrap();
        let message = saved.assistant_message.unwrap();
        assert_eq!(message.content, "好开心！");
        assert_eq!(
            message.expression.as_deref(),
            Some("@pet-image:3")
        );
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        let observation=requests[1]["messages"].as_array().unwrap().iter().find(|m|m["role"]=="tool").unwrap();
        let observation:Value=serde_json::from_str(observation["content"].as_str().unwrap()).unwrap();
        assert_eq!(observation["result"]["applied"], true);
        assert!(observation["result"].get("expression").is_none());
        assert!(!observation.to_string().contains("@pet-image:"));
        assert!(requests[0]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["function"]["name"] == "set_pet_expression"));
        drop(requests);
        server.finish();
        runtime
            .read(|s, _| {
                assert!(s
                    .connection
                    .query_row("SELECT expressions FROM personas WHERE id=1", [], |r| {
                        r.get::<_, Option<String>>(0)
                    })?
                    .is_none());
                Ok(())
            })
            .unwrap();
    });
}
#[test]
fn picture_selection_rejects_other_persona_and_disabled_images() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    runtime.read(|s,_|{s.connection.execute_batch("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(2,'其他角色','其他','其他');INSERT INTO pet_artworks(id,persona_id,kind,name,category,image_file,created_at,updated_at) VALUES(1,1,'base','初始','normal','base.png','a','a'),(2,2,'base','别人','normal','other.png','a','a'),(3,1,'expression','开心','chat','happy.png','a','a');UPDATE pet_artworks SET enabled=0 WHERE id=3;UPDATE personas SET pet_image_enabled=1 WHERE id=1;")?;Ok(())}).unwrap();
    for id in ["2", "3", "999"] {
        assert!(visual_tools::local(
            &runtime,
            1,
            1,
            0,
            &ToolCall {
                index: 0,
                id: format!("id-{id}"),
                name: "set_pet_expression".into(),
                arguments: json!({"artworkId":id})
            }
        )
        .is_err());
    }
    runtime
        .read(|s, _| {
            s.connection
                .execute("UPDATE personas SET pet_image_enabled=0 WHERE id=1", [])?;
            Ok(())
        })
        .unwrap();
    let value = visual_tools::local(
        &runtime,
        1,
        1,
        0,
        &ToolCall {
            index: 0,
            id: "temporary".into(),
            name: "set_pet_expression".into(),
            arguments: json!({"expression":"(≧▽≦)"}),
        },
    )
    .unwrap();
    assert_eq!(value["expression"], "(≧▽≦)");
}
#[test]
fn accessory_tool_returns_only_current_session_and_no_file_paths() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    runtime.read(|s,_|{s.connection.execute_batch("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'其他会话','a','a');INSERT INTO accessories(session_id,name,display_text,description,cost,source,created_at) VALUES(1,'小花','✿','花形挂件',20,'crafted','a'),(2,'另一个秘密挂件','*','不属于当前会话',20,'crafted','a');")?;Ok(())}).unwrap();
    let value = visual_tools::local(
        &runtime,
        1,
        1,
        0,
        &ToolCall {
            index: 0,
            id: "list".into(),
            name: "list_accessories".into(),
            arguments: json!({}),
        },
    )
    .unwrap();
    assert_eq!(value["items"].as_array().unwrap().len(), 1);
    assert_eq!(value["items"][0]["name"], "小花");
    assert!(value["items"][0].get("imageFile").is_none());
    assert!(visual_tools::local(
        &runtime,
        1,
        1,
        0,
        &ToolCall {
            index: 0,
            id: "bad".into(),
            name: "list_accessories".into(),
            arguments: json!({"sessionId":"2"})
        }
    )
    .is_err());
}

#[test]
fn all_custom_image_categories_are_searchable_and_selectable_with_pagination(){
 let runtime=crate::runtime::test_runtime("http://127.0.0.1:9/v1");
 runtime.read(|s,_|{s.connection.execute("UPDATE personas SET pet_image_enabled=1 WHERE id=1",[])?;
  for index in 1..=16{s.connection.execute("INSERT INTO pet_artworks(persona_id,kind,name,category,enabled,image_file,created_at,updated_at) VALUES(1,'expression',?1,?2,1,'custom.png','a','a')",rusqlite::params![format!("用户自定义{index}"),if index%2==0{"toss_up"}else{"rest_sit"}])?;}Ok(())
 }).unwrap();
 let query=|args|visual_tools::local(&runtime,1,1,0,&ToolCall{index:0,id:"query".into(),name:"list_pet_expressions".into(),arguments:args}).unwrap();
 let first=query(json!({"query":"用户自定义"}));assert_eq!(first["items"].as_array().unwrap().len(),12);
 let second=query(json!({"query":"用户自定义","beforeId":first["nextBeforeId"]}));assert_eq!(second["items"].as_array().unwrap().len(),4);
 let id=second["items"][0]["id"].as_str().unwrap();let selected=visual_tools::local(&runtime,1,1,0,&ToolCall{index:0,id:"select".into(),name:"set_pet_expression".into(),arguments:json!({"artworkId":id})}).unwrap();
 assert_eq!(selected["expression"],format!("@pet-image:{id}"));
}
struct PictureDirectory(PathBuf);

#[test]
fn chat_stickers_send_once_keep_history_and_disclose_catalog_only_on_demand() {
    for reply in ["今天也要开开心心。", ""] { tauri::async_runtime::block_on(async {
        let server=Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&call("tags","list_chat_sticker_tags",json!({}))),
            Response::sse(&call("search","search_chat_stickers",json!({"tagIds":["1"]}))),
            Response::sse(&call("send-once","send_chat_sticker",json!({"stickerId":"1"}))),
            Response::sse(&call("send-once","send_chat_sticker",json!({"stickerId":"1"}))),
            Response::sse(&(text_event(reply)+&finish_event("stop"))),
        ]);
        let dir=PictureDirectory::new();let mut runtime=crate::runtime::test_runtime(&server.url);runtime.paths=Ok(crate::platform::contracts::AppPaths::portable(dir.0.clone()));
        let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgba8(3,3).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        let file=crate::chat_images::publish(runtime.paths.as_ref().unwrap(),bytes.get_ref(),"开心.png").unwrap();
        runtime.read(|s,_|{s.connection.execute_batch("INSERT INTO sticker_libraries(id,title,created_at,updated_at) VALUES(1,'不应自动注入的私人库名','2026-10-28T00:00:00Z','2026-10-28T00:00:00Z');INSERT INTO sticker_tags(id,name,created_at,updated_at) VALUES(1,'独特开心标签','2026-10-28T00:00:00Z','2026-10-28T00:00:00Z');INSERT INTO persona_sticker_libraries VALUES(1,1,'2026-10-28T00:00:00Z');")?;
            s.connection.execute("INSERT INTO chat_stickers(id,library_id,title,description,filename,image_file,media_type,created_at,updated_at) VALUES(1,1,'秘密猫表情','很开心时使用','开心.png',?1,'image/png','2026-10-28T00:00:00Z','2026-10-28T00:00:00Z')",[&file])?;s.connection.execute("INSERT INTO sticker_tag_links VALUES(1,1)",[])?;Ok(())}).unwrap();
        let(prepared,_)=submit(&runtime,1,1,0,"发一个合适的表情包吧".into()).unwrap();
        let saved=run_persisted(&client(),prepared,|_|{}).await.unwrap();let message=saved.assistant_message.unwrap();assert_eq!(message.content,reply);assert_eq!(message.images.len(),1);assert_eq!(message.images[0].source_kind,"sticker");assert_ne!(message.images[0].image_file,file);
        let requests=server.requests.lock().unwrap();let initial=requests[1]["messages"].to_string();assert!(!initial.contains("私人库名"));assert!(!initial.contains("独特开心标签"));assert!(!initial.contains("秘密猫表情"));drop(requests);server.finish();
        runtime.read(|s,_|{crate::stickers::mutate(&mut s.connection,crate::stickers::Mutation::Delete{entity:"libraries".into(),id:"1".into(),revision:0})?;crate::chat_images::cleanup(&s.connection,runtime.paths.as_ref().unwrap())?;let read=crate::chat_images::data_url(runtime.paths.as_ref().unwrap(),&message.images[0])?;assert!(read.starts_with("data:image/png;base64,"));assert_eq!(crate::chat_images::list(&s.connection,crate::error::parse_id(&message.id)? )?.len(),1);Ok(())}).unwrap();
    });}
}

#[test]
fn chat_stickers_reject_unbound_ids_and_cancelled_requests() {
    let runtime=crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    let(prepared,_)=submit(&runtime,1,1,0,"发个表情包".into()).unwrap();
    let call=ToolCall{index:0,id:"send".into(),name:"send_chat_sticker".into(),arguments:json!({"stickerId":"999"})};
    assert_eq!(super::sticker_tools::send(&runtime,&prepared.lease,1,1,0,&call).unwrap_err().code,"NOT_FOUND");
    prepared.lease.cancel.cancel();
    assert!(super::sticker_tools::send(&runtime,&prepared.lease,1,1,0,&call).is_err());
    assert!(runtime.read(|s,_|Ok(crate::chat_images::list(&s.connection,1)?.is_empty())).unwrap());
}
impl PictureDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "yanmew-chat-picture-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for PictureDirectory {
    fn drop(&mut self) {
        if self.0.parent() == std::env::temp_dir().canonicalize().ok().as_deref()
            && self
                .0
                .file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with("yanmew-chat-picture-"))
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
#[test]
fn chat_generated_image_is_saved_once_and_reloaded_with_assistant_message() {
    tauri::async_runtime::block_on(async {
        use base64::Engine;
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(3, 3)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let pictures = Server::new_at(
            vec![Response {status:200,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&json!({"images":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(bytes.get_ref())}]})).unwrap())]}],
            "/v1/images/generations",
        );
        let args = json!({"prompt":"一只小猫在花园里"});
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&call("picture-1", "generate_chat_image", args.clone())),
            Response::sse(&call("picture-1", "generate_chat_image", args)),
            Response::sse(&call("inspect-1", "inspect_image_transparency", json!({"imageId":"1"}))),
            Response::sse(&call("view-1", "view_generated_image", json!({"imageId":"1"}))),
            Response::sse(&(text_event("画好了，请看这张图。") + &finish_event("stop"))),
        ]);
        let dir = PictureDirectory::new();
        let mut runtime = crate::runtime::test_runtime(&server.url);
        runtime.paths = Ok(crate::platform::contracts::AppPaths::portable(
            dir.0.clone(),
        ));
        runtime.read(|s,_|{s.connection.execute("INSERT INTO providers(id,name,base_url,api_key_encrypted) VALUES(2,'绘图',?1,'fake-image-key')",[&pictures.url])?;s.connection.execute_batch("INSERT INTO provider_models(provider_id,purpose,model) VALUES(2,'image','image-test');INSERT INTO provider_roles(purpose,provider_id,enabled) VALUES('image',2,1);")?;Ok(())}).unwrap();
        let (prepared, _) =
            submit(&runtime, 1, 1, 0, "请帮我生成一张小猫在花园里的图片".into()).unwrap();
        let saved = run_persisted(&client(), prepared, |_| {}).await.unwrap();
        let message = saved.assistant_message.unwrap();
        assert_eq!(message.images.len(), 1);
        assert_eq!(message.content, "画好了，请看这张图。");
        runtime
            .read(|s, _| {
                let restored = crate::sessions::round_snapshot(
                    &s.connection,
                    1,
                    1,
                    crate::error::parse_id(&saved.round.id)?,
                )?;
                assert_eq!(restored.assistant_message.unwrap().images.len(), 1);
                let path = crate::chat_images::managed_file(
                    runtime.paths.as_ref().unwrap(),
                    &message.images[0].image_file,
                )?;
                assert!(path.is_file());
                assert!(message.images[0]
                    .caption
                    .as_deref()
                    .unwrap()
                    .contains("一只小猫在花园里"));
                Ok(())
            })
            .unwrap();
        assert_eq!(pictures.requests.lock().unwrap().len(), 1);
        let requests=server.requests.lock().unwrap();
        let last=requests.last().unwrap()["messages"].as_array().unwrap();
        assert!(last.iter().any(|m|m["role"]=="tool"&&m["content"].as_str().unwrap_or("").contains("hasTransparency\":false")));
        assert!(last.iter().any(|m|m["content"].as_array().is_some_and(|p|p.iter().any(|part|part["type"]=="image_url"))));
        drop(requests);
        pictures.finish();
        server.finish();
    });
}

#[test]
fn own_base_image_tool_uses_actual_image_or_configured_vision_without_saving_attachment(){
 tauri::async_runtime::block_on(async{
  for separate in [false,true]{
   let server=Server::new(vec![Response::sse(&call("own-image","view_pet_base_image",json!({}))),Response::sse(&(text_event("我看到了自己的形象。")+&finish_event("stop")))]);
   let vision=separate.then(||Server::new(vec![Response::json(json!({"description":"可见粉色外套和猫耳"}))]));
   let dir=PictureDirectory::new();let mut runtime=crate::runtime::test_runtime(&server.url);runtime.paths=Ok(crate::platform::contracts::AppPaths::portable(dir.0.clone()));
   let mut pixels=image::RgbaImage::new(4,4);pixels.put_pixel(1,1,image::Rgba([220,100,150,255]));let mut raw=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(pixels).write_to(&mut raw,image::ImageFormat::Png).unwrap();
   let png=crate::accessories::images::normalize(raw.get_ref()).unwrap();let file=crate::accessories::images::publish(runtime.paths.as_ref().unwrap(),&png).unwrap();
   runtime.read(|s,_|{
    s.connection.execute("INSERT INTO pet_artworks(persona_id,kind,name,category,image_file,created_at,updated_at) VALUES(1,'base','默认形象','normal',?1,'a','a')",[&file])?;s.connection.execute("UPDATE personas SET pet_image_enabled=1 WHERE id=1",[])?;
    if let Some(vision)=&vision{s.connection.execute("INSERT INTO providers(id,name,base_url) VALUES(2,'识图',?1)",[&vision.url])?;s.connection.execute_batch("INSERT INTO provider_models(provider_id,purpose,model) VALUES(2,'vision','vision-model');INSERT INTO provider_roles(purpose,provider_id) VALUES('vision',2);")?;}Ok(())
   }).unwrap();
   let (prepared,_)=submit(&runtime,1,1,0,"看看你自己的默认形象".into()).unwrap();let saved=run_persisted(&client(),prepared,|_|{}).await.unwrap();assert!(saved.user_message.images.is_empty());assert!(saved.assistant_message.unwrap().images.is_empty());
   let requests=server.requests.lock().unwrap();assert_eq!(requests.len(),2);assert!(requests[0]["tools"].as_array().unwrap().iter().any(|t|t["function"]["name"]=="view_pet_base_image"));
   let messages=requests[1]["messages"].as_array().unwrap();let references=messages.iter().filter(|m|m["content"].as_array().is_some_and(|a|a.iter().any(|p|p["type"]=="image_url"))).count();assert_eq!(references,if separate{0}else{1});
   if separate{assert!(messages.iter().any(|m|m["role"]=="tool"&&m["content"].as_str().unwrap_or("").contains("粉色外套")));assert_eq!(vision.as_ref().unwrap().requests.lock().unwrap().len(),1);}
   runtime.read(|s,_|{assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM message_images",[],|r|r.get::<_,i64>(0))?,0);Ok(())}).unwrap();
  }
 });
}
#[test]
fn base_image_view_is_not_available_in_text_mode_or_with_path_arguments(){
 tauri::async_runtime::block_on(async{
  assert!(!visual_tools::definitions(false).iter().any(|t|t["function"]["name"]=="view_pet_base_image"));
  let runtime=crate::runtime::test_runtime("http://127.0.0.1:1/v1");let lease=runtime.requests.start("chat:1".into(),1).unwrap();
  let config=runtime.read(|s,p|crate::providers::connection_test_config(&s.connection,1,p)).unwrap();
  for arguments in [json!({}),json!({"path":"private.png"}),json!({"personaId":"2"})]{
   assert!(super::self_image::view(&client(),&runtime,&lease,1,1,0,&config,&ToolCall{index:0,id:"base".into(),name:"view_pet_base_image".into(),arguments}).await.is_err());
  }
 });
}

#[test]
fn generated_image_cutout_keeps_original_and_saves_once() {
 tauri::async_runtime::block_on(async{
  let server=Server::with_request_wait(vec![Response::json(json!({"expression":"(^_^)"})),Response::sse(&call("cutout","remove_image_background",json!({"imageId":"1"}))),Response::sse(&call("cutout","remove_image_background",json!({"imageId":"1"}))),Response::sse(&(text_event("透明图片已保存，原图保留。")+&finish_event("stop")))],std::time::Duration::from_secs(60));
  let dir=PictureDirectory::new();let mut runtime=crate::runtime::test_runtime(&server.url);runtime.paths=Ok(crate::platform::contracts::AppPaths::portable(dir.0.clone()));
  let base=fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/personas/deepseek/base.png")).unwrap();
  let rgba=crate::background_removal::decode(&base).unwrap();let mut opaque=image::RgbaImage::from_pixel(rgba.width(),rgba.height(),image::Rgba([255,255,255,255]));image::imageops::overlay(&mut opaque,&rgba,0,0);
  let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(opaque).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let file=crate::chat_images::publish(runtime.paths.as_ref().unwrap(),bytes.get_ref(),"original.png").unwrap();
  runtime.read(|s,_|{s.connection.execute("INSERT INTO messages(id,session_id,role,content,created_at) VALUES(100,1,'assistant','原图','a')",[])?;s.connection.execute("INSERT INTO message_images(id,message_id,filename,image_file,media_type,created_at) VALUES(1,100,'original.png',?1,'image/png','a')",[&file])?;Ok(())}).unwrap();
  let(prepared,_)=submit(&runtime,1,1,0,"把你生成的图片改成透明底".into()).unwrap();let local_client=AiClient::new(Default::default()).unwrap();let saved=run_persisted(&local_client,prepared,|_|{}).await.unwrap();
  let images=saved.assistant_message.unwrap().images;assert_eq!(images.len(),1);
  let original=crate::chat_images::managed_file(runtime.paths.as_ref().unwrap(),&file).unwrap();assert_eq!(fs::read(original).unwrap(),*bytes.get_ref());
  let result=fs::read(crate::chat_images::managed_file(runtime.paths.as_ref().unwrap(),&images[0].image_file).unwrap()).unwrap();assert!(crate::background_removal::inspect(&crate::background_removal::decode(&result).unwrap()).has_transparency);
  runtime.read(|s,_|{assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM message_images",[],|r|r.get::<_,i64>(0))?,2);Ok(())}).unwrap();server.finish();
 });
}

#[test]
fn reference_parameter_is_optional_and_only_offered_for_image_mode() {
 for image_mode in [false,true] {
  let definitions=visual_tools::definitions(image_mode);let tool=definitions.iter().find(|t|t["function"]["name"]=="generate_chat_image").unwrap();
  assert_eq!(tool["function"]["parameters"]["properties"].get("useReference").is_some(),image_mode);
  assert_eq!(tool["function"]["parameters"]["required"],json!(["prompt"]));
 }
}
#[test]
fn own_reference_picture_attaches_default_png_and_saves_one_normal_chat_image() {
 tauri::async_runtime::block_on(async {
  use std::io::{Read,Write};use base64::Engine;
  let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::new_rgb8(4,4).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
  let returned=base64::engine::general_purpose::STANDARD.encode(bytes.get_ref());
  let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let image_url=format!("http://{}/v1",listener.local_addr().unwrap());
  let worker=std::thread::spawn(move||{
   let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();let mut raw=Vec::new();let mut buf=[0u8;4096];let end=loop{let n=stream.read(&mut buf).unwrap();assert!(n>0);raw.extend_from_slice(&buf[..n]);if let Some(i)=raw.windows(4).position(|s|s==b"\r\n\r\n"){break i+4;}};
   let header=String::from_utf8_lossy(&raw[..end]).to_lowercase();assert!(header.starts_with("post /v1/images/edits "));let len:usize=header.lines().find_map(|s|s.strip_prefix("content-length:")).unwrap().trim().parse().unwrap();while raw.len()<end+len{let n=stream.read(&mut buf).unwrap();assert!(n>0);raw.extend_from_slice(&buf[..n]);}
   let payload=String::from_utf8_lossy(&raw[end..]);assert!(payload.contains("filename=\"reference.png\""));assert!(payload.contains("黄昏的海边"));assert!(payload.contains("用户自己的角色"));assert!(!payload.contains("name=\"background\""));assert!(!payload.contains("name=\"output_format\""));assert!(raw[end..].windows(8).any(|w|w==b"\x89PNG\r\n\x1a\n"));
   let body=json!({"data":[{"b64_json":returned}]}).to_string();write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
  });
  let arguments=json!({"prompt":"画你自己站在黄昏的海边，有海浪和天空背景","useReference":true});
  let server=Server::new(vec![Response::sse(&call("reference-picture","generate_chat_image",arguments.clone())),Response::sse(&call("reference-picture","generate_chat_image",arguments)),Response::sse(&(text_event("图片画好了。")+&finish_event("stop")))]);
  let dir=PictureDirectory::new();let mut runtime=crate::runtime::test_runtime(&server.url);runtime.paths=Ok(crate::platform::contracts::AppPaths::portable(dir.0.clone()));
  let mut rgba=image::RgbaImage::new(4,4);rgba.put_pixel(1,1,image::Rgba([120,150,220,255]));let mut data=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(rgba).write_to(&mut data,image::ImageFormat::Png).unwrap();
  let png=crate::accessories::images::normalize(data.get_ref()).unwrap();let file=crate::accessories::images::publish(runtime.paths.as_ref().unwrap(),&png).unwrap();
  runtime.read(|s,_|{s.connection.execute("INSERT INTO providers(id,name,base_url,api_key_encrypted) VALUES(2,'绘图',?1,'fake-image-key')",[&image_url])?;s.connection.execute_batch("INSERT INTO provider_models(provider_id,purpose,model) VALUES(2,'image','image-test');INSERT INTO provider_roles(purpose,provider_id,enabled) VALUES('image',2,1);UPDATE personas SET pet_image_enabled=1,name='用户自己的角色' WHERE id=1;")?;s.connection.execute("INSERT INTO pet_artworks(persona_id,kind,name,category,image_file,created_at,updated_at) VALUES(1,'base','默认图','normal',?1,'a','a')",[file])?;Ok(())}).unwrap();
  let reference_client=AiClient::new(crate::ai::client::Timeouts{connect:std::time::Duration::from_secs(1),first_event:std::time::Duration::from_secs(5),idle:std::time::Duration::from_secs(5),total:std::time::Duration::from_secs(20)}).unwrap();
  let(prepared,_)=submit(&runtime,1,1,0,"画你自己在海边".into()).unwrap();let saved=run_persisted(&reference_client,prepared,|_|{}).await.unwrap();let message=saved.assistant_message.unwrap();assert_eq!(message.images.len(),1);
  let image_file=crate::chat_images::managed_file(runtime.paths.as_ref().unwrap(),&message.images[0].image_file).unwrap();let rgba=image::open(image_file).unwrap().to_rgba8();assert!(rgba.pixels().all(|p|p[3]==255));
  let requests=server.requests.lock().unwrap();assert!(requests.last().unwrap()["messages"].as_array().unwrap().iter().any(|m|m["role"]=="tool"&&m["content"].as_str().unwrap_or("").contains("usedDefaultAppearance\":true")));drop(requests);worker.join().unwrap();server.finish();
 });
}
