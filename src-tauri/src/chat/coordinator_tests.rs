use crate::{
    ai::tests::{client, finish_event, text_event, Response, Server},
    chat::{cancel, submit, ChatEvent, ReplyEventKind},
    models::{ChatRoundStatus, MessageRole},
};
use serde_json::json;

#[test]
fn switching_context_rejects_old_submission_and_interrupts_inflight_round() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&(text_event("旧会话片段") + &finish_event("stop"))),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        assert!(submit(&runtime, 2, 1, 0, "越权".into()).is_err());
        let (prepared, _) = submit(&runtime, 1, 1, 0, "问题".into()).unwrap();
        let mut next_session = String::new();
        let saved = crate::chat::run_persisted(&client(), prepared, |event| {
            if matches!(event.event.kind, ReplyEventKind::Text { .. }) {
                runtime
                    .read(|s, _| {
                        next_session = crate::sessions::create(&mut s.connection, 1)?.id;
                        s.context_revision += 1;
                        runtime.requests.cancel_key("chat:1");
                        Ok(())
                    })
                    .unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(saved.round.status, ChatRoundStatus::Interrupted);
        assert_eq!(saved.assistant_message.unwrap().session_id, "1");
        assert!(submit(&runtime, 1, 1, 0, "旧页面提交".into()).is_err());
        assert!(runtime
            .read(|s, _| crate::sessions::messages(
                &s.connection,
                1,
                next_session.parse().unwrap(),
                None,
                20
            ))
            .unwrap()
            .items
            .is_empty());
        server.finish();
    });
}

#[test]
fn close_saves_partial_synchronously_and_late_terminal_does_not_duplicate() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&(text_event("已收到") + &finish_event("stop"))),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let (prepared, receipt) = submit(&runtime, 1, 1, 0, "问题".into()).unwrap();
        let saved = crate::chat::run_persisted(&client(), prepared, |event| {
            if matches!(event.event.kind, ReplyEventKind::Text { .. }) {
                let active = crate::chat::current(&runtime, 1, 1, 0).unwrap().unwrap();
                assert_eq!(active.partial_content, "已收到");
                assert!(active.round.assistant_message.is_none());
                crate::chat::interrupt_active(&runtime).unwrap();
                crate::chat::interrupt_active(&runtime).unwrap();
                let stored = cancel(
                    &runtime,
                    1,
                    1,
                    receipt.request_id.parse().unwrap(),
                    receipt.round_id.parse().unwrap(),
                )
                .unwrap();
                assert_eq!(stored.round.status, ChatRoundStatus::Interrupted);
                assert_eq!(stored.assistant_message.unwrap().content, "已收到");
            }
        })
        .await
        .unwrap();
        assert_eq!(saved.round.status, ChatRoundStatus::Interrupted);
        assert_eq!(
            runtime
                .read(|s, _| crate::sessions::messages(&s.connection, 1, 1, None, 20))
                .unwrap()
                .items
                .len(),
            2
        );
        assert!(crate::chat::current(&runtime, 1, 1, 0).unwrap().is_none());
        assert!(!runtime.requests.is_active("chat:1").unwrap());
        server.finish();
    });
}

#[test]
fn submit_persists_pending_user_message_and_rejects_second_request() {
    let server = Server::new(vec![]);
    let runtime = crate::runtime::test_runtime(&server.url);

    let (prepared, snapshot) = submit(&runtime, 1, 1, 0, "  你好  ".into()).unwrap();
    assert_eq!(snapshot.round.round.status, ChatRoundStatus::Pending);
    assert_eq!(snapshot.round.user_message.role, MessageRole::User);
    assert_eq!(snapshot.round.user_message.content, "你好");
    assert_eq!(snapshot.round_id, snapshot.round.round.id);

    let messages = runtime
        .read(|store, _| crate::sessions::messages(&store.connection, 1, 1, None, 20))
        .unwrap()
        .items;
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "你好");
    assert_eq!(messages[0].role, MessageRole::User);

    assert!(submit(&runtime, 1, 1, 0, "再发一条".into()).is_err());
    assert!(runtime.requests.is_active("chat:1").unwrap());
    drop(prepared);
    server.finish();
}

