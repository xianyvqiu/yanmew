use super::*;
use crate::ai::client::{AiClient, Timeouts};
use crate::ai::tests::{client, event, finish_event, text_event, Response, Server};
use serde_json::json;
use std::{sync::Arc, time::Duration};

const LOOP: &str = "这是一段没有任何新增事实的重复叙事。";

#[test]
fn conservative_text_samples_preserve_long_normal_text_symbols_and_code() {
    let normal: String = (0..240)
        .map(|n| {
            format!(
                "第{n}项记录了不同的日期与数值：{}。English 日本語 한국어 ( •ω• ) 😀 ∑x²\n",
                n * n
            )
        })
        .collect();
    let examples = [
        (10..70).map(|n| format!("{n}. {}\n", "该编号对应一个独立的处理项目，下面按相同结构详细记录处理过程。".repeat(5))).collect::<String>(),
        "hello ".repeat(600),
        "extraordinary! ".repeat(300),
        normal,
        "( •ω• ) (╯°□°）╯︵ ┻━┻ 哈哈哈！\n".repeat(300),
        format!("```text\n{}\n```\n已完成。", LOOP.repeat(300)),
        "let value = calculate(number);\n".repeat(300),
        format!(
            "{}{}",
            LOOP.repeat(56),
            (0..100)
                .map(|n| format!("这里是新事实{n}及新的进展{}。", n * n))
                .collect::<String>()
        ),
    ];
    for text in examples {
        let mut guard = TextGuard::default();
        // 字符逐个到达还可验证跨分片的代码围栏。
        for character in text.chars() {
            guard.observe(&character.to_string()).unwrap();
        }
    }
}

#[test]
fn repeated_text_and_encoding_require_sustained_evidence_independent_of_chunks() {
    for width in [1, 97, 4096] {
        let characters: Vec<_> = LOOP.repeat(200).chars().collect();
        let mut guard = TextGuard::default();
        let error = characters
            .chunks(width)
            .find_map(|part| guard.observe(&part.iter().collect::<String>()).err())
            .unwrap();
        assert_eq!(error.code, "CHAT_REPETITION");
    }
    let mut guard = TextGuard::default();
    guard
        .observe("这是偶尔讨论替换字符 � 的正常句子。\n\t😀")
        .unwrap();
    let error = guard.observe(&"\u{fffd}\u{0001}".repeat(300)).unwrap_err();
    assert_eq!(error.code, "CHAT_INVALID_ENCODING");
}

#[test]
fn tool_guard_compares_arguments_and_results_without_rejecting_different_valid_calls() {
    let round = |query, answer| {
        vec![(
            "search_long_memories".into(),
            json!({"query":query}),
            json!({"result":answer}),
        )]
    };
    let mut guard = ToolGuard::default();
    guard.observe(round("猫", 1)).unwrap();
    guard.observe(round("猫", 1)).unwrap();
    guard.observe(round("旅行", 1)).unwrap();
    guard.observe(round("旅行", 2)).unwrap();
    guard.observe(round("旅行", 2)).unwrap();
    assert_eq!(
        guard.observe(round("旅行", 2)).unwrap_err().code,
        "CHAT_TOOL_NO_PROGRESS"
    );
}

fn runtime(url: &str) -> crate::runtime::Runtime {
    let mut runtime = crate::runtime::test_runtime(url);
    runtime.embedding = Arc::new(crate::embedding::Engine::new(Err(AppError::storage())));
    runtime
}

