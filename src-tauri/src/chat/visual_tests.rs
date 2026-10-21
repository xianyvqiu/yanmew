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
        assert_eq!(
            saved.assistant_message.unwrap().expression.as_deref(),
            Some("@pet-image:3")
        );
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
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
struct PictureDirectory(PathBuf);
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
        pictures.finish();
        server.finish();
    });
}
