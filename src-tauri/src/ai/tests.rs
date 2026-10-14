use super::{
    client::{AiClient, Timeouts},
    provider::{Finish, GenerateRequest, Message},
    streaming::{SseDecoder, StreamAccumulator},
};
use crate::{error::AppResult, providers::ConnectionTest, state::Cancellation};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(crate) struct Response {
    pub status: u16,
    pub mime: &'static str,
    pub chunks: Vec<(u64, Vec<u8>)>,
}
impl Response {
    pub fn json(content: Value) -> Self {
        Self{status:200,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&json!({"choices":[{"message":{"content":content.to_string()},"finish_reason":"stop"}]})).unwrap())]}
    }
    pub fn sse(content: &str) -> Self {
        Self {
            status: 200,
            mime: "text/event-stream",
            chunks: vec![(0, content.as_bytes().to_vec())],
        }
    }
}
pub(crate) struct Server {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Value>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    pub fn new(responses: Vec<Response>) -> Self {
        Self::new_at(responses,"/v1/chat/completions")
    }
    pub fn new_at(responses: Vec<Response>,path:&str) -> Self {
        let expected=format!("post {path} ");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(vec![]));
        let received = requests.clone();
        let worker = std::thread::spawn(move || {
            for response in responses {
                let started = Instant::now();
                let mut stream = loop {
                    match listener.accept() {
                        Ok((s, _)) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                started.elapsed() < Duration::from_secs(5),
                                "未收到预期测试请求"
                            );
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                let header_end = loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buffer[..n]);
                    if let Some(i) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                        break i + 4;
                    }
                    assert!(request.len() < 65536);
                };
                let header = std::str::from_utf8(&request[..header_end])
                    .unwrap()
                    .to_ascii_lowercase();
                assert!(header.starts_with(&expected));
                let len: usize = header
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                while request.len() < header_end + len {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buffer[..n]);
                }
                received
                    .lock()
                    .unwrap()
                    .push(serde_json::from_slice(&request[header_end..header_end + len]).unwrap());
                if write!(stream,"HTTP/1.1 {} Test\r\nContent-Type: {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",response.status,response.mime).is_err(){continue;}
                for (delay, chunk) in response.chunks {
                    std::thread::sleep(Duration::from_millis(delay));
                    let header = format!("{:x}\r\n", chunk.len());
                    if stream
                        .write_all(header.as_bytes())
                        .and_then(|_| stream.write_all(&chunk))
                        .and_then(|_| stream.write_all(b"\r\n"))
                        .is_err()
                    {
                        break;
                    }
                }
                let _ = stream.write_all(b"0\r\n\r\n");
            }
        });
        Self {
            url,
            requests,
            worker: Some(worker),
        }
    }
    pub fn config(&self) -> ConnectionTest {
        ConnectionTest {
            base_url: self.url.clone(),
            model: "fake-model".into(),
            api_key: None,
        }
    }
    pub fn finish(mut self) {
        self.worker.take().unwrap().join().unwrap();
    }
}
pub(crate) fn event(value: Value) -> String {
    format!("data: {}\r\n\r\n", value)
}
pub(crate) fn text_event(content: &str) -> String {
    event(json!({"choices":[{"index":0,"delta":{"content":content},"finish_reason":null}]}))
}
pub(crate) fn finish_event(reason: &str) -> String {
    event(json!({"choices":[{"index":0,"delta":{},"finish_reason":reason}]}))
        + "data: [DONE]\r\n\r\n"
}
#[test]
fn configured_long_output_accepts_sse_metadata_above_old_four_mib_wire_limit() {
    tauri::async_runtime::block_on(async {
        let frame=event(json!({"model":"x".repeat(512),"choices":[{"index":0,"delta":{"content":"好"},"finish_reason":null}]}));
        let wire=frame.repeat(9000)+&finish_event("stop");
        assert!(wire.len()>4*1024*1024);
        let server=Server::new(vec![Response::sse(&wire)]);
        let request=GenerateRequest{messages:vec![Message::new("user","长回复")],max_tokens:32768,tools:vec![],json:false};
        let result=client().stream(&server.config(),&request,&Cancellation::default(),|_|Ok(())).await.unwrap();
        assert_eq!(result[0].content.chars().count(),9000);assert_eq!(result[0].finish,Finish::Stop);
        server.finish();
    });
}
fn input() -> GenerateRequest {
    GenerateRequest {
        messages: vec![Message::new("user", "测试")],
        max_tokens: 1024,
        tools: vec![],
        json: false,
    }
}
pub(crate) fn client() -> AiClient {
    AiClient::new(Timeouts {
        connect: Duration::from_secs(1),
        first_event: Duration::from_millis(500),
        idle: Duration::from_millis(500),
        total: Duration::from_secs(3),
    })
    .unwrap()
}

