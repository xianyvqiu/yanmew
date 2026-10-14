//! 首版图像适配：用户显式配置的API可访问；响应里的图片URL必须是公开HTTPS。
use super::Snapshot;
use crate::{
    ai::client::AiClient,
    error::{AppError, AppResult},
    state::Cancellation,
};
use base64::Engine;
use serde_json::{json, Value};
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    time::Duration,
};
use url::Url;
const TIMEOUT: Duration = Duration::from_secs(120);
const MAX_IMAGE: usize = crate::accessories::images::MAX_BYTES;
const MAX_RESPONSE: usize = MAX_IMAGE.div_ceil(3) * 4 + 64 * 1024;
fn failed(message: &str) -> AppError {
    AppError::new("IMAGE_GENERATION_FAILED", message)
}
fn network(_: reqwest::Error) -> AppError {
    failed("生图服务或图片资源读取失败；本次不自动重试")
}
async fn body(mut response: reqwest::Response, limit: usize) -> AppResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|size| size > limit as u64)
    {
        return Err(failed("生图返回内容超过安全上限"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(network)? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(failed("生图返回内容超过安全上限"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
fn public_url(raw: &str) -> AppResult<Url> {
    let url = Url::parse(raw).map_err(|_| failed("生图服务返回的图片地址无效"))?;
    if raw.len() > 2048
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(failed("图片资源只允许无账号的公开HTTPS地址"));
    }
    let host = url.host_str().unwrap().trim_matches(['[', ']']);
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|ip| !crate::web_search::public_ip(ip))
    {
        return Err(failed("图片地址指向本机、内网或保留地址，已拒绝读取"));
    }
    Ok(url)
}
/// 沿用公开网页读取的IP规则，逐跳重新解析并固定地址，防止DNS重绑定和代理绕过。
async fn download(raw: &str, cancel: &Cancellation) -> AppResult<Vec<u8>> {
    let mut url = public_url(raw)?;
    for hop in 0..=3 {
        cancel.check()?;
        let host = url.host_str().unwrap().trim_matches(['[', ']']).to_string();
        let port = url
            .port_or_known_default()
            .ok_or_else(|| failed("图片端口无效"))?;
        let dns = host.clone();
        let addresses = tauri::async_runtime::spawn_blocking(move || {
            (dns.as_str(), port)
                .to_socket_addrs()
                .map(|rows| rows.collect::<Vec<SocketAddr>>())
        })
        .await
        .map_err(|_| failed("图片地址解析失败"))?
        .map_err(|_| failed("图片地址解析失败"))?;
        if addresses.is_empty()
            || addresses
                .iter()
                .any(|address| !crate::web_search::public_ip(address.ip()))
        {
            return Err(failed("图片地址解析到本机、内网或保留地址，已拒绝读取"));
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .resolve_to_addrs(&host, &addresses)
            .build()
            .map_err(|_| failed("图片读取初始化失败"))?;
        let response = client
            .get(url.clone())
            .header(reqwest::header::ACCEPT, "image/png,image/webp")
            .send()
            .await
            .map_err(network)?;
        if response.status().is_redirection() {
            if hop == 3 {
                return Err(failed("图片重定向次数过多"));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| failed("图片重定向无效"))?;
            url = public_url(
                url.join(location)
                    .map_err(|_| failed("图片重定向无效"))?
                    .as_str(),
            )?;
            continue;
        }
        if !response.status().is_success() {
            return Err(failed("图片资源未成功返回"));
        }
        return body(response, MAX_IMAGE).await;
    }
    Err(failed("图片读取失败"))
}
async fn parse(value: Value, cancel: &Cancellation) -> AppResult<Vec<u8>> {
    let first = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .ok_or_else(|| failed("生图服务未返回有效图片"))?;
    if let Some(encoded) = first.get("b64_json").and_then(Value::as_str) {
        if encoded.len() > MAX_IMAGE.div_ceil(3) * 4 {
            return Err(failed("图片超过16MiB"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|_| failed("图片编码无效"))?;
        if bytes.is_empty() || bytes.len() > MAX_IMAGE {
            return Err(failed("图片为空或超过16MiB"));
        }
        return Ok(bytes);
    }
    if let Some(url) = first.get("url").and_then(Value::as_str) {
        return download(url, cancel).await;
    }
    Err(failed("生图服务未返回有效图片"))
}
pub(crate) async fn generate(
    client: &AiClient,
    snapshot: &Snapshot,
    name: &str,
    description: &str,
    cancel: &Cancellation,
) -> AppResult<Vec<u8>> {
    let prompt=format!("绘制一个用于颜文字桌宠头顶的单独饰品。名称：{name}。设计细节：{description}。真正透明背景，四边留透明空间，只有饰品本身，主体居中，清晰小尺寸可读；不要绘制宠物、人物、脸或整个场景，不要文字、水印、棋盘格、阴影底板。输出透明PNG。");
    cancel.wait(TIMEOUT,async {
        cancel.check()?;
        let response=client.search_http().post(format!("{}/images/generations",snapshot.provider.base_url)).bearer_auth(&snapshot.key)
            .json(&json!({"model":snapshot.provider.model,"prompt":prompt,"n":1,"size":"1024x1024","background":"transparent","output_format":"png"}))
            .send().await.map_err(network)?;
        if !response.status().is_success(){return Err(failed("生图服务拒绝或未完成请求，请检查模型、Key、额度及透明输出支持；本次不自动重试"));}
        let value=serde_json::from_slice(&body(response,MAX_RESPONSE).await?).map_err(|_|failed("生图响应格式无效"))?;
        parse(value,cancel).await
    }).await.map_err(|error|if error.code=="AI_TIMEOUT"{AppError::new("IMAGE_GENERATION_TIMEOUT","生图或图片读取超时，本次不自动重试；服务商可能已经计费")}else{error})
}
#[cfg(test)]
mod tests;
