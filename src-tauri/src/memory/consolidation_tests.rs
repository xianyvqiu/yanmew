use super::*;
use crate::ai::tests::{Response, Server};
use serde_json::json;

const SHORT: &str = "用户询问猫的生活习惯，助手说明猫喜欢安静环境并需要规律喂食。";
struct Sink;
impl crate::events::MemoryEventSink for Sink {
    fn memory_changed(&self) {}
}

fn set_k(runtime: &Runtime, k: usize) {
    runtime
        .read(|store, _| {
            store.connection.execute(
                "INSERT INTO settings(key,value) VALUES('memory_merge_count',?1)
            ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                [k.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
}
fn round(runtime: &Runtime, session_id: i64) -> WorkKey {
    runtime
        .read(|store, _| {
            let saved = crate::sessions::start_round(
                &mut store.connection,
                1,
                session_id,
                "猫的生活习惯是什么？",
            )?;
            let round_id = crate::error::parse_id(&saved.round.id)?;
            crate::sessions::complete_round(
                &mut store.connection,
                1,
                session_id,
                round_id,
                "猫喜欢安静环境。",
                None,
            )?;
            Ok(WorkKey {
                round_id,
                session_id,
                persona_id: 1,
            })
        })
        .unwrap()
}
fn seed(runtime: &Runtime, key: WorkKey, content: &str) {
    runtime
        .read(|store, _| {
            store.connection.execute(
                "INSERT INTO short_memories(session_id,round_id,content,created_at,updated_at)
            VALUES(?1,?2,?3,'a','a')",
                params![key.session_id, key.round_id, content],
            )?;
            store.connection.execute(
                "UPDATE sessions SET memory_revision=memory_revision+1 WHERE id=?1",
                [key.session_id],
            )?;
            Ok(())
        })
        .unwrap();
}
fn complete(runtime: &Runtime, session_id: i64) -> WorkKey {
    let key = round(runtime, session_id);
    seed(runtime, key, SHORT);
    key
}
fn value() -> GeneratedLongMemory {
    GeneratedLongMemory {
        title: "猫的生活习惯讨论".into(),
        content: SHORT.repeat(7),
    }
}
fn save(runtime: &Runtime, key: WorkKey) {
    let (snapshot, lease) = capture(runtime, key).unwrap().unwrap();
    assert_eq!(
        commit(runtime, &snapshot, &lease, &value()).unwrap(),
        Outcome::Saved
    );
    assert_eq!(
        commit(runtime, &snapshot, &lease, &value()).unwrap(),
        Outcome::AlreadySaved
    );
}
fn memories(runtime: &Runtime, session: i64) -> Vec<LongMemory> {
    runtime
        .read(|store, _| list_long(&store.connection, session))
        .unwrap()
}
fn pending(runtime: &Runtime, session: i64) -> Vec<i64> {
    runtime
        .read(|store, _| {
            Ok(pending_window(&store.connection, session, 100)?
                .iter()
                .map(|m| m.round_id)
                .collect())
        })
        .unwrap()
}

#[test]
fn default_twenty_and_future_k_changes_preserve_existing_sources() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    assert_eq!(
        runtime
            .read(|s, _| configuration(&s.connection))
            .unwrap()
            .merge_count,
        20
    );
    for _ in 0..20 {
        complete(&runtime, 1);
    }
    assert!(runtime
        .read(|s, _| candidates(&s.connection, &configuration(&s.connection)?))
        .unwrap()
        .is_empty());
    let trigger = complete(&runtime, 1);
    save(&runtime, trigger);
    assert_eq!(pending(&runtime, 1), vec![trigger.round_id]);
    let original = memories(&runtime, 1).remove(0);
    assert_eq!(original.source_count, 20);
    set_k(&runtime, 2);
    complete(&runtime, 1);
    let next = complete(&runtime, 1);
    save(&runtime, next);
    let long = memories(&runtime, 1);
    assert_eq!(long.len(), 2);
    assert_eq!(long[0], original);
    assert_eq!(long[1].source_count, 2);
    assert_eq!(long[1].activated_by_round_id, next.round_id.to_string());
    assert_eq!(pending(&runtime, 1), vec![next.round_id]);
}

#[test]
fn grouping_orders_source_rounds_and_never_crosses_missing_completed_memory() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    set_k(&runtime, 2);
    let first = round(&runtime, 1);
    let second = complete(&runtime, 1);
    let third = complete(&runtime, 1);
    assert!(capture(&runtime, third).unwrap().is_none());
    assert_eq!(
        current_status(&runtime, 1, 1, 0)
            .unwrap()
            .long_pending_count,
        0
    );
    seed(&runtime, first, SHORT);
    let (snapshot, lease) = capture(&runtime, third).unwrap().unwrap();
    assert_eq!(
        snapshot
            .window
            .iter()
            .map(|m| m.round_id)
            .collect::<Vec<_>>(),
        vec![first.round_id, second.round_id, third.round_id]
    );
    assert!(snapshot.window[0].id > snapshot.window[1].id);
    assert_eq!(
        commit(&runtime, &snapshot, &lease, &value()).unwrap(),
        Outcome::Saved
    );
    let source_rounds = runtime
        .read(|store, _| {
            let mut query = store.connection.prepare(
                "SELECT m.round_id FROM long_memory_sources s
            JOIN short_memories m ON m.id=s.short_memory_id ORDER BY s.source_order",
            )?;
            let rows = query
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .unwrap();
    assert_eq!(source_rounds, vec![first.round_id, second.round_id]);
}

#[test]
fn merge_commit_rejects_changed_source_k_provider_revision_and_cancel() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    set_k(&runtime, 2);
    complete(&runtime, 1);
    complete(&runtime, 1);
    let trigger = complete(&runtime, 1);
    for sql in [
        "UPDATE short_memories SET content=content||'来源已更新' WHERE round_id=(SELECT MIN(round_id) FROM short_memories)",
        "UPDATE settings SET value='3' WHERE key='memory_merge_count'",
        "UPDATE providers SET model='新模型' WHERE is_active=1",
        "UPDATE sessions SET memory_revision=memory_revision+1 WHERE id=1",
    ] {
        set_k(&runtime, 2);
        let (snapshot, lease) = capture(&runtime, trigger).unwrap().unwrap();
        runtime.read(|store, _| { store.connection.execute(sql, [])?; Ok(()) }).unwrap();
        assert_eq!(commit(&runtime, &snapshot, &lease, &value()).unwrap(), Outcome::Obsolete);
        assert!(memories(&runtime, 1).is_empty());
    }
    set_k(&runtime, 2);
    let (snapshot, lease) = capture(&runtime, trigger).unwrap().unwrap();
    lease.cancel.cancel();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, &value())
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    assert_eq!(pending(&runtime, 1).len(), 3);
}

#[test]
fn merge_transaction_failure_rolls_back_long_sources_states_and_fts() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    set_k(&runtime, 2);
    complete(&runtime, 1);
    complete(&runtime, 1);
    let trigger = complete(&runtime, 1);
    let (snapshot, lease) = capture(&runtime, trigger).unwrap().unwrap();
    runtime
        .read(|store, _| {
            store.connection.execute_batch(
                "CREATE TEMP TRIGGER fail_source BEFORE INSERT ON long_memory_sources
            WHEN new.source_order=2 BEGIN SELECT RAISE(ABORT,'模拟保存失败'); END;",
            )?;
            Ok(())
        })
        .unwrap();
    assert!(commit(&runtime, &snapshot, &lease, &value()).is_err());
    assert_eq!(pending(&runtime, 1).len(), 3);
    runtime
        .read(|store, _| {
            for query in [
                "SELECT COUNT(*) FROM long_memories",
                "SELECT COUNT(*) FROM long_memory_sources",
                "SELECT COUNT(*) FROM long_memories_fts WHERE long_memories_fts MATCH '猫的生活'",
            ] {
                assert_eq!(
                    store
                        .connection
                        .query_row(query, [], |r| r.get::<_, i64>(0))?,
                    0
                );
            }
            store.connection.execute_batch("DROP TRIGGER fail_source")?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, &value()).unwrap(),
        Outcome::Saved
    );
}

#[test]
fn deleting_trigger_restores_only_its_sources_and_is_atomic_with_round_deletion() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    set_k(&runtime, 2);
    let first = complete(&runtime, 1);
    let second = complete(&runtime, 1);
    let third = complete(&runtime, 1);
    save(&runtime, third);
    let fourth = complete(&runtime, 1);
    let fifth = complete(&runtime, 1);
    save(&runtime, fifth);
    assert_eq!(
        delete_completed_tail_round(&runtime, 1, 1, third.round_id)
            .unwrap_err()
            .code,
        "ROUND_NOT_TAIL"
    );
    assert!(delete_completed_tail_round(&runtime, 2, 1, fifth.round_id).is_err());
    runtime
        .read(|store, _| {
            store.connection.execute_batch(
                "CREATE TEMP TRIGGER fail_delete BEFORE DELETE ON chat_rounds
            BEGIN SELECT RAISE(ABORT,'模拟删除失败'); END;",
            )?;
            Ok(())
        })
        .unwrap();
    assert!(delete_completed_tail_round(&runtime, 1, 1, fifth.round_id).is_err());
    assert_eq!(memories(&runtime, 1).len(), 2);
    assert_eq!(pending(&runtime, 1), vec![fifth.round_id]);
    runtime
        .read(|store, _| {
            store.connection.execute_batch("DROP TRIGGER fail_delete")?;
            Ok(())
        })
        .unwrap();
    delete_completed_tail_round(&runtime, 1, 1, fifth.round_id).unwrap();
    assert_eq!(memories(&runtime, 1).len(), 1);
    assert_eq!(pending(&runtime, 1), vec![third.round_id, fourth.round_id]);
    runtime.read(|store, _| {
        let short = super::super::list_short(&store.connection, 1)?;
        assert_eq!(short.iter().map(|m| crate::error::parse_id(&m.round_id).unwrap()).collect::<Vec<_>>(), vec![first.round_id, second.round_id, third.round_id, fourth.round_id]);
        assert_eq!(short[0].integration_state, MemoryIntegrationState::Integrated);
        let messages = store.connection.query_row("SELECT COUNT(*) FROM messages WHERE round_id=?1", [fifth.round_id], |r| r.get::<_, i64>(0))?;
        assert_eq!(messages, 0);
        assert_eq!(store.connection.query_row("SELECT COUNT(*) FROM long_memories_fts WHERE long_memories_fts MATCH '猫的生活'", [], |r| r.get::<_, i64>(0))?, 1);
        Ok(())
    }).unwrap();
    let next = complete(&runtime, 1);
    save(&runtime, next);
    assert_eq!(memories(&runtime, 1).len(), 2);
}