#[test]
fn successful_stream_is_saved_before_terminal_event_is_delivered() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&(text_event("你好") + &finish_event("stop"))),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let (prepared, pending) = submit(&runtime, 1, 1, 0, "嗨".into()).unwrap();
        let runtime_for_sink = runtime.clone();
        let mut events = vec![];

        let saved = crate::chat::run_persisted(&client(), prepared, |event: ChatEvent| {
            if event.saved.is_some() {
                let row_status: String = runtime_for_sink
                    .read(|store, _| {
                        Ok(store.connection.query_row(
                            "SELECT status FROM chat_rounds WHERE id=?1",
                            [pending.round_id.parse::<i64>().unwrap()],
                            |row| row.get(0),
                        )?)
                    })
                    .unwrap();
                assert_eq!(row_status, "completed");
            }
            events.push(event);
        })
        .await
        .unwrap();

        assert_eq!(saved.round.status, ChatRoundStatus::Completed);
        let pet = runtime.pet_view().unwrap();
        assert_eq!(pet.activity, crate::pet::controller::Activity::Speaking);
        assert_eq!(pet.expression, "(^_^)");
        let terminal = events.last().unwrap();
        assert!(terminal.saved.is_some());
        assert!(matches!(
            &terminal.event.kind,
            ReplyEventKind::Finished { .. }
        ));
        assert_eq!(saved.user_message.content, "嗨");
        assert_eq!(saved.assistant_message.as_ref().unwrap().content, "你好");
        assert_eq!(
            runtime
                .read(|store, _| {
                    Ok(store.connection.query_row(
                        "SELECT count(*) FROM messages WHERE session_id=1",
                        [],
                        |row| row.get::<_, i64>(0),
                    )?)
                })
                .unwrap(),
            2
        );
        server.finish();
    });
}

#[test]
fn stream_eof_without_finish_interrupts_and_saves_partial_text() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(・ω・)"})),
            Response::sse(&text_event("还没说完")),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let (prepared, _) = submit(&runtime, 1, 1, 0, "继续".into()).unwrap();
        let mut events = vec![];

        let saved = crate::chat::run_persisted(&client(), prepared, |event| events.push(event))
            .await
            .unwrap();

        assert_eq!(saved.round.status, ChatRoundStatus::Interrupted);
        assert_eq!(
            saved.assistant_message.as_ref().unwrap().content,
            "还没说完"
        );
        assert!(events.last().unwrap().saved.is_some());
        assert!(matches!(
            &events.last().unwrap().event.kind,
            ReplyEventKind::Failed { .. }
        ));
        server.finish();
    });
}

#[test]
fn length_finish_interrupts_and_saves_truncated_text() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(・ω・)"})),
            Response::sse(&(text_event("截断内容") + &finish_event("length"))),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let (prepared, _) = submit(&runtime, 1, 1, 0, "回答我".into()).unwrap();
        let mut events = vec![];

        let saved = crate::chat::run_persisted(&client(), prepared, |event| events.push(event))
            .await
            .unwrap();

        assert_eq!(saved.round.status, ChatRoundStatus::Interrupted);
        assert_eq!(
            saved.assistant_message.as_ref().unwrap().content,
            "截断内容"
        );
        assert!(events.last().unwrap().saved.is_some());
        assert!(matches!(
            &events.last().unwrap().event.kind,
            ReplyEventKind::Finished {
                finish: crate::ai::provider::Finish::Length
            }
        ));
        server.finish();
    });
}

#[test]
fn cancellation_after_expression_saves_user_only_and_terminal_is_readable() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(json!({"expression":"(^_^)"}))]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let (prepared, pending) = submit(&runtime, 1, 1, 0, "先等等".into()).unwrap();
        let runtime_for_sink = runtime.clone();
        let mut events = vec![];

        let saved = crate::chat::run_persisted(&client(), prepared, |event| {
            if matches!(&event.event.kind, ReplyEventKind::Expression { .. }) {
                let request_id = event.event.request_id.parse::<u64>().unwrap();
                assert!(runtime_for_sink
                    .requests
                    .cancel_request("chat:1", request_id, 1)
                    .unwrap());
            }
            events.push(event);
        })
        .await
        .unwrap();

        assert_eq!(saved.round.status, ChatRoundStatus::Interrupted);
        assert!(saved.assistant_message.is_none());
        assert_eq!(saved.user_message.content, "先等等");
        let terminal = cancel(
            &runtime,
            1,
            1,
            pending.request_id.parse().unwrap(),
            pending.round_id.parse().unwrap(),
        )
        .unwrap();
        assert_eq!(terminal.round.status, ChatRoundStatus::Interrupted);
        assert!(terminal.assistant_message.is_none());
        assert_eq!(terminal.user_message.id, pending.round.user_message.id);
        assert!(events.last().unwrap().saved.is_some());
        assert!(matches!(
            &events.last().unwrap().event.kind,
            ReplyEventKind::Cancelled
        ));
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        server.finish();
    });
}
