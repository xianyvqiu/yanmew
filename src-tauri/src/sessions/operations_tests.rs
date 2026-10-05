use super::*;
use crate::{
    chat::RequestSnapshot,
    platform::contracts::{Clock, ClockSample},
    state::RequestLease,
};
use std::{sync::Arc, time::Duration};

struct FixedClock;
impl Clock for FixedClock {
    fn sample(&self) -> AppResult<ClockSample> {
        Ok(ClockSample {
            utc: chrono::DateTime::parse_from_rfc3339("2026-10-06T10:00:00.000Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            running: Duration::ZERO,
        })
    }
}

fn runtime() -> Runtime {
    let mut runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    runtime.clock = Arc::new(FixedClock);
    runtime
}

fn add_session(runtime: &Runtime, id: i64, persona: i64) {
    runtime
        .read(|store, _| {
            store.connection.execute(
                "INSERT INTO sessions(id,persona_id,title,created_at,updated_at)
             VALUES(?1,?2,'保留会话','2026-10-06T10:00:00.000Z','2026-10-06T10:00:00.000Z')",
                params![id, persona],
            )?;
            Ok(())
        })
        .unwrap();
}

fn pending(runtime: &Runtime, session_id: i64) -> (RequestLease, RequestSnapshot) {
    let lease = runtime
        .requests
        .start(format!("chat:{session_id}"), 1)
        .unwrap();
    let snapshot = runtime
        .read(|store, _| {
            let round =
                crate::sessions::start_round(&mut store.connection, 1, session_id, "用户问题")?;
            let snapshot = RequestSnapshot {
                request_id: lease.id.to_string(),
                persona_id: "1".into(),
                session_id: session_id.to_string(),
                context_revision: store.context_revision,
                round_id: round.round.id.clone(),
                seq: 2,
                partial_content: "尚未落盘的片段".into(),
                expression: Some("(^_^)".into()),
                notice: None,
                budget: None,
                regenerating: false,
                round,
            };
            store.chat_requests.insert(session_id, snapshot.clone());
            Ok(snapshot)
        })
        .unwrap();
    (lease, snapshot)
}

fn assert_context(runtime: &Runtime, session_id: Option<&str>, revision: u64) {
    runtime
        .read(|store, _| {
            let context = crate::persona::context(&store.connection, store.context_revision)?;
            assert_eq!(context.persona_id, "1");
            assert_eq!(context.session_id.as_deref(), session_id);
            assert_eq!(context.context_revision, revision);
            Ok(())
        })
        .unwrap();
}

#[test]
fn current_delete_cancels_chat_rejects_late_result_and_restores_pet_or_empty_context() {
    tauri::async_runtime::block_on(async {
        let runtime = runtime();
        add_session(&runtime, 2, 1);
        runtime
            .read(|store, _| {
                store
                    .connection
                    .execute("UPDATE sessions SET intimacy=42 WHERE id=2", [])?;
                store.connection.execute(
                    "UPDATE session_pet_states SET hunger_elapsed_ms=12345 WHERE session_id=2",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let (prepared, snapshot) =
            crate::chat::submit(&runtime, 1, 1, 0, "用户问题".into()).unwrap();
        runtime
            .read(|store, _| {
                let active = store.chat_requests.get_mut(&1).unwrap();
                active.partial_content = "尚未落盘的片段".into();
                active.seq = 2;
                store.pet.chat_expression = Some("旧会话表情".into());
                Ok(())
            })
            .unwrap();
        let preview = runtime.requests.start("memory-preview".into(), 1).unwrap();

        let outcome = delete_in_context(&runtime, 1, 1, 0).unwrap();
        assert!(outcome.context_changed);
        assert!(outcome.pet_error.is_none());
        assert_eq!(outcome.result.current_session_id.as_deref(), Some("2"));
        assert_eq!(outcome.result.context_revision, 1);
        assert_eq!(outcome.context.session_id.as_deref(), Some("2"));
        assert_eq!(preview.cancel.check().unwrap_err().code, "CANCELLED");
        assert_context(&runtime, Some("2"), 1);
        runtime
            .read(|store, _| {
                assert!(!store.chat_requests.contains_key(&1));
                assert!(store.pet.chat_expression.is_none());
                assert_eq!(
                    crate::pet_store::read(&store.connection, 2)?.hunger_elapsed_ms,
                    12345
                );
                assert!(crate::sessions::round_snapshot(
                    &store.connection,
                    1,
                    1,
                    crate::error::parse_id(&snapshot.round_id)?
                )
                .is_err());
                Ok(())
            })
            .unwrap();
        let pet = runtime.pet_view().unwrap();
        assert_eq!(pet.session_id.as_deref(), Some("2"));
        assert_eq!(pet.context_revision, 1);
        assert_eq!(pet.intimacy, Some(42));

        let mut streamed = Vec::new();
        let error = crate::chat::run_reply(&crate::ai::tests::client(), &prepared, |event| {
            streamed.push(event);
            Ok(())
        })
        .await
        .unwrap_err();
        assert_eq!(error.code, "CANCELLED");
        assert_eq!(streamed.len(), 1);
        assert!(matches!(
            streamed[0].kind,
            crate::chat::ReplyEventKind::Cancelled
        ));
        let mut terminal = Vec::new();
        assert!(
            crate::chat::run_persisted(&crate::ai::tests::client(), prepared, |event| terminal
                .push(event))
            .await
            .is_err()
        );
        assert_eq!(terminal.len(), 1);
        assert!(terminal[0].saved.is_none());
        assert!(!runtime.requests.is_active("chat:1").unwrap());
        runtime
            .read(|store, _| {
                assert!(store.chat_requests.is_empty());
                assert_eq!(
                    store
                        .connection
                        .query_row("SELECT COUNT(*) FROM messages", [], |row| row
                            .get::<_, i64>(0))?,
                    0
                );
                assert_eq!(
                    store
                        .connection
                        .query_row("SELECT COUNT(*) FROM chat_rounds", [], |row| row
                            .get::<_, i64>(0))?,
                    0
                );
                Ok(())
            })
            .unwrap();

        let last = delete_in_context(&runtime, 1, 2, 1).unwrap();
        assert!(last.context_changed);
        assert!(last.pet_error.is_none());
        assert!(last.result.current_session_id.is_none());
        assert_eq!(last.result.context_revision, 2);
        assert_context(&runtime, None, 2);
        let empty = runtime.pet_view().unwrap();
        assert!(empty.session_id.is_none());
        assert!(empty.intimacy.is_none());
    });
}

#[test]
fn non_current_delete_preserves_current_chat_preview_pet_and_revision() {
    let runtime = runtime();
    add_session(&runtime, 2, 1);
    let (current_chat, current_snapshot) = pending(&runtime, 1);
    let (target_chat, target_snapshot) = pending(&runtime, 2);
    let preview = runtime.requests.start("memory-preview".into(), 1).unwrap();
    let target_keys = [
        format!("session-title:2"),
        format!("short-memory:{}", target_snapshot.round_id),
        format!("long-memory:{}", target_snapshot.round_id),
    ];
    let current_keys = [
        format!("session-title:1"),
        format!("short-memory:{}", current_snapshot.round_id),
        format!("long-memory:{}", current_snapshot.round_id),
    ];
    let target_leases: Vec<_> = target_keys
        .iter()
        .map(|key| runtime.requests.start(key.clone(), 1).unwrap())
        .collect();
    let current_leases: Vec<_> = current_keys
        .iter()
        .map(|key| runtime.requests.start(key.clone(), 1).unwrap())
        .collect();
    runtime.pet_view().unwrap();
    let pet_before = runtime
        .read(|store, _| {
            store.pet.chat_expression = Some("当前表情".into());
            crate::pet_store::read(&store.connection, 1)
        })
        .unwrap();

    let outcome = delete_in_context(&runtime, 1, 2, 0).unwrap();
    assert!(!outcome.context_changed);
    assert!(outcome.pet_error.is_none());
    assert_eq!(outcome.result.current_session_id.as_deref(), Some("1"));
    assert_eq!(outcome.result.context_revision, 0);
    assert_context(&runtime, Some("1"), 0);
    assert!(current_chat.cancel.check().is_ok());
    assert!(preview.cancel.check().is_ok());
    assert_eq!(target_chat.cancel.check().unwrap_err().code, "CANCELLED");
    for lease in target_leases {
        assert_eq!(lease.cancel.check().unwrap_err().code, "CANCELLED");
    }
    for lease in current_leases {
        assert!(lease.cancel.check().is_ok());
    }
    runtime
        .read(|store, _| {
            assert_eq!(
                serde_json::to_value(store.chat_requests.get(&1).unwrap()).unwrap(),
                serde_json::to_value(&current_snapshot).unwrap()
            );
            assert!(!store.chat_requests.contains_key(&2));
            assert_eq!(store.pet.chat_expression.as_deref(), Some("当前表情"));
            assert_eq!(crate::pet_store::read(&store.connection, 1)?, pet_before);
            Ok(())
        })
        .unwrap();
}

#[test]
fn stale_revision_and_foreign_persona_refuse_before_cancelling_target_chat() {
    let runtime = runtime();
    let (chat, snapshot) = pending(&runtime, 1);
    runtime.read(|store, _| {
        store.connection.execute("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(2,'另一人设','设定','设定')", [])?;
        Ok(())
    }).unwrap();
    add_session(&runtime, 2, 2);
    let foreign_chat = runtime.requests.start("chat:2".into(), 2).unwrap();
    for (persona, session, revision) in [(1, 1, 1), (1, 2, 0), (2, 2, 0)] {
        let error = delete_in_context(&runtime, persona, session, revision)
            .err()
            .unwrap();
        assert_eq!(error.code, "CONTEXT_CHANGED");
        assert!(chat.cancel.check().is_ok());
        assert!(foreign_chat.cancel.check().is_ok());
    }
    assert_context(&runtime, Some("1"), 0);
    runtime
        .read(|store, _| {
            assert_eq!(
                serde_json::to_value(store.chat_requests.get(&1).unwrap()).unwrap(),
                serde_json::to_value(snapshot).unwrap()
            );
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT COUNT(*) FROM sessions", [], |row| row
                        .get::<_, i64>(0))?,
                2
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn failed_database_delete_preserves_current_data_and_revision_then_can_retry() {
    let runtime = runtime();
    add_session(&runtime, 2, 1);
    let (chat, snapshot) = pending(&runtime, 1);
    let title = runtime.requests.start("session-title:1".into(), 1).unwrap();
    let preview = runtime.requests.start("memory-preview".into(), 1).unwrap();
    runtime.read(|store, _| {
        store.connection.execute_batch(
            "INSERT INTO notebooks(session_id,content,updated_at) VALUES(1,'保留正文','2026-10-06T10:00:00.000Z');
             CREATE TRIGGER fail_runtime_delete BEFORE DELETE ON sessions WHEN OLD.id=1
             BEGIN SELECT RAISE(ABORT,'forced session delete failure'); END;"
        )?;
        Ok(())
    }).unwrap();
    assert!(delete_in_context(&runtime, 1, 1, 0).is_err());
    assert_eq!(chat.cancel.check().unwrap_err().code, "CANCELLED");
    assert!(title.cancel.check().is_ok());
    assert!(preview.cancel.check().is_ok());
    assert_context(&runtime, Some("1"), 0);
    runtime
        .read(|store, _| {
            assert_eq!(
                serde_json::to_value(store.chat_requests.get(&1).unwrap()).unwrap(),
                serde_json::to_value(&snapshot).unwrap()
            );
            let round = crate::sessions::round_snapshot(
                &store.connection,
                1,
                1,
                crate::error::parse_id(&snapshot.round_id)?,
            )?;
            assert_eq!(
                serde_json::to_value(round).unwrap(),
                serde_json::to_value(&snapshot.round).unwrap()
            );
            assert_eq!(
                store.connection.query_row(
                    "SELECT content FROM notebooks WHERE session_id=1",
                    [],
                    |row| row.get::<_, String>(0)
                )?,
                "保留正文"
            );
            store
                .connection
                .execute_batch("DROP TRIGGER fail_runtime_delete;")?;
            Ok(())
        })
        .unwrap();
    let retried = delete_in_context(&runtime, 1, 1, 0).unwrap();
    assert!(retried.context_changed);
    assert_eq!(retried.result.current_session_id.as_deref(), Some("2"));
    assert_eq!(retried.result.context_revision, 1);
    assert_eq!(title.cancel.check().unwrap_err().code, "CANCELLED");
    assert_eq!(preview.cancel.check().unwrap_err().code, "CANCELLED");
    assert_context(&runtime, Some("2"), 1);
}
