use super::*;
use crate::ai::tests::{Response, Server};
use serde_json::json;

const SHORT: &str = "用户讨论规律喂食，助手建议每天按时喂猫。";
const LONG: &str = "用户讨论猫的习惯和规律喂食，助手提供安静环境与按时喂食建议。newbody";

fn completed(runtime: &Runtime) -> i64 {
    runtime
        .read(|store, _| {
            let round =
                crate::sessions::start_round(&mut store.connection, 1, 1, "猫需要怎样的照顾？")?;
            let id = crate::error::parse_id(&round.round.id)?;
            crate::sessions::complete_round(
                &mut store.connection,
                1,
                1,
                id,
                "旧回复：保持安静环境。",
                Some("喵"),
            )?;
            Ok(id)
        })
        .unwrap()
}

fn fixture(url: &str, with_memory: bool) -> (Runtime, i64) {
    let runtime = crate::runtime::test_runtime(url);
    let first = completed(&runtime);
    let tail = completed(&runtime);
    runtime.read(|store, _| {
        for (key, value) in [("short_memory_min_chars", "1"), ("short_memory_max_chars", "100"),
            ("long_memory_min_chars", "1"), ("long_memory_max_chars", "100")] {
            store.connection.execute("UPDATE settings SET value=?1 WHERE key=?2", params![value, key])?;
        }
        if with_memory {
            // 手动旧正文故意超过生成预算，读取和重说不应拒绝或裁剪它。
            store.connection.execute("INSERT INTO short_memories(id,session_id,round_id,content,integration_state,created_at,updated_at) VALUES(9,1,?1,?2,'integrated','a','a')",
                params![first, "猫喜欢安静环境。".repeat(30)])?;
            store.connection.execute("INSERT INTO short_memories(id,session_id,round_id,content,integration_state,created_at,updated_at) VALUES(3,1,?1,'旧短期内容','integrated','a','a')", [tail])?;
            store.connection.execute("INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(7,1,'原长期标题','原长期正文oldbody',2,?1,'a','a')", [tail])?;
            store.connection.execute("INSERT INTO long_memory_sources VALUES(7,9,1),(7,3,2)", [])?;
            store.connection.execute("INSERT INTO long_memory_embeddings VALUES(7,1,'test',2,X'0000803F00000000','old','a')", [])?;
        }
        Ok(())
    }).unwrap();
    (runtime, tail)
}

fn snapshot(runtime: &Runtime, round: i64) -> Snapshot {
    runtime
        .read(|store, _| capture(&store.connection, 1, 1, round))
        .unwrap()
}

fn replacement(snapshot: &Snapshot) -> Replacement {
    Replacement {
        short_content: SHORT.into(),
        longs: snapshot
            .longs
            .iter()
            .map(|long| {
                (
                    long.memory.id.clone(),
                    consolidation::GeneratedLongMemory {
                        title: "新的事实记录".into(),
                        content: LONG.into(),
                    },
                )
            })
            .collect(),
    }
}

fn assert_code<T>(result: AppResult<T>, code: &str) {
    match result {
        Err(error) => assert_eq!(error.code, code),
        Ok(_) => panic!("预期错误 {code}"),
    }
}

fn digest(runtime: &Runtime, round: i64) -> String {
    runtime
        .read(|store, _| {
            let round = crate::sessions::round_snapshot(&store.connection, 1, 1, round)?;
            let shorts = list_short(&store.connection, 1)?;
            let longs = consolidation::list_long(&store.connection, 1)?;
            let revision: i64 = store.connection.query_row(
                "SELECT memory_revision FROM sessions WHERE id=1",
                [],
                |r| r.get(0),
            )?;
            let vectors: i64 = store.connection.query_row(
                "SELECT COUNT(*) FROM long_memory_embeddings",
                [],
                |r| r.get(0),
            )?;
            Ok(serde_json::to_string(&(round, shorts, longs, revision, vectors)).unwrap())
        })
        .unwrap()
}

