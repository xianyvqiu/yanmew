use super::*;
use crate::ai::tests::{client, finish_event, text_event, Response, Server};
use serde_json::json;

fn fixture(url: &str) -> (Runtime, RoundSnapshot) {
    let runtime = crate::runtime::test_runtime(url);
    let round=runtime.read(|s,_| {
        let round=crate::sessions::start_round(&mut s.connection,1,1,"测试用户只出现一次")?;
        let saved=crate::sessions::complete_round(&mut s.connection,1,1,round.round.id.parse().unwrap(),"原来的完整回复",Some("(^_^)"))?;
        s.connection.execute("INSERT INTO short_memories(session_id,round_id,content,integration_state,created_at,updated_at) VALUES(1,?1,'原来的短期记忆','pending','a','a')",[saved.round.id.parse::<i64>().unwrap()])?;
        Ok(saved)
    }).unwrap();
    (runtime, round)
}
#[test]
fn successful_regeneration_preserves_ids_reward_and_excludes_old_reply() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(o_o)"})),
            Response::sse(&(text_event("新的完整回复") + &finish_event("stop"))),
            Response::json(json!({"content":"用户提出测试问题角色给出了新的完整回复"})),
        ]);
        let (runtime, old) = fixture(&server.url);
        let (reply, receipt, memory) =
            regenerate(&runtime, 1, 1, 0, old.round.id.parse().unwrap()).unwrap();
        assert!(receipt.regenerating);
        assert!(submit(&runtime, 1, 1, 0, "重复发送".into()).is_err());
        let mut events = vec![];
        let saved = run_regenerated(&client(), reply, memory, |event| events.push(event))
            .await
            .unwrap();
        assert_eq!(saved.round, old.round);
        assert_eq!(saved.user_message, old.user_message);
        assert_eq!(
            saved.assistant_message.as_ref().unwrap().id,
            old.assistant_message.as_ref().unwrap().id
        );
        assert_eq!(
            saved.assistant_message.as_ref().unwrap().content,
            "新的完整回复"
        );
        assert_eq!(events.iter().filter(|e| e.saved.is_some()).count(), 1);
        runtime
            .read(|s, _| {
                assert_eq!(
                    s.connection.query_row(
                        "SELECT intimacy FROM sessions WHERE id=1",
                        [],
                        |r| r.get::<_, i64>(0)
                    )?,
                    1
                );
                assert_eq!(
                    s.connection
                        .query_row("SELECT content FROM short_memories", [], |r| r
                            .get::<_, String>(0))?,
                    "用户提出测试问题角色给出了新的完整回复"
                );
                assert_eq!(
                    s.connection
                        .query_row("SELECT count(*) FROM messages", [], |r| r.get::<_, i64>(0))?,
                    2
                );
                assert!(s.chat_requests.is_empty());
                Ok(())
            })
            .unwrap();
        let requests = server.requests.lock().unwrap();
        let messages = requests[1]["messages"].as_array().unwrap();
        assert_eq!(
            messages
                .iter()
                .filter(|m| m["role"] == "user" && m["content"] == "测试用户只出现一次")
                .count(),
            1
        );
        assert!(!messages.iter().any(|m| m["content"] == "原来的完整回复"));
        let tools = requests[1]["tools"].as_array().unwrap();
        assert!(!tools.iter().any(|t| matches!(
            t["function"]["name"].as_str(),
            Some("update_notebook" | "create_reminder")
        )));
        drop(requests);
        server.finish();
    });
}
#[test]
fn memory_failure_keeps_original_reply_and_memory() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(o_o)"})),
            Response::sse(&(text_event("新的完整回复") + &finish_event("stop"))),
            Response::json(json!({"content":"太短"})),
        ]);
        let (runtime, old) = fixture(&server.url);
        let (reply, _, memory) =
            regenerate(&runtime, 1, 1, 0, old.round.id.parse().unwrap()).unwrap();
        let mut events = vec![];
        assert!(
            run_regenerated(&client(), reply, memory, |event| events.push(event))
                .await
                .is_err()
        );
        assert_eq!(events.last().unwrap().saved.as_ref(), Some(&old));
        runtime
            .read(|s, _| {
                assert_eq!(crate::sessions::last_round(&s.connection, 1, 1)?, Some(old));
                assert_eq!(
                    s.connection
                        .query_row("SELECT content FROM short_memories", [], |r| r
                            .get::<_, String>(0))?,
                    "原来的短期记忆"
                );
                Ok(())
            })
            .unwrap();
        server.finish();
    });
}
#[test]
fn cancelled_regeneration_and_shutdown_never_save_preview() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(json!({"expression":"(o_o)"}))]);
        let (runtime, old) = fixture(&server.url);
        let (reply, _, memory) =
            regenerate(&runtime, 1, 1, 0, old.round.id.parse().unwrap()).unwrap();
        assert!(run_regenerated(&client(), reply, memory, |event| {
            if matches!(event.event.kind, ReplyEventKind::Expression { .. }) {
                interrupt_active(&runtime).unwrap();
            }
        })
        .await
        .is_err());
        runtime
            .read(|s, _| {
                assert_eq!(crate::sessions::last_round(&s.connection, 1, 1)?, Some(old));
                assert!(s.chat_requests.is_empty());
                Ok(())
            })
            .unwrap();
        server.finish();
    });
}