#[test]
fn arbitrary_utf8_splits_multiline_and_heartbeat() {
    let source = ": heartbeat\r\n\r\n".to_owned()
        + &text_event("你好，喵🐈")
        + "data: {\"choices\":\n"
        + "data: [{\"index\":0,\"delta\":{\"content\":\"世界\"},\"finish_reason\":null}]}\n\n"
        + &finish_event("stop");
    for split in 0..=source.len() {
        let mut decoder = SseDecoder::default();
        let mut state = StreamAccumulator::default();
        for chunk in [&source.as_bytes()[..split], &source.as_bytes()[split..]] {
            for data in decoder.push(chunk).unwrap() {
                state.push(&data).unwrap();
            }
        }
        assert_eq!(state.finish().unwrap()[0].content, "你好，喵🐈世界");
    }
    let mut decoder = SseDecoder::default();
    let mut state = StreamAccumulator::default();
    for byte in source.bytes() {
        for data in decoder.push(&[byte]).unwrap() {
            state.push(&data).unwrap();
        }
    }
    assert_eq!(state.finish().unwrap()[0].content, "你好，喵🐈世界");
}
#[test]
fn tool_arguments_only_become_calls_after_valid_termination() {
    let mut state = StreamAccumulator::default();
    for value in [
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"id":"b","type":"function","function":{"name":"search","arguments":"{\"query\":"}},{"index":0,"id":"a","type":"function","function":{"name":"open","arguments":"{\"id\":"}}]}}]}),
        json!({"choices":[{"index":1,"delta":{"content":"另一个 choice"}},{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"1}"}},{"index":1,"function":{"arguments":"\"往事\"}"}}]}}]}),
        json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"},{"index":1,"delta":{},"finish_reason":"stop"}]}),
    ] {
        state.push(&value.to_string()).unwrap();
        assert!(!state.is_done());
    }
    state.push("[DONE]").unwrap();
    let result = state.finish().unwrap();
    assert_eq!(result[0].tools[0].id, "a");
    assert_eq!(result[0].tools[1].arguments, json!({"query":"往事"}));
    assert_eq!(result[1].content, "另一个 choice");
    let mut bad = StreamAccumulator::default();
    bad.push(r#"{"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#)
        .unwrap();
    bad.push("[DONE]").unwrap();
    assert!(bad.finish().is_err());
    let mut truncated = StreamAccumulator::default();
    truncated.push(r#"{"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{"}}]},"finish_reason":"length"}]}"#).unwrap();
    truncated.push("[DONE]").unwrap();
    let result = truncated.finish().unwrap();
    assert_eq!(result[0].finish, Finish::Length);
    assert!(result[0].tools.is_empty());
}
#[test]
fn http_stream_eof_errors_and_status_are_not_success() {
    tauri::async_runtime::block_on(async {
        let data = text_event("中文不丢字") + &finish_event("stop");
        let server = Server::new(vec![Response {
            status: 200,
            mime: "text/event-stream",
            chunks: data.bytes().map(|b| (0, vec![b])).collect(),
        }]);
        let mut received = String::new();
        let result = client()
            .stream(
                &server.config(),
                &input(),
                &Cancellation::default(),
                |delta| {
                    let super::provider::Delta::Text { text, .. } = delta;
                    received.push_str(&text);
                    Ok(())
                },
            )
            .await
            .unwrap();
        assert_eq!(received, "中文不丢字");
        assert_eq!(result[0].content, received);
        server.finish();
        let server = Server::new(vec![Response::sse(&text_event("只收到一半"))]);
        assert_eq!(
            client()
                .stream(
                    &server.config(),
                    &input(),
                    &Cancellation::default(),
                    |_| Ok(())
                )
                .await
                .unwrap_err()
                .code,
            "AI_STREAM_INTERRUPTED"
        );
        server.finish();
        for status in [401, 429, 500] {
            let server = Server::new(vec![Response {
                status,
                mime: "application/json",
                chunks: vec![(0, b"private key and upstream details".to_vec())],
            }]);
            let error = client()
                .complete(&server.config(), &input(), &Cancellation::default())
                .await
                .unwrap_err();
            assert_eq!(error.code, "PROVIDER_UNAVAILABLE");
            assert!(!error.message.contains("private"));
            assert_eq!(server.requests.lock().unwrap().len(), 1);
            server.finish();
        }
    });
}
#[test]
fn timeout_and_cancellation_drop_late_chunks() {
    tauri::async_runtime::block_on(async {
        let small = AiClient::new(Timeouts {
            connect: Duration::from_secs(1),
            first_event: Duration::from_millis(50),
            idle: Duration::from_millis(50),
            total: Duration::from_secs(1),
        })
        .unwrap();
        let server = Server::new(vec![Response {
            status: 200,
            mime: "text/event-stream",
            chunks: vec![
                (0, b": ping\n\n".to_vec()),
                (100, text_event("太迟").into_bytes()),
            ],
        }]);
        assert_eq!(
            small
                .stream(
                    &server.config(),
                    &input(),
                    &Cancellation::default(),
                    |_| Ok(())
                )
                .await
                .unwrap_err()
                .code,
            "CHAT_FIRST_RESPONSE_TIMEOUT"
        );
        server.finish();
        let server = Server::new(vec![Response {
            status: 200,
            mime: "text/event-stream",
            chunks: vec![
                (0, text_event("第一段").into_bytes()),
                (
                    100,
                    (text_event("迟到") + &finish_event("stop")).into_bytes(),
                ),
            ],
        }]);
        let cancel = Cancellation::default();
        let mut calls = 0;
        let error = client()
            .stream(&server.config(), &input(), &cancel, |_| {
                calls += 1;
                cancel.cancel();
                Ok(())
            })
            .await
            .unwrap_err();
        assert_eq!(error.code, "CANCELLED");
        assert_eq!(calls, 1);
        server.finish();
        let server = Server::new(vec![Response {
            status: 200,
            mime: "text/event-stream",
            chunks: vec![
                (0, text_event("开头").into_bytes()),
                (100, finish_event("stop").into_bytes()),
            ],
        }]);
        assert_eq!(
            small
                .stream(
                    &server.config(),
                    &input(),
                    &Cancellation::default(),
                    |_| Ok(())
                )
                .await
                .unwrap_err()
                .code,
            "CHAT_NO_PROGRESS"
        );
        server.finish();
    });
}
#[test]
fn structured_output_validates_finish_json_and_schema() {
    tauri::async_runtime::block_on(async {
        for body in [
            json!({"choices":[{"message":{"content":"{\"count\":1}"},"finish_reason":"length"}]}),
            json!({"choices":[{"message":{"content":"not-json"},"finish_reason":"stop"}]}),
            json!({"choices":[{"message":{"content":"{\"count\":0}"},"finish_reason":"stop"}]}),
        ] {
            let server = Server::new(vec![Response {
                status: 200,
                mime: "application/json",
                chunks: vec![(0, serde_json::to_vec(&body).unwrap())],
            }]);
            let result: AppResult<Value> = client()
                .structured(
                    &server.config(),
                    &input(),
                    &Cancellation::default(),
                    |v: &Value| {
                        if v["count"] == 1 {
                            Ok(())
                        } else {
                            Err(super::streaming::protocol())
                        }
                    },
                )
                .await;
            assert!(result.is_err());
            server.finish();
        }
    });
}
