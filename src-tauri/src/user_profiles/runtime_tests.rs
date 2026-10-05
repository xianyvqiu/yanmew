use super::*;
use crate::{chat, models::ChatRoundStatus, runtime::Runtime};

fn setup() -> (Runtime, i64) {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1");
    let profile = runtime
        .read(|s, _| {
            let id = create(&s.connection, "我的背景", "原背景")?;
            crate::sessions::set_user_profile(&mut s.connection, 1, 1, Some(id))?;
            Ok(id)
        })
        .unwrap();
    (runtime, profile)
}
fn started(runtime: &Runtime) -> (chat::PreparedReply, chat::RequestSnapshot) {
    let (reply, receipt) = chat::submit(runtime, 1, 1, 0, "问题".into()).unwrap();
    runtime
        .read(|s, _| {
            s.chat_requests.get_mut(&1).unwrap().partial_content = "已收到片段".into();
            Ok(())
        })
        .unwrap();
    (reply, receipt)
}
fn round(runtime: &Runtime, receipt: &chat::RequestSnapshot) -> crate::models::RoundSnapshot {
    runtime
        .read(|s, _| {
            crate::sessions::round_snapshot(&s.connection, 1, 1, receipt.round_id.parse().unwrap())
        })
        .unwrap()
}

#[test]
fn user_profile_runtime_update_interrupts_only_references_and_invalid_or_noop_does_not_cancel() {
    let (runtime, profile) = setup();
    let (reply, receipt) = started(&runtime);
    let unrelated = runtime.requests.start("chat:2".into(), 1).unwrap();
    assert_eq!(
        update_in_runtime(&runtime, profile, " ", "内容")
            .err()
            .unwrap()
            .code,
        "VALIDATION_ERROR"
    );
    assert_eq!(
        update_in_runtime(&runtime, profile + 100, "名称", "内容")
            .err()
            .unwrap()
            .code,
        "NOT_FOUND"
    );
    assert!(update_in_runtime(&runtime, profile, "我的背景", "原背景")
        .unwrap()
        .sessions
        .is_empty());
    assert_eq!(
        round(&runtime, &receipt).round.status,
        ChatRoundStatus::Pending
    );
    let outcome = update_in_runtime(&runtime, profile, "新名称", "新背景").unwrap();
    assert_eq!(outcome.sessions, vec![1]);
    assert_eq!(outcome.result.unwrap().content, "新背景");
    assert!(unrelated.cancel.check().is_ok());
    let saved = round(&runtime, &receipt);
    assert_eq!(saved.round.status, ChatRoundStatus::Interrupted);
    assert_eq!(saved.round.stop_reason.as_deref(), Some("user_cancelled"));
    assert_eq!(saved.assistant_message.unwrap().content, "已收到片段");
    runtime
        .read(|s, _| {
            assert_eq!(s.context_revision, 0);
            assert_eq!(crate::sessions::get(&s.connection, 1, 1)?.intimacy, 0);
            Ok(())
        })
        .unwrap();
    drop(reply);
    assert!(!runtime.requests.is_active("chat:1").unwrap());
}

#[test]
fn user_profile_runtime_binding_rejects_stale_targets_before_interrupting_and_preserves_state() {
    let (runtime, profile) = setup();
    let (reply, receipt) = started(&runtime);
    assert_eq!(
        bind_in_runtime(&runtime, 1, 1, 9, None).err().unwrap().code,
        "CONTEXT_CHANGED"
    );
    assert_eq!(
        bind_in_runtime(&runtime, 2, 1, 0, None).err().unwrap().code,
        "CONTEXT_CHANGED"
    );
    assert_eq!(
        bind_in_runtime(&runtime, 1, 2, 0, None).err().unwrap().code,
        "CONTEXT_CHANGED"
    );
    assert_eq!(
        bind_in_runtime(&runtime, 1, 1, 0, Some(profile + 100))
            .err()
            .unwrap()
            .code,
        "NOT_FOUND"
    );
    assert!(bind_in_runtime(&runtime, 1, 1, 0, Some(profile))
        .unwrap()
        .sessions
        .is_empty());
    assert_eq!(
        round(&runtime, &receipt).round.status,
        ChatRoundStatus::Pending
    );
    let saved = bind_in_runtime(&runtime, 1, 1, 0, None)
        .unwrap()
        .result
        .unwrap();
    assert!(saved.user_profile_id.is_none());
    assert!(saved.is_current);
    assert_eq!(saved.intimacy, 0);
    assert_eq!(
        round(&runtime, &receipt).assistant_message.unwrap().content,
        "已收到片段"
    );
    runtime
        .read(|s, _| {
            assert_eq!(s.context_revision, 0);
            Ok(())
        })
        .unwrap();
    drop(reply);
}

