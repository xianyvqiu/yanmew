use super::{
    openai_compatible,
    provider::{Choice, Delta, GenerateRequest},
    streaming::{protocol, SseDecoder, StreamAccumulator},
};
use crate::{
    error::{AppError, AppResult},
    providers::ConnectionTest,
    state::Cancellation,
};
use serde::de::DeserializeOwned;
use std::time::Duration;

#[derive(Clone, Copy)]
pub struct Timeouts {
    pub connect: Duration,
    pub first_event: Duration,
    pub idle: Duration,
    pub total: Duration,
}
impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            first_event: Duration::from_secs(30),
            idle: Duration::from_secs(30),
            total: Duration::from_secs(300),
        }
    }
}
#[derive(Clone)]
pub struct AiClient {
    http: reqwest::Client,
    timeouts: Timeouts,
}
fn network() -> AppError {
    AppError::new(
        "PROVIDER_UNAVAILABLE",
        "无法连接模型服务，请检查网络、接口地址和提供商状态",
    )
}
pub(crate) fn transport(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        return AppError::new("AI_TIMEOUT", "连接模型服务超时，请检查网络、代理及提供商状态");
    }
    if error.is_builder() {
        return AppError::new("PROVIDER_UNAVAILABLE", "请求配置无效，请检查接口地址格式，以及 API Key 是否含换行或非法字符");
    }
    // 仅在内存中检查错误链；不把可能携带地址、代理凭据的原文传入 DTO 或日志。
    let message = transport_message(&error);
    AppError::new("PROVIDER_UNAVAILABLE", message)
}
fn transport_message(error: &(dyn std::error::Error + 'static)) -> &'static str {
    let mut source = Some(error);
    let mut dns = false;
    let mut tls = false;
    let mut proxy = false;
    let mut refused = false;
    while let Some(cause) = source {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            dns |= matches!(io.raw_os_error(), Some(11001 | 11002 | 11003 | 11004));
            refused |= io.kind() == std::io::ErrorKind::ConnectionRefused;
        }
        let text = cause.to_string().to_ascii_lowercase();
        dns |= ["dns error", "failed to lookup address", "no such host is known", "name or service not known"].iter().any(|mark| text.contains(mark));
        tls |= ["certificate", "cert_e_", "sec_e_untrusted_root", "sec_e_wrong_principal", "tls handshake", "ssl handshake"].iter().any(|mark| text.contains(mark));
        proxy |= ["proxy authentication", "proxy connect", "tunnel error", "unsuccessful tunnel"].iter().any(|mark| text.contains(mark));
        source = cause.source();
    }
    if proxy { "代理连接失败，请检查代理是否运行、认证是否有效，以及该地址的代理规则" }
    else if tls { "TLS/证书校验或握手失败，请检查系统时间、证书及代理的 HTTPS 设置" }
    else if dns { "域名解析失败，请检查接口域名、DNS 和代理设置" }
    else if refused { "连接被拒绝，请检查接口端口、本地代理是否运行及防火墙设置" }
    else { "无法建立模型服务连接，请检查网络、代理、防火墙及接口地址" }
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[derive(Debug)]
    struct Wrapped(std::io::Error);
    impl std::fmt::Display for Wrapped {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "request failed")
        }
    }
    impl std::error::Error for Wrapped {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.0) }
    }

    #[test]
    fn nested_network_causes_are_classified_without_exposing_private_details() {
        for (cause, expected) in [
            ("dns error: private-api-key", "域名解析失败"),
            ("certificate verify failed: private-api-key", "TLS/证书"),
            ("unsuccessful tunnel: proxy-user:private-api-key", "代理连接失败"),
            ("unrecognized transport: private-api-key", "无法建立模型服务连接"),
        ] {
            let error = Wrapped(std::io::Error::other(cause));
            let message = transport_message(&error);
            assert!(message.contains(expected), "{message}");
            assert!(!message.contains("private-api-key"));
            assert!(!message.contains("proxy-user"));
        }
        assert!(transport_message(&Wrapped(std::io::Error::from_raw_os_error(11001))).contains("域名解析失败"));
    }

    #[test]
    fn real_request_errors_keep_credentials_out_of_feedback() {
        tauri::async_runtime::block_on(async {
            let client = reqwest::Client::builder().no_proxy().build().unwrap();
            let malformed = client.get("http://127.0.0.1/")
                .bearer_auth("private-api-key\r\ninvalid")
                .send().await.unwrap_err();
            let feedback = transport(malformed);
            assert!(feedback.message.contains("请求配置无效"));
            assert!(!feedback.message.contains("private-api-key"));
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            drop(listener);
            let refused = client.get(format!("http://{address}/?secret=private-api-key"))
                .timeout(Duration::from_secs(3)).send().await.unwrap_err();
            let feedback = transport(refused);
            assert!(feedback.message.contains("连接被拒绝"), "{}", feedback.message);
            assert!(!feedback.message.contains("private-api-key"));
            assert!(!feedback.message.contains(&address.to_string()));
        });
    }
}
pub(crate) fn http_failure_message(status: u16) -> String {
    let advice = match status {
        401 => "认证失败，请检查 API Key 是否有效且属于当前服务商",
        402 => "服务商拒绝计费请求，请检查账户余额及额度",
        403 => "访问被拒绝，请检查模型权限、账户或地区限制",
        404 => "接口或模型不存在，请检查 Base URL 路径和模型名称",
        407 => "代理要求认证，请检查代理账号和连接设置",
        429 => "请求受到限制，请检查额度、并发限制或稍后重试",
        500..=599 => "服务端暂时异常，请稍后重试或查看提供商状态",
        300..=399 => "接口发生重定向，程序未自动跟随，请填写服务商给出的最终 API 地址",
        _ => "请求未成功，请检查接口、模型及服务商配置",
    };
    format!("提供商返回 HTTP {status}：{advice}")
}
impl AiClient {
    /// 搜索 API 复用已有连接池；公开网页读取另行校验并固定 DNS 地址。
    pub(crate) fn search_http(&self) -> &reqwest::Client { &self.http }
    pub(crate) fn total_timeout(&self) -> Duration {
        self.timeouts.total
    }
    pub fn new(timeouts: Timeouts) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(timeouts.connect)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(|_| network())?;
        Ok(Self { http, timeouts })
    }
    async fn send(
        &self,
        config: &ConnectionTest,
        input: &GenerateRequest,
        stream: bool,
        cancel: &Cancellation,
    ) -> AppResult<reqwest::Response> {
        cancel.check()?;
        crate::providers::validate_url(&config.base_url)?;
        let mut body = openai_compatible::request_body(&config.model, input, stream)?;
        openai_compatible::apply_message_protocol(&config.base_url, &mut body);
        super::usage::request(&config.base_url, &mut body);
        if crate::chat_capacity::output_parameter(config) == "max_completion_tokens" {
            if let Some(value) = body.as_object_mut().and_then(|v| v.remove("max_tokens")) {
                body["max_completion_tokens"] = value;
            }
        }
        let mut request = self
            .http
            .post(format!(
                "{}/chat/completions",
                config.base_url.trim_end_matches('/')
            ))
            .json(&body);
        if let Some(key) = &config.api_key {
            request = request.bearer_auth(key);
        }
        let response = cancel
            // 非流式服务可能直到整段生成完成才返回响应头，不能套用首 token 时限。
            .wait(if stream { self.timeouts.first_event } else { self.timeouts.total }, async {
                request.send().await.map_err(transport)
            })
            .await?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            // 只从有界错误 JSON 的稳定代码判别容量错误，不展示上游正文或凭据。
            if matches!(status, 400 | 413 | 422) {
                let mut response = response;
                let parsed = cancel.wait(Duration::from_secs(3), async {
                    let mut bytes = Vec::new();
                    while let Some(chunk) = response.chunk().await.map_err(transport)? {
                        if bytes.len() + chunk.len() > 65536 { return Err(protocol()); }
                        bytes.extend_from_slice(&chunk);
                    }
                    Ok(serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                }).await?;
                let code = parsed.as_ref().and_then(|v| v.pointer("/error/code")).and_then(serde_json::Value::as_str);
                if matches!(code, Some("context_length_exceeded" | "context_window_exceeded" | "input_too_long")) {
                    return Err(AppError::new("CONTEXT_TOO_LARGE", "提供商明确返回上下文容量不足，请更换模型或减少本次内容"));
                }
                return Err(AppError::new("PROVIDER_UNAVAILABLE", http_failure_message(status)));
            }
            return Err(AppError::new(
                "PROVIDER_UNAVAILABLE",
                http_failure_message(response.status().as_u16()),
            ));
        }
        Ok(response)
    }
    /// 测试连接成功后可选查询模型列表；失败不改变连接测试的成功结果。
    pub async fn discover_capacity(&self, config: &ConnectionTest) {
        let cancel = Cancellation::default();
        let result: AppResult<serde_json::Value> = cancel.wait(Duration::from_secs(3), async {
            let mut request = self.http.get(format!("{}/models",config.base_url.trim_end_matches('/').trim_end_matches("/chat/completions")));
            if let Some(key)=&config.api_key { request=request.bearer_auth(key); }
            let mut response=request.send().await.map_err(transport)?;
            if !response.status().is_success() {return Err(network());}
            let mut bytes=Vec::new();
            while let Some(chunk)=response.chunk().await.map_err(transport)? {
                if bytes.len()+chunk.len()>1024*1024 {return Err(protocol());}
                bytes.extend_from_slice(&chunk);
            }
            serde_json::from_slice(&bytes).map_err(|_|protocol())
        }).await;
        if let Ok(value)=result {crate::chat_capacity::remember(&config.base_url,&config.model,&value);}
    }
    pub async fn complete(
        &self,
        config: &ConnectionTest,
        input: &GenerateRequest,
        cancel: &Cancellation,
    ) -> AppResult<Choice> {
        cancel.check()?;
        let request = crate::chat_capacity::prepare_request(config, input)?;
        let input = &request;
        cancel
            .wait(self.timeouts.total, async {
                let mut response = self.send(config, input, false, cancel).await?;
                let mut bytes = Vec::new();
                while let Some(chunk) = cancel
                    .wait(self.timeouts.total, async {
                        response.chunk().await.map_err(transport)
                    })
                    .await?
                {
                    if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
                        return Err(protocol());
                    }
                    bytes.extend_from_slice(&chunk);
                }
                cancel.check()?;
                (if input.tools.is_empty(){openai_compatible::complete_response(&bytes)}else{openai_compatible::complete_tool_response(&bytes)}).map_err(|mut error| {
                    if error.code == "AI_TRUNCATED" {
                        error.message = format!(
                            "模型达到本次最大输出上限（{} tokens，可能包含思考内容），尚未生成完整结果；可在提供商高级设置调整最大输出。原数据未修改",
                            input.max_tokens,
                        );
                    }
                    error
                })
            })
            .await
    }
    /// Embedding 复用连接池、取消、无重定向策略和有界响应；不调用聊天模型。
    pub async fn embedding(&self, config: &ConnectionTest, text: &str, cancel: &Cancellation) -> AppResult<Vec<f32>> {
        cancel.wait(Duration::from_secs(30), async {
            crate::providers::validate_url(&config.base_url)?;
            let mut request = self.http.post(format!("{}/embeddings", config.base_url.trim_end_matches('/')))
                .json(&serde_json::json!({"model": config.model, "input": [text], "encoding_format": "float"}));
            if let Some(key) = &config.api_key { request = request.bearer_auth(key); }
            let mut response = request.send().await.map_err(|_| crate::embedding::unavailable("无法连接 Embedding 服务，请检查网络和配置"))?;
            if !response.status().is_success() { return Err(crate::embedding::unavailable(&format!("Embedding 服务返回 HTTP {}，请检查密钥或模型权限", response.status().as_u16()))); }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|_| crate::embedding::unavailable("Embedding 响应读取失败"))? {
                if bytes.len() + chunk.len() > 1024 * 1024 { return Err(crate::embedding::unavailable("Embedding 响应过大")); }
                bytes.extend_from_slice(&chunk);
            }
            #[derive(serde::Deserialize)] struct Response { data: Vec<Item> }
            #[derive(serde::Deserialize)] struct Item { index: usize, embedding: Vec<f32> }
            let parsed: Response = serde_json::from_slice(&bytes).map_err(|_| crate::embedding::unavailable("Embedding 响应格式无效"))?;
            if parsed.data.len() != 1 || parsed.data[0].index != 0 { return Err(crate::embedding::unavailable("Embedding 响应数量或顺序无效")); }
            crate::embedding::normalize(parsed.data.into_iter().next().unwrap().embedding)
        }).await
    }
    pub async fn structured<T: DeserializeOwned>(
        &self,
        config: &ConnectionTest,
        input: &GenerateRequest,
        cancel: &Cancellation,
        validate: impl FnOnce(&T) -> AppResult<()>,
    ) -> AppResult<T> {
        let choice = self.complete(config, input, cancel).await?;
        let output: T = serde_json::from_str(&choice.content).map_err(|_| protocol())?;
        validate(&output)?;
        cancel.check()?;
        Ok(output)
    }
    pub async fn stream(
        &self,
        config: &ConnectionTest,
        input: &GenerateRequest,
        cancel: &Cancellation,
        mut emit: impl FnMut(Delta) -> AppResult<()>,
    ) -> AppResult<Vec<Choice>> {
        cancel.check()?;
        let request = crate::chat_capacity::prepare_request(config, input)?;
        let input = &request;
        cancel
            .wait(self.timeouts.total, async {
                let started = tokio::time::Instant::now();
                let mut response = self.send(config, input, true, cancel).await.map_err(|error| {
                    if error.code == "AI_TIMEOUT" {
                        AppError::new("CHAT_FIRST_RESPONSE_TIMEOUT", "等待模型首个有效响应超时，已停止本轮。")
                    } else { error }
                })?;
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if content_type.split(';').next().unwrap_or("").trim() != "text/event-stream" {
                    return Err(protocol());
                }
                let mut decoder = SseDecoder::default();
                let mut accumulator = StreamAccumulator::default();
                let mut last_progress: Option<tokio::time::Instant> = None;
                let mut bytes = 0usize;
                // SSE 每 token 都可能重复元数据；固定 4 MiB 会误截断正常 32768-token 回复。
                // 按本次输出预算预留每 token 2 KiB 传输开销，仍保留单帧上限和总超时。
                let wire_limit=(input.max_tokens as usize).saturating_mul(2048).max(4*1024*1024);
                loop {
                    let timeout = if let Some(last) = last_progress {
                        self.timeouts.idle.saturating_sub(last.elapsed())
                    } else {
                        self.timeouts.first_event.saturating_sub(started.elapsed())
                    };
                    let stalled = || {
                        if last_progress.is_some() {
                            AppError::new("CHAT_NO_PROGRESS", "模型长时间没有有效输出进展，已停止本轮；已收到的内容仍保留。")
                        } else {
                            AppError::new("CHAT_FIRST_RESPONSE_TIMEOUT", "等待模型首个有效响应超时，已停止本轮。")
                        }
                    };
                    cancel.check()?;
                    if timeout.is_zero() { return Err(stalled()); }
                    let chunk = cancel
                        .wait(timeout, async { response.chunk().await.map_err(transport) })
                        .await.map_err(|error| {
                            if error.code != "AI_TIMEOUT" { return error; }
                            stalled()
                        })?;
                    let Some(chunk) = chunk else {
                        let result=accumulator.finish();
                        if result.as_ref().is_ok_and(|choices|choices.iter().any(|c|c.reasoning_content.as_ref().is_some_and(|s|!s.is_empty()))){crate::native_tools::observed_reasoning(&config.base_url,&config.model);}
                        return result;
                    };
                    bytes += chunk.len();
                    if bytes > wire_limit {
                        return Err(protocol());
                    }
                    for event in decoder.push(&chunk)? {
                        if event.trim().is_empty() {
                            continue;
                        }
                        let deltas = accumulator.push(&event)?;
                        if accumulator.made_progress() {
                            last_progress = Some(tokio::time::Instant::now());
                        }
                        for delta in deltas {
                            cancel.check()?;
                            emit(delta)?;
                        }
                        if accumulator.is_done() {
                            cancel.check()?;
                            let result=accumulator.finish();
                        if result.as_ref().is_ok_and(|choices|choices.iter().any(|c|c.reasoning_content.as_ref().is_some_and(|s|!s.is_empty()))){crate::native_tools::observed_reasoning(&config.base_url,&config.model);}
                        return result;
                        }
                    }
                }
            })
            .await.map_err(|error| {
                if error.code == "AI_TIMEOUT" {
                    AppError::new("CHAT_TOTAL_TIMEOUT", "本次生成已达到总时长上限，已停止接收；已收到的内容仍保留。")
                } else { error }
            })
    }
}