#[test]
fn output_guards_preserve_one_interrupted_reply_without_reward_or_provider_dormancy() {
    tauri::async_runtime::block_on(async {
        for (text, reason) in [
            (LOOP.repeat(200), "repetition"),
            ("\u{fffd}".repeat(600), "invalid_encoding"),
        ] {
            let server = Server::new(vec![
                Response::json(json!({"expression":"(^_^)"})),
                Response::sse(
                    &(text_event("已收到的开头。") + &text_event(&text) + &finish_event("stop")),
                ),
            ]);
            let runtime = runtime(&server.url);
            let (reply, _) = crate::chat::submit(&runtime, 1, 1, 0, "请继续讲述".into()).unwrap();
            let mut terminal = 0;
            let saved = crate::chat::run_persisted(&client(), reply, |event| {
                if event.saved.is_some() {
                    terminal += 1;
                }
            })
            .await
            .unwrap();
            assert_eq!(saved.round.stop_reason.as_deref(), Some(reason));
            assert_eq!(
                saved.round.status,
                crate::models::ChatRoundStatus::Interrupted
            );
            assert!(saved
                .assistant_message
                .unwrap()
                .content
                .starts_with("已收到的开头。"));
            assert_eq!(terminal, 1);
            runtime
                .read(|store, _| {
                    store.pet.require_ai()?;
                    assert_eq!(
                        store.connection.query_row(
                            "SELECT intimacy FROM sessions WHERE id=1",
                            [],
                            |r| r.get::<_, i64>(0)
                        )?,
                        0
                    );
                    assert_eq!(
                        store.connection.query_row(
                            "SELECT COUNT(*) FROM messages WHERE role='assistant'",
                            [],
                            |r| r.get::<_, i64>(0)
                        )?,
                        1
                    );
                    assert_eq!(
                        store.connection.query_row(
                            "SELECT COUNT(*) FROM chat_rounds WHERE status='completed'",
                            [],
                            |r| r.get::<_, i64>(0)
                        )?,
                        0
                    );
                    Ok(())
                })
                .unwrap();
            assert!(crate::chat::current(&runtime, 1, 1, 0).unwrap().is_none());
            server.finish();
        }
    });
}

fn tool(id: &str, name: &str, args: Value) -> Response {
    Response::sse(
        &(event(
            json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":null}]}),
        ) + &finish_event("tool_calls")),
    )
}

#[test]
fn repeating_tools_stop_and_keep_successful_notebook_write() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            tool("initial-read", "read_notebook", json!({})),
            tool(
                "write-1",
                "update_notebook",
                json!({"operation":"append","section_index":null,"content":"已成功记录的事实"}),
            ),
            tool("read-1", "read_notebook", json!({})),
            tool("read-2", "read_notebook", json!({})),
            tool("read-3", "read_notebook", json!({})),
        ]);
        let runtime = runtime(&server.url);
        runtime.read(|store, _| {
            store.connection.execute("INSERT INTO settings(key,value) VALUES('tool_call_rounds','6') ON CONFLICT(key) DO UPDATE SET value=excluded.value", [])?;
            Ok(())
        }).unwrap();
        let (reply, _) = crate::chat::submit(&runtime, 1, 1, 0, "请记下来再查看".into()).unwrap();
        let mut failure = String::new();
        let saved = crate::chat::run_persisted(&client(), reply, |event| {
            if let crate::chat::ReplyEventKind::Failed { code, .. } = event.event.kind {
                failure = code;
            }
        })
        .await
        .unwrap();
        assert_eq!(failure, "CHAT_TOOL_NO_PROGRESS");
        assert_eq!(saved.round.stop_reason.as_deref(), Some("tool_limit"));
        runtime
            .read(|store, _| {
                store.pet.require_ai()?;
                let content: String = store.connection.query_row(
                    "SELECT content FROM notebooks WHERE session_id=1",
                    [],
                    |r| r.get(0),
                )?;
                assert_eq!(content, "已成功记录的事实");
                assert_eq!(
                    store.connection.query_row(
                        "SELECT intimacy FROM sessions WHERE id=1",
                        [],
                        |r| r.get::<_, i64>(0)
                    )?,
                    0
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(server.requests.lock().unwrap().len(), 6);
        server.finish();
    });
}

#[test]
fn whole_round_deadline_includes_expression_and_keeps_already_received_text() {
    tauri::async_runtime::block_on(async {
        let mut expression = Response::json(json!({"expression":"(^_^)"}));
        expression.chunks[0].0 = 250;
        let server = Server::new(vec![
            expression,
            Response {
                status: 200,
                mime: "text/event-stream",
                chunks: vec![
                    (250, text_event("这段正文应保留").into_bytes()),
                    (250, finish_event("stop").into_bytes()),
                ],
            },
        ]);
        let runtime = runtime(&server.url);
        let client = AiClient::new(Timeouts {
            connect: Duration::from_secs(1),
            first_event: Duration::from_millis(400),
            idle: Duration::from_millis(400),
            total: Duration::from_millis(650),
        })
        .unwrap();
        let (reply, _) = crate::chat::submit(&runtime, 1, 1, 0, "你好".into()).unwrap();
        let saved = crate::chat::run_persisted(&client, reply, |_| {})
            .await
            .unwrap();
        assert_eq!(saved.round.stop_reason.as_deref(), Some("timeout"));
        assert_eq!(saved.assistant_message.unwrap().content, "这段正文应保留");
        runtime.read(|store, _| store.pet.require_ai()).unwrap();
        server.finish();
    });
}
