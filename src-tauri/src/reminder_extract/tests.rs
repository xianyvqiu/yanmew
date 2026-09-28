use super::*;
use crate::ai::{
    provider::GenerateRequest,
    tests::{client, event, finish_event, Response, Server},
};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct Events(AtomicUsize);
impl ReminderEventSink for Events {
    fn reminders_changed(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn reminder_failed(&self, _: &AppError) {
        panic!("工具失败应直接返回，不冒充后台调度故障");
    }
}

fn runtime() -> Runtime {
    crate::runtime::test_runtime("http://127.0.0.1:1/v1")
}
fn arguments() -> Value {
    json!({"name":"复诊","content":"带上检查报告去复诊","fireAt":"2099-10-02T09:00:00+08:00","schedule":null})
}
fn call(id: &str, arguments: Value) -> ToolCall {
    ToolCall {
        index: 0,
        id: id.into(),
        name: TOOL_NAME.into(),
        arguments,
    }
}
fn count(runtime: &Runtime) -> i64 {
    runtime
        .read(|store, _| {
            Ok(store
                .connection
                .query_row("SELECT count(*) FROM reminders", [], |row| row.get(0))?)
        })
        .unwrap()
}

#[test]
fn creates_all_four_kinds_in_existing_storage_with_chat_source() {
    let runtime = runtime();
    let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
    let mut handler = ReminderCreation::new(&runtime, &request, 1, 1, 0).unwrap();
    let events = Events::default();
    for (index, schedule) in [
        Value::Null,
        json!({"version":1,"repeat":"daily","time":"09:00"}),
        json!({"version":1,"repeat":"weekly","time":"09:00","weekdays":[1,5]}),
        json!({"version":1,"repeat":"monthly","time":"09:00","days":[31]}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = arguments();
        if !schedule.is_null() {
            input["fireAt"] = Value::Null;
        }
        input["schedule"] = schedule;
        let result = handler
            .execute(&call(&format!("call-{index}"), input), &Utc, &events)
            .unwrap();
        assert_eq!(
            result.reminder.kind,
            ["once", "daily", "weekly", "monthly"][index]
        );
        assert_eq!(result.reminder.source, "chat");
        assert_eq!(
            result.reminder.status,
            crate::models::ReminderStatus::Pending
        );
        assert_eq!(result.request_id, request.id.to_string());
        assert!(result.reminder.fire_at.is_some());
        assert!(!result.already_applied);
    }
    drop(handler);
    drop(request);
    assert_eq!(count(&runtime), 4);
    assert_eq!(events.0.load(Ordering::Relaxed), 4);
    runtime
        .read(|store, _| {
            let saved = reminders::get_rule(&store.connection, 1)?;
            assert_eq!(saved.fire_at.as_deref(), Some("2099-10-02T01:00:00.000Z"));
            Ok(())
        })
        .unwrap();
}

#[test]
fn invalid_or_incomplete_arguments_never_save_partial_rules() {
    let runtime = runtime();
    let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
    let mut handler = ReminderCreation::new(&runtime, &request, 1, 1, 0).unwrap();
    let events = Events::default();
    let mut invalid = vec![json!({"name":"复诊","content":"带报告"}), Value::Null];
    for (field, value) in [
        ("name", json!("  ")),
        ("content", json!(" ")),
        ("fireAt", Value::Null),
        ("fireAt", json!("明天上午")),
        ("fireAt", json!("2099-02-30T09:00:00+08:00")),
        ("fireAt", json!("2020-01-01T00:00:00Z")),
        ("fireAt", json!("2099-10-02T09:00:00")),
        ("sessionId", json!("2")),
        ("source", json!("manual")),
    ] {
        let mut value_to_check = arguments();
        value_to_check[field] = value;
        invalid.push(value_to_check);
    }
    for schedule in [
        json!({"version":1,"repeat":"daily","time":"9:00"}),
        json!({"version":2,"repeat":"daily","time":"09:00"}),
        json!({"repeat":"daily","time":"09:00"}),
        json!({"version":1,"repeat":"weekly","time":"09:00","weekdays":[]}),
        json!({"version":1,"repeat":"weekly","time":"09:00","weekdays":[0,8]}),
        json!({"version":1,"repeat":"weekly","time":"09:00","weekdays":[1,1]}),
        json!({"version":1,"repeat":"monthly","time":"09:00","days":[32]}),
        json!({"version":1,"repeat":"daily","time":"09:00","next_local_at":"2099-01-01T09:00:00"}),
    ] {
        let mut value_to_check = arguments();
        value_to_check["fireAt"] = Value::Null;
        value_to_check["schedule"] = schedule;
        invalid.push(value_to_check);
    }
    let mut both = arguments();
    both["schedule"] = json!({"version":1,"repeat":"daily","time":"09:00"});
    invalid.push(both);
    for input in invalid {
        assert_eq!(
            handler
                .execute(&call("bad", input), &Utc, &events)
                .unwrap_err()
                .code,
            "VALIDATION_ERROR"
        );
    }
    assert_eq!(count(&runtime), 0);
    assert_eq!(events.0.load(Ordering::Relaxed), 0);
}

#[test]
fn duplicate_call_is_not_replayed_and_changed_arguments_are_rejected() {
    let runtime = runtime();
    let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
    let mut handler = ReminderCreation::new(&runtime, &request, 1, 1, 0).unwrap();
    let events = Events::default();
    let original = call("same", arguments());
    let first = handler.execute(&original, &Utc, &events).unwrap();
    let second = handler.execute(&original, &Utc, &events).unwrap();
    assert!(second.already_applied);
    assert_eq!(first.reminder.id, second.reminder.id);
    let mut changed = original.clone();
    changed.arguments["content"] = json!("另一件事");
    assert_eq!(
        handler.execute(&changed, &Utc, &events).unwrap_err().code,
        "TOOL_CALL_CONFLICT"
    );
    assert_eq!(count(&runtime), 1);
    assert_eq!(events.0.load(Ordering::Relaxed), 1);
    // 用户已删除该提醒时，迟到的重复调用也不能把它重新创建。
    runtime
        .read(|store, _| reminders::delete_rule(&store.connection, 1))
        .unwrap();
    assert!(
        handler
            .execute(&original, &Utc, &events)
            .unwrap()
            .already_applied
    );
    assert_eq!(count(&runtime), 0);
}

#[test]
fn cancelled_changed_or_unrelated_requests_cannot_create_reminders() {
    let runtime = runtime();
    let events = Events::default();
    let wrong = Arc::new(runtime.requests.start("expressions:1".into(), 1).unwrap());
    assert!(ReminderCreation::new(&runtime, &wrong, 1, 1, 0).is_err());
    let other_runtime = self::runtime();
    let foreign = Arc::new(other_runtime.requests.start("chat:1".into(), 1).unwrap());
    assert!(ReminderCreation::new(&runtime, &foreign, 1, 1, 0).is_err());
    let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
    assert!(ReminderCreation::new(&runtime, &request, 2, 1, 0).is_err());
    let mut handler = ReminderCreation::new(&runtime, &request, 1, 1, 0).unwrap();
    runtime
        .read(|store, _| {
            store.context_revision += 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        handler
            .execute(&call("old", arguments()), &Utc, &events)
            .unwrap_err()
            .code,
        "CONTEXT_CHANGED"
    );
    drop(handler);
    let mut handler = ReminderCreation::new(&runtime, &request, 1, 1, 1).unwrap();
    request.cancel.cancel();
    assert_eq!(
        handler
            .execute(&call("cancelled", arguments()), &Utc, &events)
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    assert_eq!(count(&runtime), 0);
}

#[test]
fn database_failure_has_no_success_notification_and_can_retry_safely() {
    let runtime = runtime();
    let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
    let mut handler = ReminderCreation::new(&runtime, &request, 1, 1, 0).unwrap();
    let events = Events::default();
    runtime.read(|store, _| {
        store.connection.execute_batch("CREATE TRIGGER reject_reminder BEFORE INSERT ON reminders BEGIN SELECT RAISE(ABORT,'test'); END;")?; Ok(())
    }).unwrap();
    let call = call("retry", arguments());
    assert_eq!(
        handler.execute(&call, &Utc, &events).unwrap_err().code,
        "STORAGE_ERROR"
    );
    assert_eq!(count(&runtime), 0);
    assert_eq!(events.0.load(Ordering::Relaxed), 0);
    runtime
        .read(|store, _| {
            store
                .connection
                .execute_batch("DROP TRIGGER reject_reminder;")?;
            Ok(())
        })
        .unwrap();
    assert!(
        !handler
            .execute(&call, &Utc, &events)
            .unwrap()
            .already_applied
    );
    request.cancel.cancel();
    // 后续正文取消不撤销已经提交的全局提醒。
    assert_eq!(count(&runtime), 1);
}

#[test]
fn model_tool_response_uses_existing_transport_then_persists_real_result() {
    tauri::async_runtime::block_on(async {
        let arguments = arguments().to_string();
        let response = event(json!({"choices":[{"index":0,"delta":{"tool_calls":[{
            "index":0,"id":"call-reminder","type":"function","function":{"name":TOOL_NAME,"arguments":arguments}
        }]}}]}))
            + &finish_event("tool_calls");
        let server = Server::new(vec![Response::sse(&response)]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
        let zone = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
        let now = DateTime::parse_from_rfc3339("2026-09-28T03:00:00Z")
            .unwrap()
            .to_utc();
        let context = time_context(now, &zone);
        assert!(context.content.contains("2026-09-28T11:00:00+08:00"));
        assert!(context.content.contains("星期1"));
        let input = GenerateRequest {
            messages: vec![
                context,
                Message::new("user", "2099年10月2日上午9点提醒我带报告复诊"),
            ],
            tools: vec![definition()],
            json: false,
            max_tokens: 1024,
        };
        let choices = client()
            .stream(&server.config(), &input, &request.cancel, |_| Ok(()))
            .await
            .unwrap();
        assert_eq!(choices[0].tools.len(), 1);
        let mut handler = ReminderCreation::new(&runtime, &request, 1, 1, 0).unwrap();
        let tool_call = choices[0].tools[0].clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            handler.execute(&tool_call, &zone, &Events::default())
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.reminder.id, "1");
        assert_eq!(count(&runtime), 1);
        assert_eq!(
            server.requests.lock().unwrap()[0]["tools"][0]["function"]["name"],
            TOOL_NAME
        );
        server.finish();
    });
}