#[test]
fn one_ai_call_generates_title_and_body_without_trigger_summary() {
    tauri::async_runtime::block_on(async {
        let expected = value();
        let server = Server::new(vec![Response::json(
            json!({"title":expected.title,"content":expected.content}),
        )]);
        let runtime = crate::runtime::test_runtime(&server.url);
        set_k(&runtime, 2);
        complete(&runtime, 1);
        complete(&runtime, 1);
        let trigger = round(&runtime, 1);
        seed(
            &runtime,
            trigger,
            "这是触发轮的独特内容，不得参加此前的长期整合。",
        );
        assert_eq!(
            generate(runtime.clone(), trigger).await.unwrap(),
            Outcome::Saved
        );
        assert_eq!(
            generate(runtime.clone(), trigger).await.unwrap(),
            Outcome::Missing
        );
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        let messages = requests[0]["messages"].as_array().unwrap();
        assert!(messages[0]["content"].as_str().unwrap().contains(FACT_RULE));
        let input: serde_json::Value =
            serde_json::from_str(messages[1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(input["source_memories"].as_array().unwrap().len(), 2);
        assert!(!messages[1]["content"]
            .as_str()
            .unwrap()
            .contains("触发轮的独特内容"));
        drop(requests);
        let config = runtime.read(|s, _| configuration(&s.connection)).unwrap();
        for text in [
            "{}",
            "{\"title\":\"\",\"content\":\"正文\"}",
            "{\"title\":\"标题\",\"content\":\"太短\"}",
            "{\"title\":\"标题\",\"content\":\"正文\",\"extra\":true}",
        ] {
            assert!(parse_output(text, &config).is_err());
        }
        assert_eq!(memories(&runtime, 1)[0].title, "猫的生活习惯讨论");
        server.finish();
    });
}

#[test]
fn worker_recovers_ready_groups_and_long_failure_does_not_block_other_session() {
    tauri::async_runtime::block_on(async {
        let output = value();
        let server = Server::new(vec![Response::json(
            json!({"title":output.title,"content":output.content}),
        )]);
        let runtime = crate::runtime::test_runtime(&server.url);
        set_k(&runtime, 2);
        complete(&runtime, 1);
        complete(&runtime, 1);
        let failed_trigger = complete(&runtime, 1);
        runtime.read(|store, _| {
            store.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'另一会话','a','a')", [])?;
            Ok(())
        }).unwrap();
        for _ in 0..3 {
            complete(&runtime, 2);
        }
        let mut worker = Worker::default();
        let key = TaskKey {
            scope: failed_trigger,
            kind: WorkKind::Long,
        };
        for _ in 0..3 {
            worker
                .finish(
                    &runtime,
                    key,
                    0,
                    Err(AppError::new("MEMORY_INVALID_OUTPUT", "模拟整合失败")),
                    Instant::now(),
                )
                .unwrap();
        }
        let view = current_status(&runtime, 1, 1, 0).unwrap();
        assert_eq!(view.long_pending_count, 1);
        assert!(view.long_error.is_some());
        assert!(view.error.is_none());
        assert!(!view.long_auto_retry_pending);
        let deadline = Instant::now() + Duration::from_secs(3);
        while memories(&runtime, 2).is_empty() {
            worker.tick(&runtime, &Sink).await.unwrap();
            assert!(Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(memories(&runtime, 1).is_empty());
        assert_eq!(memories(&runtime, 2).len(), 1);
        retry_current(&runtime, 1, 1, 0).unwrap();
        assert!(current_status(&runtime, 1, 1, 0)
            .unwrap()
            .long_error
            .is_none());
        server.finish();
    });
}
