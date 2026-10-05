use super::*;
use crate::ai::tests::{client, event, finish_event, text_event, Response, Server};
use serde_json::{json, Value};

fn runtime(url: &str) -> Runtime {
    let mut runtime = crate::runtime::test_runtime(url);
    // 验证检索降级，不重新加载/基准测试实际模型资源。
    runtime.embedding = Arc::new(crate::embedding::Engine::new(Err(AppError::storage())));
    runtime
}
fn tool_response(name: &str, args: Value) -> Response {
    Response::sse(
        &(event(
            json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"recall-1","type":"function","function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":null}]}),
        ) + &finish_event("tool_calls")),
    )
}
#[test]
fn protected_overflow_sends_no_network_request() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![]);
        let runtime = runtime(&server.url);
        runtime
            .read(|s, _| {
                s.connection.execute(
                    "UPDATE providers SET context_window_override=100 WHERE is_active=1",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let (reply, _) = submit(&runtime, 1, 1, 0, "绝不截断输入".into()).unwrap();
        let saved = run_persisted(&client(), reply, |_| {}).await.unwrap();
        assert_eq!(
            saved.round.status,
            crate::models::ChatRoundStatus::Interrupted
        );
        assert!(saved.assistant_message.is_none());
        assert!(server.requests.lock().unwrap().is_empty());
        server.finish();
    });
}
#[test]
fn memory_tools_page_sources_and_reject_other_session_or_extra_scope_arguments() {
    tauri::async_runtime::block_on(async {
        let runtime = runtime("http://127.0.0.1:1/v1");
        // 标题搜索不携带正文，也不能因为正文超过预取预算而漏掉记录。
        let memory = crate::memory_index::tests::seed(&runtime, 1, "本会话记忆", &"西湖".repeat(1500));
        runtime.read(|s,_| {
            s.connection.execute("INSERT INTO short_memories(id,session_id,round_id,content,created_at,updated_at) VALUES(1,1,1,'本轮事实','a','a')",[])?;
            s.connection.execute("INSERT INTO long_memory_sources(long_memory_id,short_memory_id,source_order) VALUES(?1,1,1)",[memory])?;
            s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'别的','a','a')",[])?;
            Ok(())
        }).unwrap();
        let other = crate::memory_index::tests::seed(&runtime, 2, "他人记忆", "不可查看");
        let reply = prepare_reply(&runtime, 1, 1, 0, "回忆".into()).unwrap();
        let call = |name: &str, args: Value| crate::ai::provider::ToolCall {
            index: 0,
            id: "r".into(),
            name: name.into(),
            arguments: args,
        };
        let result = memory_tools::execute(
            &runtime,
            &reply.lease,
            1,
            1,
            0,
            &call(
                "open_long_memory_sources",
                json!({"memory_id":memory.to_string(),"limit":1}),
            ),
        )
        .await
        .unwrap();
        assert_eq!(result["items"][0]["round_id"], "1");
        assert_eq!(result["items"][0]["content"], "本轮事实");
        let error = memory_tools::execute(
            &runtime,
            &reply.lease,
            1,
            1,
            0,
            &call("open_long_memory", json!({"memory_id":other.to_string()})),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "NOT_FOUND");
        let error = memory_tools::execute(
            &runtime,
            &reply.lease,
            1,
            1,
            0,
            &call(
                "open_long_memory",
                json!({"memory_id":memory.to_string(),"session_id":"2"}),
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code, "VALIDATION_ERROR");
        let result = memory_tools::execute(
            &runtime,
            &reply.lease,
            1,
            1,
            0,
            &call("search_long_memories", json!({"query":"西湖"})),
        )
        .await
        .unwrap();
        assert_eq!(result["items"][0]["memory_id"], memory.to_string());
        assert!(result["items"][0].get("content").is_none());
    });
}
#[test]
fn ordered_context_deduplicates_input_and_short_sources_and_isolates_sessions() {
    let runtime = runtime("http://127.0.0.1:1/v1");
    runtime.read(|s,_| {
        s.connection.execute("INSERT INTO user_profiles(id,name,content,created_at,updated_at) VALUES(1,'用户','用户喜欢茶','a','a')",[])?;
        s.connection.execute("UPDATE sessions SET user_profile_id=1 WHERE id=1",[])?;
        s.connection.execute("INSERT INTO notebooks(session_id,content,updated_at) VALUES(1,'记事本内容','a')",[])?;
        s.connection.execute("UPDATE settings SET value='3' WHERE key='recent_message_count'",[])?;
        let first=crate::sessions::start_round(&mut s.connection,1,1,"第一轮问题")?;
        let id=crate::error::parse_id(&first.round.id)?;
        crate::sessions::complete_round(&mut s.connection,1,1,id,"第一轮回答",None)?;
        s.connection.execute("INSERT INTO short_memories(session_id,round_id,content,created_at,updated_at) VALUES(1,?1,'仍在原文中的短期','a','a')",[id])?;
        s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'别的','a','a')",[])?;
        s.connection.execute("INSERT INTO notebooks(session_id,content,updated_at) VALUES(2,'绝不可出现','a')",[])?;
        Ok(())
    }).unwrap();
    let (reply, _) = submit(&runtime, 1, 1, 0, "本次独有输入".into()).unwrap();
    let (request, budget) = reply.context.clone().request(vec![], &[]).unwrap();
    let contents = request
        .messages
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>();
    assert!(
        contents
            .iter()
            .position(|s| s.contains("用户喜欢茶"))
            .unwrap()
            < contents
                .iter()
                .position(|s| s.contains("记事本内容"))
                .unwrap()
    );
    assert_eq!(contents.iter().filter(|s| **s == "本次独有输入").count(), 1);
    assert!(!contents.join("\n").contains("仍在原文中的短期"));
    assert!(!contents.join("\n").contains("绝不可出现"));
    assert_eq!(budget.effective_max_output_tokens, 32768);
    assert_eq!(contents[contents.len() - 2], "第一轮回答");
}
#[test]
fn known_budget_removes_whole_optional_parts_and_keeps_large_persona_and_tool_pairs() {
    let runtime = runtime("http://127.0.0.1:1/v1");
    runtime
        .read(|s, _| {
            s.connection.execute(
                "UPDATE providers SET context_window_override=90000 WHERE is_active=1",
                [],
            )?;
            s.connection.execute(
                "UPDATE personas SET prompt=?1 WHERE id=1",
                ["人".repeat(20000)],
            )?;
            s.connection.execute(
                "INSERT INTO notebooks(session_id,content,updated_at) VALUES(1,?1,'a')",
                ["旧".repeat(12000)],
            )?;
            Ok(())
        })
        .unwrap();
    let (reply, _) = submit(&runtime, 1, 1, 0, "完整当前输入".into()).unwrap();
    let dynamic = vec![
        Message {
            role: "assistant".into(),
            content: String::new(),
            tool_calls: Some(vec![
                json!({"id":"x","type":"function","function":{"name":"read_notebook","arguments":"{}"}}),
            ]),
            tool_call_id: None,
        },
        Message::tool_result("x", json!({"ok":true})),
    ];
    let (request, budget) = reply.context.clone().request(vec![], &dynamic).unwrap();
    assert!(budget.reduced);
    assert!(request
        .messages
        .iter()
        .any(|m| m.content == "人".repeat(20000)));
    assert!(request.messages.iter().any(|m| m.content == "完整当前输入"));
    assert_eq!(
        request.messages[request.messages.len() - 1]
            .tool_call_id
            .as_deref(),
        Some("x")
    );
    assert!(
        budget.estimated_input_tokens
            + budget.effective_max_output_tokens as usize
            + crate::chat_capacity::SAFETY
            <= 90000
    );
    assert_eq!(
        runtime
            .read(|s, _| Ok(s.connection.query_row(
                "SELECT length(content) FROM notebooks WHERE session_id=1",
                [],
                |r| r.get::<_, i64>(0)
            )?))
            .unwrap(),
        12000
    );
}
#[test]
fn quick_recall_prefetch_adds_older_memory_without_extra_chat_model_call() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&(text_event("旧经历") + &finish_event("stop"))),
        ]);
        let runtime = runtime(&server.url);
        let old = crate::memory_index::tests::seed(
            &runtime,
            1,
            "西湖旅行",
            "曾在杭州西湖游玩，带回桂花糕。",
        );
        crate::memory_index::tests::seed(&runtime, 1, "最近记忆", "昨天讨论计算机。");
        runtime
            .read(|s, _| {
                s.connection.execute(
                    "UPDATE settings SET value='1' WHERE key='recent_long_memory_count'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        let (reply, _) = submit(&runtime, 1, 1, 0, "桂花糕是在什么时候带回的？".into()).unwrap();
        run_persisted(&client(), reply, |_| {}).await.unwrap();
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 2); // 一次表情、一次正文；无聊天 AI 检索判断。
        assert!(requests[1]["messages"]
            .to_string()
            .contains(&format!("混合检索旧长期记忆 {old}")));
        assert!(requests[1]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["function"]["name"] == "open_chat_round"));
        drop(requests);
        server.finish();
    });
}
#[test]
fn deep_recall_pairs_tool_result_and_reads_only_current_session_sources() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            tool_response("open_chat_round", json!({"round_id":"1"})),
            Response::sse(&(text_event("已查原文") + &finish_event("stop"))),
        ]);
        let runtime = runtime(&server.url);
        crate::memory_index::tests::seed(&runtime, 1, "旅行", "西湖");
        let (reply, _) = submit(&runtime, 1, 1, 0, "请查原始回合".into()).unwrap();
        let saved = run_persisted(&client(), reply, |_| {}).await.unwrap();
        assert_eq!(
            saved.round.status,
            crate::models::ChatRoundStatus::Completed
        );
        let requests = server.requests.lock().unwrap();
        let messages = requests[2]["messages"].as_array().unwrap();
        assert_eq!(messages[messages.len() - 1]["role"], "tool");
        assert_eq!(messages[messages.len() - 1]["tool_call_id"], "recall-1");
        assert!(messages[messages.len() - 1]["content"]
            .as_str()
            .unwrap()
            .contains("原始问题"));
        assert!(messages[messages.len() - 2]["tool_calls"].is_array());
        drop(requests);
        server.finish();
    });
}
#[test]
fn explicit_overflow_retries_once_without_deleting_history() {
    tauri::async_runtime::block_on(async {
        let overflow = || Response {
            status: 400,
            mime: "application/json",
            chunks: vec![(
                0,
                serde_json::to_vec(
                    &json!({"error":{"code":"context_length_exceeded","message":"private"}}),
                )
                .unwrap(),
            )],
        };
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            overflow(),
            overflow(),
        ]);
        let runtime = runtime(&server.url);
        crate::memory_index::tests::seed(&runtime, 1, "旧资料", "旧资料正文");
        let (reply, _) = submit(&runtime, 1, 1, 0, "新问题".into()).unwrap();
        let mut notices = vec![];
        let saved = run_persisted(&client(), reply, |e| {
            if let ReplyEventKind::ContextNotice { message, .. } = e.event.kind {
                notices.push(message)
            }
        })
        .await
        .unwrap();
        assert_eq!(
            saved.round.status,
            crate::models::ChatRoundStatus::Interrupted
        );
        assert!(notices.iter().any(|v| v == context::NOTICE));
        assert_eq!(server.requests.lock().unwrap().len(), 3);
        assert_eq!(
            runtime
                .read(|s, _| Ok(s.connection.query_row(
                    "SELECT count(*) FROM long_memories",
                    [],
                    |r| r.get::<_, i64>(0)
                )?))
                .unwrap(),
            1
        );
        server.finish();
    });
}
#[test]
fn length_limit_continues_same_round_and_same_messages_without_tools_or_new_expression() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![
            Response::json(json!({"expression":"(^_^)"})),
            Response::sse(&(text_event("前半 ") + &finish_event("length"))),
            Response::sse(&(text_event("后半") + &finish_event("stop"))),
        ]);
        let runtime = runtime(&server.url);
        let (reply, initial) = submit(&runtime, 1, 1, 0, "长回答".into()).unwrap();
        let stopped = run_persisted(&client(), reply, |_| {}).await.unwrap();
        assert_eq!(stopped.round.stop_reason.as_deref(), Some("length_limit"));
        let old_ai = stopped.assistant_message.unwrap();
        let (reply, next) =
            continue_reply(&runtime, 1, 1, 0, initial.round_id.parse().unwrap()).unwrap();
        assert_ne!(initial.request_id, next.request_id);
        assert_eq!(next.partial_content, "前半 ");
        let completed = run_persisted(&client(), reply, |_| {}).await.unwrap();
        assert_eq!(completed.round.id, initial.round_id);
        assert_eq!(
            completed.round.status,
            crate::models::ChatRoundStatus::Completed
        );
        let ai = completed.assistant_message.unwrap();
        assert_eq!(ai.id, old_ai.id);
        assert_eq!(ai.content, "前半 后半");
        assert!(completed.round.stop_reason.is_none());
        assert_eq!(
            runtime
                .read(|s, _| Ok(s.connection.query_row(
                    "SELECT count(*) FROM messages",
                    [],
                    |r| r.get::<_, i64>(0)
                )?))
                .unwrap(),
            2
        );
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[2].get("tools").is_none());
        assert!(requests[2]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["role"] == "assistant" && v["content"] == "前半 "));
        drop(requests);
        server.finish();
    });
}
