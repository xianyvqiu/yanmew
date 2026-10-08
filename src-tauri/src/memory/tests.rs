use super::*;
use crate::ai::tests::{Response, Server};
use serde_json::json;

const SUMMARY: &str = "用户询问猫的生活习惯，助手说明猫喜欢安静环境并需要规律喂食。";
struct Sink;
impl crate::events::MemoryEventSink for Sink {
    fn memory_changed(&self) {}
}
fn complete(runtime: &Runtime, session_id: i64) -> WorkKey {
    runtime
        .read(|s, _| {
            let round = crate::sessions::start_round(
                &mut s.connection,
                1,
                session_id,
                "猫的生活习惯是什么？",
            )?;
            let round_id = crate::error::parse_id(&round.round.id)?;
            crate::sessions::complete_round(
                &mut s.connection,
                1,
                session_id,
                round_id,
                "猫喜欢安静环境，应当规律喂食。",
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
fn count(runtime: &Runtime, session: i64) -> usize {
    runtime
        .read(|s, _| Ok(list_short(&s.connection, session)?.len()))
        .unwrap()
}

#[test]
fn only_earliest_completed_round_per_session_is_recoverable() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    let first = complete(&runtime, 1);
    let second = complete(&runtime, 1);
    runtime.read(|s, _| {
        let round = crate::sessions::start_round(&mut s.connection, 1, 1, "未完成")?;
        crate::sessions::interrupt_round(&mut s.connection, 1, 1, crate::error::parse_id(&round.round.id)?, Some("部分"), None)?;
        crate::sessions::append_proactive_message(&mut s.connection, 1, 1, "主动问候", None)?;
        s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'另一会话','a','a')", [])?;
        Ok(())
    }).unwrap();
    let other = complete(&runtime, 2);
    let found = runtime.read(|s, _| candidates(&s.connection)).unwrap();
    assert_eq!(
        found.iter().map(|k| k.round_id).collect::<Vec<_>>(),
        vec![first.round_id, other.round_id]
    );
    let (snapshot, lease) = capture(&runtime, first).unwrap().unwrap();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, SUMMARY).unwrap(),
        Outcome::Saved
    );
    let found = runtime.read(|s, _| candidates(&s.connection)).unwrap();
    assert_eq!(found[0].round_id, second.round_id);
    assert_eq!(current_status(&runtime, 1, 1, 0).unwrap().pending_count, 1);
    assert_eq!(
        current_status(&runtime, 1, 2, 0).unwrap_err().code,
        "CONTEXT_CHANGED"
    );
}
#[test]
fn stale_revision_message_provider_and_settings_never_commit() {
    for change in [
        "UPDATE sessions SET memory_revision=memory_revision+1 WHERE id=1",
        "UPDATE messages SET content='编辑后的原文' WHERE role='user'",
        "UPDATE providers SET model='another-model' WHERE is_active=1",
        "UPDATE settings SET value='120' WHERE key='short_memory_max_chars'",
    ] {
        let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
        let key = complete(&runtime, 1);
        let (snapshot, lease) = capture(&runtime, key).unwrap().unwrap();
        runtime
            .read(|s, _| {
                s.connection.execute(change, [])?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            commit(&runtime, &snapshot, &lease, SUMMARY).unwrap(),
            Outcome::Obsolete
        );
        assert_eq!(count(&runtime, 1), 0);
        assert_eq!(
            runtime
                .read(|s, _| Ok(candidates(&s.connection)?.len()))
                .unwrap(),
            1
        );
        drop(lease);
        let (fresh, lease) = capture(&runtime, key).unwrap().unwrap();
        assert_eq!(
            commit(&runtime, &fresh, &lease, SUMMARY).unwrap(),
            Outcome::Saved
        );
    }
}
#[test]
fn cancellation_deleted_source_and_existing_memory_are_preserved() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    let key = complete(&runtime, 1);
    let (snapshot, lease) = capture(&runtime, key).unwrap().unwrap();
    lease.cancel.cancel();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, SUMMARY)
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    assert_eq!(count(&runtime, 1), 0);
    drop(lease);
    let (snapshot, lease) = capture(&runtime, key).unwrap().unwrap();
    runtime.read(|s, _| {
        s.connection.execute("INSERT INTO short_memories(session_id,round_id,content,created_at,updated_at) VALUES(1,?1,'用户手动保存的内容','a','a')", [key.round_id])?;
        Ok(())
    }).unwrap();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, SUMMARY).unwrap(),
        Outcome::AlreadySaved
    );
    assert_eq!(
        runtime
            .read(|s, _| Ok(list_short(&s.connection, 1)?[0].content.clone()))
            .unwrap(),
        "用户手动保存的内容"
    );
    drop(lease);
    let second = complete(&runtime, 1);
    let (snapshot, lease) = capture(&runtime, second).unwrap().unwrap();
    runtime
        .read(|s, _| {
            s.connection
                .execute("DELETE FROM chat_rounds WHERE id=?1", [second.round_id])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, SUMMARY).unwrap(),
        Outcome::Missing
    );
    drop(lease);
    let third = complete(&runtime, 1);
    let (snapshot, lease) = capture(&runtime, third).unwrap().unwrap();
    runtime
        .read(|s, _| {
            s.connection
                .execute("DELETE FROM sessions WHERE id=1", [])?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, SUMMARY).unwrap(),
        Outcome::Missing
    );
    assert_eq!(count(&runtime, 1), 0);
}
#[test]
fn structured_output_and_han_range_are_checked_without_truncation() {
    let config = Configuration {
        provider: None,
        minimum: 12,
        maximum: 100,
        merge_count: 20,
        long_minimum: 150,
        long_maximum: 250,
    };
    assert_eq!(han_count("猫cat🙂，𠀀"), 2);
    assert_eq!(
        parse_output(&json!({"content":SUMMARY}).to_string(), &config).unwrap(),
        SUMMARY
    );
    for bad in [
        json!({"content":"太短"}),
        json!({"content":"汉".repeat(101)}),
        json!({"content":SUMMARY,"extra":1}),
    ] {
        assert_eq!(
            parse_output(&bad.to_string(), &config).unwrap_err().code,
            "MEMORY_INVALID_OUTPUT"
        );
    }
    assert!(parse_output("```json\n{}\n```", &config).is_err());
}
#[test]
fn selected_memory_provider_and_fallback_use_only_source_messages() {
    tauri::async_runtime::block_on(async {
        let chat = Server::new(vec![Response::json(json!({"content":SUMMARY}))]);
        let memory = Server::new(vec![Response::json(json!({"content":SUMMARY}))]);
        let runtime = crate::runtime::test_runtime(&chat.url);
        runtime.read(|s, _| { s.connection.execute("INSERT INTO providers(name,base_url,model,memory_is_active) VALUES('记忆',?1,'memory-model',1)", [&memory.url])?; Ok(()) }).unwrap();
        let first = complete(&runtime, 1);
        assert_eq!(
            generate(runtime.clone(), first).await.unwrap(),
            Outcome::Saved
        );
        assert_eq!(
            generate(runtime.clone(), first).await.unwrap(),
            Outcome::Missing
        );
        assert_eq!(memory.requests.lock().unwrap().len(), 1);
        let sent = memory.requests.lock().unwrap()[0].clone();
        assert_eq!(sent["model"], "memory-model");
        assert!(sent["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains(FACT_RULE));
        assert_eq!(sent["messages"].as_array().unwrap().len(), 2);
        let source: serde_json::Value =
            serde_json::from_str(sent["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(source["user_message"], "猫的生活习惯是什么？");
        assert_eq!(
            source["assistant_message"],
            "猫喜欢安静环境，应当规律喂食。"
        );
        runtime
            .read(|s, _| {
                s.connection
                    .execute("UPDATE providers SET memory_is_active=0", [])?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            generate(runtime.clone(), complete(&runtime, 1))
                .await
                .unwrap(),
            Outcome::Saved
        );
        assert_eq!(count(&runtime, 1), 2);
        memory.finish();
        chat.finish();
    });
}
#[test]
fn length_termination_does_not_save_even_valid_json() {
    tauri::async_runtime::block_on(async {
        let body = json!({"choices":[{"message":{"content":json!({"content":SUMMARY}).to_string()},"finish_reason":"length"}]}).to_string();
        let server = Server::new(vec![Response {
            status: 200,
            mime: "application/json",
            chunks: vec![(0, body.into_bytes())],
        }]);
        let runtime = crate::runtime::test_runtime(&server.url);
        assert_eq!(
            generate(runtime.clone(), complete(&runtime, 1))
                .await
                .unwrap_err()
                .code,
            "AI_TRUNCATED"
        );
        assert_eq!(count(&runtime, 1), 0);
        assert_eq!(current_status(&runtime, 1, 1, 0).unwrap().pending_count, 1);
        server.finish();
    });
}
#[test]
fn failed_attempts_are_bounded_and_explicit_retry_reopens_missing_work() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    let key = complete(&runtime, 1);
    let worker = Worker::default();
    let now = Instant::now();
    for attempt in 1..=3 {
        worker
            .finish(
                &runtime,
                key,
                0,
                Err(AppError::new("PROVIDER_UNAVAILABLE", "模拟失败")),
                now,
            )
            .unwrap();
        assert_ne!(runtime.pet_view().unwrap().activity, crate::pet::controller::Activity::Dormant);
        let queue = runtime.memory.queue.lock().unwrap();
        let failure = &queue.failures[&TaskKey::from(key)];
        assert_eq!(failure.attempts, attempt);
        assert_eq!(
            failure.retry_at,
            RETRY_DELAYS.get(attempt - 1).map(|delay| now + *delay)
        );
    }
    assert!(
        !current_status(&runtime, 1, 1, 0)
            .unwrap()
            .auto_retry_pending
    );
    assert!(retry_current(&runtime, 1, 1, 99).is_err());
    retry_current(&runtime, 1, 1, 0).unwrap();
    assert!(runtime.memory.queue.lock().unwrap().failures.is_empty());
    assert_eq!(current_status(&runtime, 1, 1, 0).unwrap().pending_count, 1);
}
#[test]
fn fresh_worker_recovers_missing_round_after_stale_result() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"content":SUMMARY})),
            Response::json(json!({"content":SUMMARY})),
        ]);
        let runtime = crate::runtime::test_runtime(&server.url);
        let key = complete(&runtime, 1);
        let (snapshot, lease) = capture(&runtime, key).unwrap().unwrap();
        runtime.read(|s, _| { s.connection.execute("UPDATE sessions SET memory_revision=memory_revision+1 WHERE id=1", [])?; s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'其他','a','a')", [])?; Ok(()) }).unwrap();
        assert_eq!(
            commit(&runtime, &snapshot, &lease, SUMMARY).unwrap(),
            Outcome::Obsolete
        );
        drop(lease);
        complete(&runtime, 2);
        // 新工作器不需要旧内存队列，仅凭持久化缺失关系恢复两会话。
        let mut worker = Worker::default();
        tokio::time::timeout(Duration::from_secs(3), async {
            while count(&runtime, 1) != 1 || count(&runtime, 2) != 1 {
                worker.tick(&runtime, &Sink).await.unwrap();
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            worker.tick(&runtime, &Sink).await.unwrap();
        })
        .await
        .unwrap();
        assert_eq!(count(&runtime, 1), 1);
        assert_eq!(count(&runtime, 2), 1);
        server.finish();
    });
}