#[test]
fn user_profile_runtime_delete_unbinds_shared_sessions_but_keeps_interrupted_history() {
    let (runtime, profile) = setup();
    runtime.read(|s, _| {
        s.connection.execute("INSERT INTO sessions(id,persona_id,user_profile_id,title,created_at,updated_at) VALUES(2,1,?1,'另一会话','a','a')", [profile])?;
        Ok(())
    }).unwrap();
    let (reply, receipt) = started(&runtime);
    let outcome = delete_in_runtime(&runtime, profile).unwrap();
    assert_eq!(outcome.sessions, vec![1, 2]);
    outcome.result.unwrap();
    runtime
        .read(|s, _| {
            assert_eq!(crate::sessions::list(&s.connection, 1)?.len(), 2);
            assert!(crate::sessions::get(&s.connection, 1, 1)?
                .user_profile_id
                .is_none());
            assert!(crate::sessions::get(&s.connection, 1, 2)?
                .user_profile_id
                .is_none());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        round(&runtime, &receipt).assistant_message.unwrap().content,
        "已收到片段"
    );
    drop(reply);
}

#[test]
fn user_profile_runtime_failed_save_keeps_original_data_and_already_committed_interruption() {
    let (runtime, profile) = setup();
    let (reply, receipt) = started(&runtime);
    runtime.read(|s, _| {
        s.connection.execute_batch("CREATE TRIGGER reject_profile BEFORE UPDATE ON user_profiles BEGIN SELECT RAISE(ABORT,'test failure'); END;")?;
        Ok(())
    }).unwrap();
    let outcome = update_in_runtime(&runtime, profile, "新名", "新内容").unwrap();
    assert_eq!(outcome.result.unwrap_err().code, "STORAGE_ERROR");
    assert_eq!(outcome.sessions, vec![1]);
    assert_eq!(
        runtime
            .read(|s, _| get(&s.connection, profile))
            .unwrap()
            .content,
        "原背景"
    );
    assert_eq!(
        round(&runtime, &receipt).round.status,
        ChatRoundStatus::Interrupted
    );
    drop(reply);
}

#[test]
fn user_profile_runtime_regeneration_cancel_preserves_original_reply_and_memory() {
    let (runtime, profile) = setup();
    let (reply, receipt) = started(&runtime);
    let original = runtime.read(|s, _| {
        let saved = crate::sessions::complete_round(&mut s.connection, 1, 1, receipt.round_id.parse().unwrap(), "原完整回复", None)?;
        s.connection.execute("INSERT INTO short_memories(session_id,round_id,content,created_at,updated_at) VALUES(1,?1,'原短期记忆','a','a')", [receipt.round_id.parse::<i64>().unwrap()])?;
        s.chat_requests.remove(&1);
        Ok(saved)
    }).unwrap();
    drop(reply);
    let lease = runtime.requests.start("chat:1".into(), 1).unwrap();
    runtime
        .read(|s, _| {
            let mut preview = receipt.clone();
            preview.request_id = lease.id.to_string();
            preview.regenerating = true;
            preview.partial_content = "重说预览".into();
            preview.round = original.clone();
            s.chat_requests.insert(1, preview);
            Ok(())
        })
        .unwrap();
    delete_in_runtime(&runtime, profile)
        .unwrap()
        .result
        .unwrap();
    assert!(lease.cancel.check().is_err());
    assert_eq!(round(&runtime, &receipt), original);
    runtime
        .read(|s, _| {
            assert_eq!(
                s.connection
                    .query_row("SELECT content FROM short_memories", [], |r| r
                        .get::<_, String>(0))?,
                "原短期记忆"
            );
            assert_eq!(crate::sessions::get(&s.connection, 1, 1)?.intimacy, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn user_profile_runtime_real_stream_late_terminal_does_not_duplicate_or_complete_after_change() {
    use crate::ai::tests::{client, finish_event, text_event, Response, Server};
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(serde_json::json!({"expression":"(^_^)"})),
            Response::sse(&(text_event("已收到片段") + &finish_event("stop"))),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let profile = runtime
            .read(|s, _| create(&s.connection, "背景", "内容"))
            .unwrap();
        let (prepared, _) = chat::submit(&runtime, 1, 1, 0, "问题".into()).unwrap();
        let saved = chat::run_persisted(&client(), prepared, |event| {
            if matches!(event.event.kind, chat::ReplyEventKind::Text { .. }) {
                bind_in_runtime(&runtime, 1, 1, 0, Some(profile))
                    .unwrap()
                    .result
                    .unwrap();
            }
        })
        .await
        .unwrap();
        assert_eq!(saved.round.status, ChatRoundStatus::Interrupted);
        assert_eq!(saved.assistant_message.unwrap().content, "已收到片段");
        runtime
            .read(|s, _| {
                assert_eq!(
                    crate::sessions::messages(&s.connection, 1, 1, None, 20)?
                        .items
                        .len(),
                    2
                );
                assert_eq!(crate::sessions::get(&s.connection, 1, 1)?.intimacy, 0);
                Ok(())
            })
            .unwrap();
        assert!(chat::current(&runtime, 1, 1, 0).unwrap().is_none());
        assert!(!runtime.requests.is_active("chat:1").unwrap());
        server.finish();
    });
}
