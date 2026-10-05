use super::*;
fn completed(runtime: &Runtime) -> RoundSnapshot {
    runtime
        .read(|store, _| {
            let round = start_round(&mut store.connection, 1, 1, "旧用户消息")?;
            complete_round(
                &mut store.connection,
                1,
                1,
                crate::error::parse_id(&round.round.id)?,
                "旧回复",
                Some("(^_^)"),
            )
        })
        .unwrap()
}
#[test]
fn edit_keeps_memory_and_rejects_stale_or_empty_input() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1");
    let round = completed(&runtime);
    runtime.read(|store,_|{store.connection.execute("INSERT INTO short_memories(session_id,round_id,content,integration_state,created_at,updated_at) VALUES(1,?1,'原记忆','pending','a','a')",[crate::error::parse_id(&round.round.id)?])?;Ok(())}).unwrap();
    let id = crate::error::parse_id(&round.user_message.id).unwrap();
    let saved = edit(&runtime, 1, 1, 0, id, "旧用户消息", "新用户消息").unwrap();
    assert_eq!(saved.id, round.user_message.id);
    assert_eq!(saved.content, "新用户消息");
    assert_eq!(
        edit(&runtime, 1, 1, 0, id, "旧用户消息", "再次修改")
            .unwrap_err()
            .code,
        "MESSAGE_CONFLICT"
    );
    assert!(edit(&runtime, 1, 1, 0, id, "新用户消息", " ").is_err());
    runtime
        .read(|store, _| {
            assert_eq!(
                store
                    .connection
                    .query_row("SELECT content FROM short_memories", [], |r| r
                        .get::<_, String>(0))?,
                "原记忆"
            );
            Ok(())
        })
        .unwrap();
}
#[test]
fn tail_proactive_only_deletes_itself_and_cannot_skip_over_it() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1");
    let round = completed(&runtime);
    let id=runtime.read(|store,_|{store.connection.execute("INSERT INTO messages(session_id,role,content,created_at) VALUES(1,'assistant','主动消息','a')",[])?;Ok(store.connection.last_insert_rowid())}).unwrap();
    assert_eq!(
        delete_tail(
            &runtime,
            1,
            1,
            0,
            crate::error::parse_id(&round.assistant_message.as_ref().unwrap().id).unwrap(),
            "旧回复"
        )
        .unwrap_err()
        .code,
        "ROUND_NOT_TAIL"
    );
    delete_tail(&runtime, 1, 1, 0, id, "主动消息").unwrap();
    runtime
        .read(|store, _| {
            assert_eq!(last_round(&store.connection, 1, 1)?.unwrap(), round);
            Ok(())
        })
        .unwrap();
}
#[test]
fn pending_and_partial_rounds_delete_together_and_late_reply_cannot_save() {
    for partial in [None, Some("部分回复")] {
        let runtime = crate::runtime::test_runtime("http://127.0.0.1:1");
        let round = runtime
            .read(|store, _| {
                let round = start_round(&mut store.connection, 1, 1, "用户消息")?;
                if partial.is_some() {
                    interrupt_round_with_reason(
                        &mut store.connection,
                        1,
                        1,
                        crate::error::parse_id(&round.round.id)?,
                        partial,
                        None,
                        "user_cancelled",
                    )
                } else {
                    Ok(round)
                }
            })
            .unwrap();
        let target = round
            .assistant_message
            .as_ref()
            .unwrap_or(&round.user_message);
        delete_tail(
            &runtime,
            1,
            1,
            0,
            crate::error::parse_id(&target.id).unwrap(),
            &target.content,
        )
        .unwrap();
        runtime
            .read(|store, _| {
                assert!(complete_round(
                    &mut store.connection,
                    1,
                    1,
                    crate::error::parse_id(&round.round.id)?,
                    "迟到回复",
                    None
                )
                .is_err());
                assert_eq!(
                    store
                        .connection
                        .query_row("SELECT count(*) FROM messages", [], |r| r.get::<_, i64>(0))?,
                    0
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn completed_tail_rolls_back_trigger_and_failure_rolls_back_everything() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1");
    let source = completed(&runtime);
    let target = completed(&runtime);
    runtime.read(|s,_| {
        s.connection.execute("INSERT INTO short_memories(id,session_id,round_id,content,integration_state,created_at,updated_at) VALUES(1,1,?1,'来源','integrated','a','a'),(2,1,?2,'触发轮记忆','pending','a','a')",params![source.round.id.parse::<i64>().unwrap(),target.round.id.parse::<i64>().unwrap()])?;
        s.connection.execute("INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(1,1,'标题','长期事实',1,?1,'a','a')",[target.round.id.parse::<i64>().unwrap()])?;
        s.connection.execute("INSERT INTO long_memory_sources(long_memory_id,short_memory_id,source_order) VALUES(1,1,1)",[])?;
        s.connection.execute_batch("CREATE TRIGGER fail_tail BEFORE DELETE ON chat_rounds BEGIN SELECT RAISE(ABORT,'test rollback'); END;")?;
        Ok(())
    }).unwrap();
    let assistant = target.assistant_message.as_ref().unwrap();
    let id = assistant.id.parse().unwrap();
    assert!(delete_tail(&runtime, 1, 1, 0, id, &assistant.content).is_err());
    runtime
        .read(|s, _| {
            assert_eq!(
                s.connection.query_row(
                    "SELECT integration_state FROM short_memories WHERE id=1",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                "integrated"
            );
            assert_eq!(
                s.connection
                    .query_row("SELECT count(*) FROM long_memories", [], |r| r
                        .get::<_, i64>(0))?,
                1
            );
            s.connection.execute_batch("DROP TRIGGER fail_tail;")?;
            Ok(())
        })
        .unwrap();
    delete_tail(&runtime, 1, 1, 0, id, &assistant.content).unwrap();
    runtime
        .read(|s, _| {
            assert_eq!(
                s.connection.query_row(
                    "SELECT integration_state FROM short_memories WHERE id=1",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                "pending"
            );
            assert_eq!(
                s.connection
                    .query_row("SELECT count(*) FROM long_memories", [], |r| r
                        .get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                s.connection
                    .query_row("SELECT count(*) FROM short_memories", [], |r| r
                        .get::<_, i64>(0))?,
                1
            );
            assert_eq!(last_round(&s.connection, 1, 1)?.unwrap(), source);
            Ok(())
        })
        .unwrap();
}
