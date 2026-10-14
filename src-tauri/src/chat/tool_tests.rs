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

#[test]
fn normal_search_persists_sources_and_proactive_fabricated_calls_are_denied() {
    tauri::async_runtime::block_on(async {
        for proactive in [false,true] {
            let search = Server::new_at(if proactive {vec![]} else {vec![Response {
                status:200,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&json!({"results":[{"title":"真实资料","url":"https://example.com/news","content":"检索摘要"}]})).unwrap())],
            }]},"/v1/search");
            let server=Server::new(vec![Response::json(json!({"expression":"(^_^)"})),Response::sse(&calls(&[("web","search_web",json!({"query":"公开资料"}))])),Response::sse(&(text_event("如实回答。")+&finish_event("stop")))]);
            let runtime=crate::runtime::test_runtime(&server.url);
            runtime.read(|s,secrets|{
                crate::web_search::save(&mut s.connection,None,crate::web_search::Input {name:"搜索".into(),kind:crate::web_search::Kind::Tavily,base_url:search.url.clone(),api_key:crate::crypto::SecretUpdate::Set{value:"fake-test-key".into()}},secrets)?;
                crate::web_search::enable(&mut s.connection,true)
            }).unwrap();
            let (mut prepared,_)=submit(&runtime,1,1,0,"请核实资料".into()).unwrap();prepared.proactive=proactive;
            let mut searching=false;
            let saved=run_persisted(&client(),prepared,|event| {if matches!(event.event.kind,ReplyEventKind::WebActivity{phase:Some(_)}){searching=true;}}).await.unwrap();
            let content=saved.assistant_message.unwrap().content;
            let requests=server.requests.lock().unwrap();
            let exposed=requests[1]["tools"].as_array().unwrap().iter().any(|v|v["function"]["name"]=="search_web");
            let last=requests[2]["messages"].as_array().unwrap().last().unwrap();
            let result:Value=serde_json::from_str(last["content"].as_str().unwrap()).unwrap();
            if proactive {
                assert!(!exposed);assert_eq!(result["error"]["code"],"PERMISSION_DENIED");assert!(!content.contains("https://example.com/news"));
            } else {
                assert!(exposed);assert!(searching);assert_eq!(result["result"]["kind"],"search_summary");assert!(content.contains("https://example.com/news"));
            }
            drop(requests);server.finish();search.finish();
        }
    });
}

#[test]
fn accessory_tool_prepares_same_preview_without_purchasing_or_blocking_chat() {
    tauri::async_runtime::block_on(async {
        let args=json!({"name":"小蝴蝶结","display_text":"🎀","description":"想戴上它陪你说话。"});
        let server=Server::new(vec![Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&calls(&[("craft","request_accessory_craft",args.clone())])),
            Response::sse(&calls(&[("craft","request_accessory_craft",args)])),
            Response::sse(&(text_event("预览准备好了，是否购买由你决定。")+&finish_event("stop")))]);
        let runtime=crate::runtime::test_runtime(&server.url);
        runtime.read(|s,_|{s.connection.execute("UPDATE sessions SET intimacy=10 WHERE id=1",[])?;Ok(())}).unwrap();
        let (prepared,_)=submit(&runtime,1,1,0,"你想要什么饰品？".into()).unwrap();
        let saved=run_persisted(&client(),prepared,|_|{}).await.unwrap();assert_eq!(saved.round.status,crate::models::ChatRoundStatus::Completed);
        let context=crate::memory::management::Context {persona_id:"1".into(),session_id:"1".into(),context_revision:0};
        let preview=crate::accessories::craft::current(&runtime,&context).unwrap().unwrap();assert_eq!(preview.display_text,"🎀");assert_eq!(preview.cost,"20");
        runtime.read(|s,_|{assert_eq!(crate::currency::get_balance(&s.connection)?,0);assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM accessories",[],|r|r.get::<_,i64>(0))?,0);Ok(())}).unwrap();
        let requests=server.requests.lock().unwrap();
        let results:Vec<Value>=requests[3]["messages"].as_array().unwrap().iter().filter(|m|m["role"]=="tool").map(|m|serde_json::from_str(m["content"].as_str().unwrap()).unwrap()).collect();
        assert_eq!(results[0]["result"]["previewId"],results[1]["result"]["previewId"]);drop(requests);server.finish();
    });
}

#[test]
fn festival_tool_loop_saves_once_and_returns_receipt_to_model() {
    tauri::async_runtime::block_on(async {
        let input=json!({"kind":"birthday","name":"用户生日","calendarType":"solar","month":4,"day":6,"isLeapMonth":false,"blessing":"生日快乐","note":null});
        let args=json!({"input":input});
        let server=Server::new(vec![Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&calls(&[("date-write","create_session_festival",args.clone())])),
            Response::sse(&calls(&[("date-write","create_session_festival",args)])),
            Response::sse(&(text_event("已经记住你的生日日期。")+&finish_event("stop")))]);
        let runtime=crate::runtime::test_runtime(&server.url);
        let (prepared,_)=submit(&runtime,1,1,0,"我的公历生日是4月6日，请记录。".into()).unwrap();
        let saved=run_persisted(&client(),prepared,|_|{}).await.unwrap();
        assert_eq!(saved.assistant_message.unwrap().content,"已经记住你的生日日期。");
        runtime.read(|s,_|{assert_eq!(s.connection.query_row("SELECT count(*) FROM session_festivals WHERE session_id=1 AND source='ai' AND festival_id IS NULL",[],|r|r.get::<_,i64>(0))?,1);Ok(())}).unwrap();
        let requests=server.requests.lock().unwrap();
        assert!(requests[1]["tools"].as_array().unwrap().iter().any(|t|t["function"]["name"]=="create_session_festival"));
        let reply=requests[3]["messages"].as_array().unwrap().last().unwrap();
        assert_eq!(reply["tool_call_id"],"date-write");
        let receipt:Value=serde_json::from_str(reply["content"].as_str().unwrap()).unwrap();
        assert_eq!(receipt["result"]["already_applied"],true);
        drop(requests);server.finish();
    });
}

#[test]
fn regeneration_hides_and_rejects_festival_mutation_tools() {
    tauri::async_runtime::block_on(async {
        let args=json!({"input":{"kind":"custom","name":"禁止写入","calendarType":"solar","month":1,"day":1,"isLeapMonth":false,"blessing":"祝福","note":null}});
        let server=Server::new(vec![Response::sse(&calls(&[("forbidden-date","create_session_festival",args)]))]);
        let runtime=crate::runtime::test_runtime(&server.url);
        let (mut prepared,_)=submit(&runtime,1,1,0,"重说测试".into()).unwrap();prepared.regenerating=true;
        let result=tools::run(&client(),&prepared,&mut |_|Ok(())).await;
        assert_eq!(result.unwrap_err().code,"PERMISSION_DENIED");
        runtime.read(|s,_|{assert_eq!(s.connection.query_row("SELECT count(*) FROM session_festivals",[],|r|r.get::<_,i64>(0))?,0);Ok(())}).unwrap();
        let requests=server.requests.lock().unwrap();
        assert!(!requests[0]["tools"].as_array().unwrap().iter().any(|t|crate::festivals::tools::is_write(t["function"]["name"].as_str().unwrap())));
        drop(requests);server.finish();
    });
}
