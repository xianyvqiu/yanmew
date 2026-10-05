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
fn transport(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::new("AI_TIMEOUT", "连接模型服务超时，请检查网络或提供商状态")
    } else {
        network()
    }
}
impl AiClient {
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
            .wait(self.timeouts.first_event, async {
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
                return Err(AppError::new("PROVIDER_UNAVAILABLE", format!("提供商返回 HTTP {status}，请检查密钥、模型权限或稍后重试")));
            }
            return Err(AppError::new(
                "PROVIDER_UNAVAILABLE",
                format!(
                    "提供商返回 HTTP {}，请检查密钥、模型权限或稍后重试",
                    response.status().as_u16()
                ),
            ));
        }
        Ok(response)
    }
    /// 测试连接成功后可选查询模型列表；失败不改变连接测试的成功结果。
    pub async fn discover_capacity(&self, config: &ConnectionTest) {
        let cancel = Cancellation::default();
        let result: AppResult<serde_json::Value> = cancel.wait(Duration::from_secs(3), async {
            let mut request = self.http.get(format!("{}/models",config.base_url.trim_end_matches('/')));
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
        cancel
            .wait(self.timeouts.total, async {
                let mut response = self.send(config, input, false, cancel).await?;
                let mut bytes = Vec::new();
                while let Some(chunk) = cancel
                    .wait(self.timeouts.idle, async {
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
                openai_compatible::complete_response(&bytes)
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
                        return accumulator.finish();
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
                            return accumulator.finish();
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
