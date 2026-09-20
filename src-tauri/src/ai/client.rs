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
        let body = openai_compatible::request_body(&config.model, input, stream)?;
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
                let mut response = self.send(config, input, true, cancel).await?;
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
                let mut received_event = false;
                let mut bytes = 0usize;
                loop {
                    let timeout = if received_event {
                        self.timeouts.idle
                    } else {
                        self.timeouts.first_event.saturating_sub(started.elapsed())
                    };
                    let chunk = cancel
                        .wait(timeout, async { response.chunk().await.map_err(transport) })
                        .await?;
                    let Some(chunk) = chunk else {
                        return accumulator.finish();
                    };
                    bytes += chunk.len();
                    if bytes > 4 * 1024 * 1024 {
                        return Err(protocol());
                    }
                    for event in decoder.push(&chunk)? {
                        if event.trim().is_empty() {
                            continue;
                        }
                        received_event = true;
                        for delta in accumulator.push(&event)? {
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
            .await
    }
}
