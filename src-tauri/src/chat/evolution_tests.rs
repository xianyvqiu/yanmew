use super::*;
use crate::{
    ai::tests::{client, event, finish_event, text_event, Response, Server},
    events::{ChatToolEventSink, PersonaUpdated, ReminderEventSink},
    models::ChatRoundStatus,
};
use serde_json::{json, Value};

#[derive(Default)]
struct Events {
    updates: Mutex<Vec<PersonaUpdated>>,
    failures: Mutex<Vec<AppError>>,
}
impl ReminderEventSink for Events {
    fn reminders_changed(&self) {}
    fn reminder_failed(&self, _: &AppError) {}
}
impl ChatToolEventSink for Events {
    fn notebook_changed(&self) {}
    fn persona_updated(&self, update: &PersonaUpdated) {
        self.updates.lock().unwrap().push(update.clone());
    }
    fn persona_evolution_failed(&self, error: &AppError) {
        self.failures.lock().unwrap().push(error.clone());
    }
}
fn prompt(runtime: &Runtime) -> (String, String, Option<String>) {
    runtime
        .read(|s, _| {
            Ok(s.connection.query_row(
                "SELECT prompt,initial_prompt,last_evolved_at FROM personas WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap()
}
fn calls(args: Value, duplicate: bool) -> String {
    let call = json!({"index":0,"id":"evolve","type":"function","function":{
        "name":"update_persona_prompt","arguments":serde_json::to_string(&args).unwrap()}});
    let mut items = vec![call.clone()];
    if duplicate {
        let mut next = call;
        next["index"] = json!(1);
        next["id"] = json!("evolve-again");
        items.push(next);
    }
    event(json!({"choices":[{"index":0,"delta":{"tool_calls":items},"finish_reason":null}]}))
        + &finish_event("tool_calls")
}
fn server(args: Value, duplicate: bool, finish: &str) -> Server {
    Server::new(vec![
        Response::json(json!({"expression":"(^_^)"})),
        Response::sse(&calls(args, duplicate)),
        Response::sse(&(text_event("完整回复") + &finish_event(finish))),
    ])
}
fn offered(request: &Value) -> bool {
    request["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .any(|t| t["function"]["name"] == "update_persona_prompt")
    })
}

#[test]
fn candidate_is_saved_only_after_completed_reply_and_duplicate_is_idempotent() {
    let server = server(
        json!({"new_prompt":"电子小猫，保留核心设定，交流时认真解释重要误会。"}),
        true,
        "stop",
    );
    let runtime = crate::runtime::test_runtime(&server.url);
    let before = prompt(&runtime);
    let events = Arc::new(Events::default());
    let (mut prepared, _) = submit(&runtime, 1, 1, 0, "以后解释时先说结论再说明理由，这是长期的交流偏好。".into()).unwrap();
    prepared.set_tool_events(events.clone());
    let mut saw_text = false;
    let saved = tauri::async_runtime::block_on(run_persisted(&client(), prepared, |event| {
        if matches!(event.event.kind, ReplyEventKind::Text { .. }) {
            saw_text = true;
            assert_eq!(prompt(&runtime), before, "正文生成中不能提前改人设");
        }
    }))
    .unwrap();
    assert!(saw_text);
    assert_eq!(saved.round.status, ChatRoundStatus::Completed);
    let after = prompt(&runtime);
    assert_ne!(after.0, before.0);
    assert_eq!(after.1, before.1);
    assert!(after.2.is_some());
    assert_eq!(events.updates.lock().unwrap().len(), 1);
    assert_eq!(events.updates.lock().unwrap()[0].session_id, "1");
    assert!(events.failures.lock().unwrap().is_empty());
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 3, "没有额外的自调判断请求");
    assert!(offered(&requests[1]));
    let results: Vec<Value> = requests[2]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "tool")
        .map(|m| serde_json::from_str(m["content"].as_str().unwrap()).unwrap())
        .collect();
    assert_eq!(results[0]["result"]["saved"], false);
    assert_eq!(results[1]["result"]["already_staged"], true);
    drop(requests);
    server.finish();
}

#[test]
fn normal_chat_does_not_request_extra_evolution_check() {
    let server = Server::new(vec![
        Response::json(json!({"expression":"(^_^)"})),
        Response::sse(&(text_event("普通回复") + &finish_event("stop"))),
    ]);
    let runtime = crate::runtime::test_runtime(&server.url);
    let before = prompt(&runtime);
    let (prepared, _) = submit(&runtime, 1, 1, 0, "你好".into()).unwrap();
    let saved = tauri::async_runtime::block_on(run_persisted(&client(), prepared, |_| {})).unwrap();
    assert_eq!(saved.round.status, ChatRoundStatus::Completed);
    assert_eq!(prompt(&runtime), before);
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    server.finish();
}

#[test]
fn interrupted_length_cancel_or_changed_context_discards_candidate() {
    for mode in ["length", "cancel", "context"] {
        let server = server(
            json!({"new_prompt":"必要的完整新设定"}),
            false,
            if mode == "length" { "length" } else { "stop" },
        );
        let runtime = crate::runtime::test_runtime(&server.url);
        let before = prompt(&runtime);
        let events = Arc::new(Events::default());
        let (mut prepared, _) = submit(&runtime, 1, 1, 0, "交流".into()).unwrap();
        let cancel = prepared.lease.cancel.clone();
        prepared.set_tool_events(events.clone());
        let saved = tauri::async_runtime::block_on(run_persisted(&client(), prepared, |event| {
            if matches!(event.event.kind, ReplyEventKind::Text { .. }) {
                if mode == "cancel" {
                    cancel.cancel();
                }
                if mode == "context" {
                    runtime
                        .read(|s, _| {
                            s.context_revision += 1;
                            Ok(())
                        })
                        .unwrap();
                }
            }
        }))
        .unwrap();
        assert_eq!(saved.round.status, ChatRoundStatus::Interrupted, "{mode}");
        assert_eq!(prompt(&runtime), before, "{mode}");
        assert!(events.updates.lock().unwrap().is_empty());
        if mode == "length" {
            let (continuation, _) =
                continue_reply(&runtime, 1, 1, 0, saved.round.id.parse().unwrap()).unwrap();
            assert!(continuation.evolution.is_none(), "长度停止后不保留旧候选");
        }
        server.finish();
    }
}

#[test]
fn disabled_or_invalid_arguments_never_update_persona() {
    for (disabled, args) in [
        (true, json!({"new_prompt":"新设定"})),
        (false, json!({"new_prompt":"  "})),
        (
            false,
            json!({"new_prompt":"新设定","initial_prompt":"越权"}),
        ),
    ] {
        let server = server(args, false, "stop");
        let runtime = crate::runtime::test_runtime(&server.url);
        if disabled {
            runtime
                .read(|s, _| {
                    s.connection
                        .execute("UPDATE personas SET self_evolve_enabled=0 WHERE id=1", [])?;
                    Ok(())
                })
                .unwrap();
        }
        let before = prompt(&runtime);
        let (prepared, _) = submit(&runtime, 1, 1, 0, "交流".into()).unwrap();
        let saved =
            tauri::async_runtime::block_on(run_persisted(&client(), prepared, |_| {})).unwrap();
        assert_eq!(saved.round.status, ChatRoundStatus::Completed);
        assert_eq!(prompt(&runtime), before);
        if disabled {
            assert!(!offered(&server.requests.lock().unwrap()[1]));
        }
        server.finish();
    }
}

#[test]
fn changed_snapshot_or_switch_prevents_overwriting_user_work() {
    for sql in [
        "UPDATE personas SET prompt='用户手动编辑' WHERE id=1",
        "UPDATE personas SET initial_prompt='用户新基准' WHERE id=1",
        "UPDATE personas SET self_evolve_enabled=0 WHERE id=1",
        "UPDATE personas SET last_evolved_at='另一次调整' WHERE id=1",
    ] {
        let server = server(json!({"new_prompt":"旧候选"}), false, "stop");
        let runtime = crate::runtime::test_runtime(&server.url);
        let events = Arc::new(Events::default());
        let (mut prepared, _) = submit(&runtime, 1, 1, 0, "交流".into()).unwrap();
        prepared.set_tool_events(events.clone());
        let mut manual = None;
        let saved = tauri::async_runtime::block_on(run_persisted(&client(), prepared, |event| {
            if matches!(event.event.kind, ReplyEventKind::Text { .. }) {
                runtime
                    .read(|s, _| {
                        s.connection.execute(sql, [])?;
                        Ok(())
                    })
                    .unwrap();
                manual = Some(prompt(&runtime));
            }
        }))
        .unwrap();
        assert_eq!(saved.round.status, ChatRoundStatus::Completed);
        assert_eq!(prompt(&runtime), manual.unwrap());
        assert!(events.updates.lock().unwrap().is_empty());
        server.finish();
    }
}

#[test]
fn evolution_storage_failure_preserves_chat_and_reports_local_error() {
    let server = server(json!({"new_prompt":"新设定"}), false, "stop");
    let runtime = crate::runtime::test_runtime(&server.url);
    let before = prompt(&runtime);
    runtime.read(|s, _| { s.connection.execute_batch("CREATE TRIGGER test_fail_evolution BEFORE UPDATE OF prompt ON personas BEGIN SELECT RAISE(ABORT,'test failure'); END;")?; Ok(()) }).unwrap();
    let events = Arc::new(Events::default());
    let (mut prepared, _) = submit(&runtime, 1, 1, 0, "交流".into()).unwrap();
    prepared.set_tool_events(events.clone());
    let saved = tauri::async_runtime::block_on(run_persisted(&client(), prepared, |_| {})).unwrap();
    assert_eq!(saved.round.status, ChatRoundStatus::Completed);
    assert_eq!(saved.assistant_message.unwrap().content, "完整回复");
    assert_eq!(prompt(&runtime), before);
    assert_eq!(runtime.pet_view().unwrap().intimacy, Some(1));
    assert!(runtime.pet_view().unwrap().error.is_none());
    assert!(events.updates.lock().unwrap().is_empty());
    assert_eq!(
        events.failures.lock().unwrap()[0].code,
        "PERSONA_EVOLUTION_FAILED"
    );
    server.finish();
}
