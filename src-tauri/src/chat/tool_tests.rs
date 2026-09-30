use super::*;
use crate::ai::tests::{client, event, finish_event, text_event, Response, Server};
use serde_json::{json, Value};

fn calls(items: &[(&str, &str, Value)]) -> String {
    // 每个调用的参数拆成两个 SSE 数据块，验证组装后才执行。
    let mut output = String::new();
    for (index, (id, name, arguments)) in items.iter().enumerate() {
        let arguments = arguments.to_string();
        output.push_str(&event(json!({"choices":[{"index":0,"delta":{"tool_calls":[{
            "index":index,"id":id,"type":"function","function":{"name":name,"arguments":&arguments[..1]}
        }]},"finish_reason":null}]})));
        output.push_str(&event(json!({"choices":[{"index":0,"delta":{"tool_calls":[{
            "index":index,"function":{"arguments":&arguments[1..]}
        }]},"finish_reason":null}]})));
    }
    output + &finish_event("tool_calls")
}
fn append() -> Value { json!({"operation":"append","section_index":null,"content":"中文🙂内容"}) }

#[test]
fn fragmented_tools_return_matching_ids_and_writes_persist_once() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&calls(&[("read", "read_notebook", json!({})), ("save", "update_notebook", append()),
                ("unknown", "execute_sql", json!({"sql":"DELETE FROM notebooks"}))])),
            Response::sse(&calls(&[("save", "update_notebook", append())])),
            Response::sse(&(text_event("已经记下来了。") + &finish_event("stop"))),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        runtime.read(|s, _| crate::config::save(&mut s.connection, [("notebook_max_chars".into(),"2".into())].into_iter().collect())).unwrap();
        let (prepared, _) = submit(&runtime, 1, 1, 0, "记下这些".into()).unwrap();
        let saved = run_persisted(&client(), prepared, |_| {}).await.unwrap();
        assert_eq!(saved.assistant_message.unwrap().content, "已经记下来了。");
        let view = crate::notebook::current(&runtime, 1, 1, 0).unwrap();
        assert_eq!(view.notebook.unwrap().content, "中文🙂内容");
        let requests = server.requests.lock().unwrap();
        let messages = requests[2]["messages"].as_array().unwrap();
        let assistant = messages.iter().find(|m| m["tool_calls"].is_array()).unwrap();
        assert!(assistant["tool_calls"][0]["function"]["arguments"].is_string());
        let results: Vec<_> = messages.iter().filter(|m| m["role"] == "tool").collect();
        assert_eq!(results.iter().map(|m| m["tool_call_id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["read","save","unknown"]);
        let value: Value = serde_json::from_str(results[1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(value["result"]["over_limit"], true);
        let value: Value = serde_json::from_str(results[2]["content"].as_str().unwrap()).unwrap();
        assert_eq!(value["error"]["code"], "UNKNOWN_TOOL");
        let duplicate = requests[3]["messages"].as_array().unwrap().last().unwrap();
        let value: Value = serde_json::from_str(duplicate["content"].as_str().unwrap()).unwrap();
        assert_eq!(value["result"]["already_applied"], true);
        drop(requests);
        server.finish();
    });
}

#[test]
fn tool_round_limit_blocks_further_writes_and_marks_round_interrupted() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&calls(&[("read","read_notebook",json!({}))])),
            Response::sse(&calls(&[("save","update_notebook",append())]))]);
        let runtime = crate::runtime::test_runtime(&server.url);
        runtime.read(|s, _| crate::config::save(&mut s.connection, [("tool_call_rounds".into(),"1".into())].into_iter().collect())).unwrap();
        let (prepared, _) = submit(&runtime, 1, 1, 0, "hi".into()).unwrap();
        let mut failure = String::new();
        let saved = run_persisted(&client(), prepared, |event| if let ReplyEventKind::Failed {code,..} = event.event.kind {failure=code}).await.unwrap();
        assert_eq!(failure, "TOOL_LIMIT_REACHED");
        assert_eq!(saved.round.status, crate::models::ChatRoundStatus::Interrupted);
        assert!(crate::notebook::current(&runtime,1,1,0).unwrap().notebook.is_none());
        assert!(server.requests.lock().unwrap()[2].get("tools").is_none());
        server.finish();
    });
}

struct CancelAfterSave(crate::state::Cancellation);
impl crate::events::ReminderEventSink for CancelAfterSave {
    fn reminders_changed(&self) {}
    fn reminder_failed(&self, _: &AppError) {}
}
impl crate::events::ChatToolEventSink for CancelAfterSave { fn notebook_changed(&self) { self.0.cancel(); } }

#[test]
fn cancelling_after_tool_commit_keeps_notebook_and_skips_next_call() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&calls(&[("read","read_notebook",json!({})),("save","update_notebook",append()),
                ("again","update_notebook",append())]))]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let (mut prepared, _) = submit(&runtime, 1, 1, 0, "hi".into()).unwrap();
        prepared.set_tool_events(Arc::new(CancelAfterSave(prepared.lease.cancel.clone())));
        let saved = run_persisted(&client(), prepared, |_| {}).await.unwrap();
        assert_eq!(saved.round.status, crate::models::ChatRoundStatus::Interrupted);
        assert_eq!(crate::notebook::current(&runtime,1,1,0).unwrap().notebook.unwrap().content,"中文🙂内容");
        assert_eq!(server.requests.lock().unwrap().len(),2);
        server.finish();
    });
}

#[test]
fn manual_notebook_save_rejects_stale_snapshot_and_wrong_context() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    let initial = crate::notebook::current(&runtime,1,1,0).unwrap();
    let saved = crate::notebook::save_current(&runtime,1,1,0,initial.notebook.clone(),"第一版".into()).unwrap();
    assert_eq!(crate::notebook::save_current(&runtime,1,1,0,initial.notebook,"旧页覆盖".into()).err().unwrap().code,"NOTEBOOK_CONFLICT");
    assert_eq!(crate::notebook::current(&runtime,1,1,99).err().unwrap().code,"CONTEXT_CHANGED");
    let cleared = crate::notebook::save_current(&runtime,1,1,0,saved.notebook,String::new()).unwrap();
    assert_eq!(cleared.notebook.unwrap().content, "");
}

#[test]
fn chat_reminder_uses_existing_storage_and_final_reply_has_no_tools_at_limit() {
    tauri::async_runtime::block_on(async {
        let args = json!({"name":"喝水","content":"喝一杯水","fireAt":null,
            "schedule":{"version":1,"repeat":"daily","time":"09:00"}});
        let server = Server::new(vec![Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&calls(&[("remind","create_reminder",args.clone())])),
            Response::sse(&calls(&[("remind","create_reminder",args)])),
            Response::sse(&(text_event("每日提醒已创建。") + &finish_event("stop")))]);
        let runtime = crate::runtime::test_runtime(&server.url);
        runtime.read(|s, _| crate::config::save(&mut s.connection, [("tool_call_rounds".into(),"2".into())].into_iter().collect())).unwrap();
        let (prepared, _) = submit(&runtime,1,1,0,"每天9点提醒我喝水".into()).unwrap();
        let saved = run_persisted(&client(),prepared, |_| {}).await.unwrap();
        assert_eq!(saved.round.status,crate::models::ChatRoundStatus::Completed);
        runtime.read(|s, _| {
            assert_eq!(s.connection.query_row("SELECT count(*) FROM reminders WHERE source='chat'",[],|r|r.get::<_,i64>(0))?,1);
            Ok(())
        }).unwrap();
        assert!(server.requests.lock().unwrap()[3].get("tools").is_none());
        server.finish();
    });
}
