use super::tests::{event, finish_event, text_event, Response, Server};
use super::{
    client::{AiClient, Timeouts},
    provider::{Finish, GenerateRequest, Message},
    streaming::{SseDecoder, StreamAccumulator},
};
use crate::state::Cancellation;
use serde_json::json;
use std::time::Duration;

fn input() -> GenerateRequest {
    GenerateRequest {
        messages: vec![Message::new("user", "测试")],
        max_tokens: 1024,
        tools: vec![],
        json: false,
    }
}
fn timed_client() -> AiClient {
    AiClient::new(Timeouts {
        connect: Duration::from_secs(1),
        first_event: Duration::from_millis(200),
        idle: Duration::from_millis(150),
        total: Duration::from_secs(2),
    })
    .unwrap()
}

#[test]
fn progress_requires_recognized_nonempty_content_reasoning_or_arguments() {
    let mut accumulator = StreamAccumulator::default();
    for (delta, expected) in [
        (json!({"role":"assistant"}), false),
        (json!({"content":""}), false),
        (json!({"unrecognized":"metadata"}), false),
        (json!({"reasoning_content":"正在思考"}), true),
        (json!({"reasoning":"另一种推理字段"}), true),
        (json!({"content":"你好"}), true),
        (
            json!({"tool_calls":[{"index":0,"id":"one","type":"function","function":{"name":"lookup"}}]}),
            false,
        ),
        (
            json!({"tool_calls":[{"index":0,"function":{"arguments":"{}"}}]}),
            true,
        ),
        (json!({}), false),
    ] {
        accumulator
            .push(&json!({"choices":[{"index":0,"delta":delta,"finish_reason":null}]}).to_string())
            .unwrap();
        assert_eq!(accumulator.made_progress(), expected);
    }
    let mut decoder = SseDecoder::default();
    assert_eq!(
        decoder.push(b"data: \xff\n\n").unwrap_err().code,
        "CHAT_INVALID_ENCODING"
    );
}

#[test]
fn heartbeats_role_and_empty_deltas_cannot_extend_first_or_idle_deadline() {
    tauri::async_runtime::block_on(async {
        for has_text in [false, true] {
            let mut chunks = vec![(
                0,
                if has_text {
                    text_event("开头")
                } else {
                    event(json!({"choices":[{"index":0,"delta":{"role":"assistant"}}]}))
                }
                .into_bytes(),
            )];
            for _ in 0..12 {
                chunks.push((
                    30,
                    (": ping\r\n\r\n".to_owned() + &text_event("") + &event(json!({"choices":[]})))
                        .into_bytes(),
                ));
            }
            chunks.push((0, finish_event("stop").into_bytes()));
            let server = Server::new(vec![Response {
                status: 200,
                mime: "text/event-stream",
                chunks,
            }]);
            let error = timed_client()
                .stream(&server.config(), &input(), &Cancellation::default(), |_| {
                    Ok(())
                })
                .await
                .unwrap_err();
            assert_eq!(
                error.code,
                if has_text {
                    "CHAT_NO_PROGRESS"
                } else {
                    "CHAT_FIRST_RESPONSE_TIMEOUT"
                }
            );
            server.finish();
        }
    });
}

#[test]
fn reasoning_and_fragmented_tool_arguments_keep_stream_alive_without_exposing_reasoning() {
    tauri::async_runtime::block_on(async {
        for field in ["reasoning_content", "reasoning", "arguments"] {
            let mut chunks = vec![];
            if field == "arguments" {
                chunks.push((0,event(json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"one","type":"function","function":{"name":"lookup","arguments":"{\"query\":\""}}]}}]})).into_bytes()));
            }
            for _ in 0..6 {
                let delta = if field == "arguments" {
                    json!({"tool_calls":[{"index":0,"function":{"arguments":"字"}}]})
                } else {
                    json!({field:"有效推理增量"})
                };
                chunks.push((
                    60,
                    event(json!({"choices":[{"index":0,"delta":delta}]})).into_bytes(),
                ));
            }
            let ending = if field == "arguments" {
                event(
                    json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"}"}}]}}]}),
                ) + &finish_event("tool_calls")
            } else {
                text_event("最终回答") + &finish_event("stop")
            };
            chunks.push((0, ending.into_bytes()));
            let server = Server::new(vec![Response {
                status: 200,
                mime: "text/event-stream",
                chunks,
            }]);
            let mut visible = String::new();
            let choices = timed_client()
                .stream(
                    &server.config(),
                    &input(),
                    &Cancellation::default(),
                    |delta| {
                        let super::provider::Delta::Text { text, .. } = delta;
                        visible.push_str(&text);
                        Ok(())
                    },
                )
                .await
                .unwrap();
            if field == "arguments" {
                assert_eq!(choices[0].finish, Finish::ToolCalls);
                assert_eq!(choices[0].tools[0].arguments["query"], "字".repeat(6));
                assert!(visible.is_empty());
            } else {
                assert_eq!(visible, "最终回答");
            }
            server.finish();
        }
    });
}