#[tokio::test]
async fn regeneration_reuses_original_sources_and_updates_in_place() {
    let server = Server::new(vec![
        Response::json(json!({"content":SHORT})),
        Response::json(json!({"title":"新的事实记录", "content":LONG})),
    ]);
    let (runtime, round) = fixture(&server.url, true);
    let original = snapshot(&runtime, round);
    let generated = generate(
        &runtime,
        &original,
        &Cancellation::default(),
        &original.round.user_message.content,
        "新回复：每天按时喂猫。",
    )
    .await
    .unwrap();
    {
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0]["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("新回复"));
        let payload: serde_json::Value =
            serde_json::from_str(requests[1]["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            payload["source_memories"][0],
            original.longs[0].sources[0].memory.content
        );
        assert_eq!(payload["source_memories"][1], SHORT);
        assert!(!requests[1]["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("原长期正文"));
    }
    runtime
        .read(|store, _| {
            let tx = store.connection.transaction()?;
            apply(&tx, &original, &generated, "b")?;
            tx.execute(
                "UPDATE messages SET content='新回复：每天按时喂猫。' WHERE id=?1",
                [crate::error::parse_id(
                    &original.round.assistant_message.as_ref().unwrap().id,
                )?],
            )?;
            tx.execute(
                "UPDATE sessions SET memory_revision=memory_revision+1 WHERE id=1",
                [],
            )?;
            tx.commit()?;
            let current = capture(&store.connection, 1, 1, round)?;
            assert_eq!(current.short.as_ref().unwrap().id, "3");
            assert_eq!(
                current.short.as_ref().unwrap().integration_state,
                MemoryIntegrationState::Integrated
            );
            assert_eq!(current.short.as_ref().unwrap().created_at, "a");
            assert_eq!(current.longs[0].memory.id, "7");
            assert_eq!(
                current.longs[0].memory.activated_by_round_id,
                original.longs[0].memory.activated_by_round_id
            );
            assert_eq!(
                current.longs[0]
                    .sources
                    .iter()
                    .map(|s| (&s.memory.id, s.order))
                    .collect::<Vec<_>>(),
                original.longs[0]
                    .sources
                    .iter()
                    .map(|s| (&s.memory.id, s.order))
                    .collect::<Vec<_>>()
            );
            assert_eq!(current.revision, original.revision + 1);
            let vectors: i64 = store.connection.query_row(
                "SELECT COUNT(*) FROM long_memory_embeddings",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(vectors, 0);
            let fts: i64 = store.connection.query_row(
                "SELECT COUNT(*) FROM long_memories_fts WHERE long_memories_fts MATCH 'newbody'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(fts, 1);
            Ok(())
        })
        .unwrap();
}

#[test]
fn regeneration_missing_short_inserts_pending_without_new_long_or_revision() {
    let (runtime, round) = fixture("http://127.0.0.1:9/v1", false);
    let original = snapshot(&runtime, round);
    runtime
        .read(|store, _| {
            let tx = store.connection.transaction()?;
            apply(&tx, &original, &replacement(&original), "b")?;
            tx.commit()?;
            let current = capture(&store.connection, 1, 1, round)?;
            assert_eq!(
                current.short.as_ref().unwrap().integration_state,
                MemoryIntegrationState::Pending
            );
            assert!(current.longs.is_empty());
            assert_eq!(current.revision, original.revision);
            assert_eq!(current.round, original.round);
            Ok(())
        })
        .unwrap();
}

#[test]
fn regeneration_apply_conflicts_before_any_write() {
    for change in [
        "UPDATE sessions SET memory_revision=memory_revision+1 WHERE id=1",
        "UPDATE messages SET expression='新表情' WHERE role='assistant'",
        "UPDATE provider_models SET model='new-model' WHERE purpose='chat' AND provider_id=(SELECT provider_id FROM provider_roles WHERE purpose='chat')",
        "UPDATE settings SET value='90' WHERE key='long_memory_max_chars'",
        "UPDATE short_memories SET content='手动改过来源' WHERE id=9",
        "UPDATE long_memory_sources SET source_order=3 WHERE short_memory_id=3",
        "UPDATE long_memories SET title='手动改过标题' WHERE id=7",
        "DELETE FROM long_memories WHERE id=7",
    ] {
        let (runtime, round) = fixture("http://127.0.0.1:9/v1", true);
        let original = snapshot(&runtime, round);
        runtime
            .read(|store, _| {
                store.connection.execute(change, [])?;
                Ok(())
            })
            .unwrap();
        let before = digest(&runtime, round);
        runtime
            .read(|store, _| {
                let tx = store.connection.transaction()?;
                assert_code(
                    apply(&tx, &original, &replacement(&original), "b"),
                    "MEMORY_CONFLICT",
                );
                // 即使调用方提交冲突后的事务，服务也没有提前写入。
                tx.commit()?;
                Ok(())
            })
            .unwrap();
        assert_eq!(digest(&runtime, round), before);
    }
}

#[test]
fn regeneration_caller_rollback_restores_memories_fts_and_vectors() {
    let (runtime, round) = fixture("http://127.0.0.1:9/v1", true);
    let original = snapshot(&runtime, round);
    let before = digest(&runtime, round);
    runtime
        .read(|store, _| {
            let tx = store.connection.transaction()?;
            apply(&tx, &original, &replacement(&original), "b")?;
            tx.execute(
                "UPDATE messages SET content='新回复' WHERE role='assistant'",
                [],
            )?;
            tx.rollback()?;
            let fts: i64 = store.connection.query_row(
                "SELECT COUNT(*) FROM long_memories_fts WHERE long_memories_fts MATCH 'oldbody'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(fts, 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(digest(&runtime, round), before);
}

#[tokio::test]
async fn regeneration_long_failure_preserves_all_old_data() {
    let server = Server::new(vec![
        Response::json(json!({"content":SHORT})),
        Response::json(json!({"title":"标题", "content":"没有汉字预算", "extra":true})),
    ]);
    let (runtime, round) = fixture(&server.url, true);
    let original = snapshot(&runtime, round);
    let before = digest(&runtime, round);
    assert_code(
        generate(
            &runtime,
            &original,
            &Cancellation::default(),
            "用户原文",
            "新回复",
        )
        .await,
        "MEMORY_INVALID_OUTPUT",
    );
    assert_eq!(digest(&runtime, round), before);
}

#[tokio::test]
async fn regeneration_length_cancel_and_invalid_short_never_replace() {
    let truncated = Response { status:200, mime:"application/json", chunks:vec![(0,serde_json::to_vec(&json!({"choices":[{"message":{"content":json!({"content":SHORT}).to_string()},"finish_reason":"length"}]})).unwrap())] };
    for (response,code) in [
        (truncated,"AI_TRUNCATED"),
        (Response::json(json!({"content":"a","extra":true})),"MEMORY_INVALID_OUTPUT"),
        (Response::json(json!({"content":"  "})),"MEMORY_INVALID_OUTPUT"),
    ] {
        let server = Server::new(vec![response]);
        let (runtime, round) = fixture(&server.url, true);
        let original = snapshot(&runtime, round);
        let before = digest(&runtime, round);
        assert_code(
            generate(
                &runtime,
                &original,
                &Cancellation::default(),
                "用户原文",
                "新回复",
            )
            .await,
            code,
        );
        assert_eq!(digest(&runtime, round), before);
    }
    let (runtime, round) = fixture("http://127.0.0.1:9/v1", true);
    let original = snapshot(&runtime, round);
    let before = digest(&runtime, round);
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_code(
        generate(&runtime, &original, &cancel, "用户原文", "新回复").await,
        "CANCELLED",
    );
    assert_eq!(digest(&runtime, round), before);
}

#[test]
fn regeneration_actual_message_tail_and_deleted_round_conflict() {
    let (runtime, round) = fixture("http://127.0.0.1:9/v1", true);
    let original = snapshot(&runtime, round);
    runtime
        .read(|store, _| {
            crate::sessions::append_proactive_message(
                &mut store.connection,
                1,
                1,
                "主动消息在底部",
                None,
            )?;
            assert_code(capture(&store.connection, 1, 1, round), "ROUND_NOT_TAIL");
            let tx = store.connection.transaction()?;
            assert_code(
                apply(&tx, &original, &replacement(&original), "b"),
                "MEMORY_CONFLICT",
            );
            tx.rollback()?;
            store
                .connection
                .execute("DELETE FROM long_memories WHERE id=7", [])?;
            store
                .connection
                .execute("DELETE FROM chat_rounds WHERE id=?1", [round])?;
            let tx = store.connection.transaction()?;
            assert_code(
                apply(&tx, &original, &replacement(&original), "b"),
                "MEMORY_CONFLICT",
            );
            Ok(())
        })
        .unwrap();
}
