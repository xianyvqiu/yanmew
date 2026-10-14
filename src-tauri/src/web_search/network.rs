//! API 地址由用户配置；模型指定的网页必须逐跳解析、校验并固定公开 IP。
use super::{tools::SearchArgs, Kind, Snapshot};
use crate::{
    ai::client::AiClient,
    error::{AppError, AppResult},
    state::Cancellation,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    time::Duration,
};
use url::Url;
const MAX_BYTES: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub title: String,
    pub url: String,
    pub excerpt: String,
    pub published_at: Option<String>,
    pub retrieved_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub title: String,
    pub url: String,
    pub content: String,
    pub retrieved_at: String,
    pub truncated: bool,
}
pub(super) fn limited(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}
fn failed(message: &str) -> AppError {
    AppError::new("WEB_SEARCH_FAILED", message)
}
fn format_error() -> AppError {
    AppError::new("WEB_SEARCH_FORMAT_ERROR", "搜索服务返回的数据格式无效")
}
fn timed(error: AppError) -> AppError {
    if error.code == "AI_TIMEOUT" {
        AppError::new("WEB_SEARCH_TIMEOUT", "搜索或读取网页超时；本次未取得资料")
    } else {
        error
    }
}
fn transport(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::new("WEB_SEARCH_TIMEOUT", "连接搜索服务或网页超时")
    } else {
        failed("无法连接搜索服务或网页，请检查网络和配置")
    }
}
fn status(status: reqwest::StatusCode) -> AppResult<()> {
    if status.is_success() {
        return Ok(());
    }
    Err(match status.as_u16() {
        401 | 403 => AppError::new(
            "WEB_SEARCH_UNAUTHORIZED",
            "服务拒绝访问，请检查 Key、权限或网页限制",
        ),
        429 | 432 | 433 => AppError::new(
            "WEB_SEARCH_QUOTA",
            "服务额度不足或请求过于频繁，请检查服务账户",
        ),
        _ => failed("服务返回错误，本次未取得资料；未自动切换其他服务"),
    })
}
async fn bytes(mut response: reqwest::Response) -> AppResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|n| n > MAX_BYTES as u64)
    {
        return Err(failed("返回内容过大，本次未读取"));
    }
    let mut data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        if data.len() + chunk.len() > MAX_BYTES {
            return Err(failed("返回内容过大，本次未读取"));
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}
async fn post(
    client: &AiClient,
    snapshot: &Snapshot,
    path: &str,
    body: Value,
    cancel: &Cancellation,
) -> AppResult<Value> {
    cancel
        .wait(TIMEOUT, async {
            let response = client
                .search_http()
                .post(format!("{}{path}", snapshot.entry.base_url))
                .bearer_auth(&snapshot.key)
                .json(&body)
                .send()
                .await
                .map_err(transport)?;
            status(response.status())?;
            serde_json::from_slice(&bytes(response).await?).map_err(|_| format_error())
        })
        .await
        .map_err(timed)
}
pub(super) async fn search(
    client: &AiClient,
    snapshot: &Snapshot,
    args: &SearchArgs,
    cancel: &Cancellation,
) -> AppResult<Vec<Source>> {
    let (path, body) = match snapshot.entry.kind {
        Kind::Tavily | Kind::Custom => (
            "/search",
            json!({"query":args.query,"search_depth":"basic","max_results":5,"include_answer":false,"include_raw_content":false,"include_published_date":true,"auto_parameters":false,"topic":args.topic,"time_range":args.time_range}),
        ),
        Kind::Bocha => {
            let freshness = match args.time_range.as_deref() {
                Some("day") => "oneDay",
                Some("week") => "oneWeek",
                Some("month") => "oneMonth",
                Some("year") => "oneYear",
                _ => "noLimit",
            };
            (
                "/v1/web-search",
                json!({"query":args.query,"summary":true,"count":5,"freshness":freshness}),
            )
        }
    };
    let value = post(client, snapshot, path, body, cancel).await?;
    parse_search(snapshot.entry.kind, &value)
}
pub(super) fn parse_search(kind: Kind, value: &Value) -> AppResult<Vec<Source>> {
    let rows = match kind {
        Kind::Tavily | Kind::Custom => {
            if value.get("error").is_some() {
                return Err(failed("Tavily 兼容搜索服务返回错误，请检查服务状态和额度"));
            }
            value.get("results").and_then(Value::as_array)
        }
        Kind::Bocha => {
            if !value
                .get("code")
                .is_some_and(|code| code.as_i64() == Some(200) || code.as_str() == Some("200"))
            {
                return Err(failed("博查返回搜索错误，请检查 Key、服务状态和额度"));
            }
            value
                .pointer("/data/webPages/value")
                .and_then(Value::as_array)
        }
    }
    .ok_or_else(format_error)?;
    let time = chrono::Utc::now().to_rfc3339();
    let mut results = Vec::new();
    for row in rows.iter().take(5) {
        let raw_url = row
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(format_error)?;
        // 不向模型或界面返回不可打开的协议、带凭据地址或明显私有 IP。
        let Ok(url) = page_url(raw_url) else { continue };
        let title = row
            .get(if kind != Kind::Bocha { "title" } else { "name" })
            .and_then(Value::as_str)
            .unwrap_or(raw_url);
        let excerpt = if kind != Kind::Bocha {
            row.get("content")
        } else {
            row.get("summary")
                .filter(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()))
                .or_else(|| row.get("snippet"))
        };
        let date = row
            .get(if kind != Kind::Bocha {
                "published_date"
            } else {
                "datePublished"
            })
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        if !results
            .iter()
            .any(|source: &Source| source.url == url.as_str())
        {
            results.push(Source {
                title: limited(title, 120),
                url: url.to_string(),
                excerpt: limited(excerpt.and_then(Value::as_str).unwrap_or(""), 800),
                published_at: date.map(|s| limited(s, 80)),
                retrieved_at: time.clone(),
            });
        }
    }
    Ok(results)
}
pub(super) fn page_url(raw: &str) -> AppResult<Url> {
    let mut url = Url::parse(raw).map_err(|_| failed("网页地址无效"))?;
    if raw.len() > 2048
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
    {
        return Err(AppError::new(
            "WEB_PAGE_DENIED",
            "只允许读取不带凭据的公开 HTTP/HTTPS 网页",
        ));
    }
    if let Some(host) = url.host_str() {
        if host.eq_ignore_ascii_case("localhost")
            || host.ends_with(".localhost")
            || host.ends_with(".local")
        {
            return Err(AppError::new("WEB_PAGE_DENIED", "不能读取本机或内网网页"));
        }
        if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
            if !public_ip(ip) {
                return Err(AppError::new("WEB_PAGE_DENIED", "不能读取本机或内网网页"));
            }
        }
    }
    url.set_fragment(None);
    Ok(url)
}
pub(crate) fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && (b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(ip));
            }
            let s = ip.segments();
            // 只接受全球单播；排除文档、6to4/Teredo 等特殊转发地址。
            (s[0] & 0xe000) == 0x2000
                && !matches!(s[0], 0x2002 | 0x3fff)
                && !(s[0] == 0x2001 && (s[1] <= 0x01ff || s[1] == 0x0db8))
        }
    }
}
pub(super) async fn open(raw: &str, cancel: &Cancellation) -> AppResult<Page> {
    let url = page_url(raw)?;
    cancel
        .wait(TIMEOUT, read_public(url, cancel))
        .await
        .map_err(timed)
}
async fn read_public(mut url: Url, cancel: &Cancellation) -> AppResult<Page> {
    for hop in 0..=3 {
        cancel.check()?;
        let host = url
            .host_str()
            .ok_or_else(|| failed("网页地址无效"))?
            .trim_matches(['[', ']'])
            .to_string();
        let port = url
            .port_or_known_default()
            .ok_or_else(|| failed("网页端口无效"))?;
        let dns_host = host.clone();
        let addresses = tauri::async_runtime::spawn_blocking(move || {
            (dns_host.as_str(), port)
                .to_socket_addrs()
                .map(|rows| rows.collect::<Vec<SocketAddr>>())
        })
        .await
        .map_err(|_| failed("无法解析网页地址"))?
        .map_err(|_| failed("无法解析网页地址"))?;
        if addresses.is_empty() || addresses.iter().any(|address| !public_ip(address.ip())) {
            return Err(AppError::new(
                "WEB_PAGE_DENIED",
                "网页地址解析到本机、内网或保留地址，已拒绝读取",
            ));
        }
        // 固定已校验的 DNS 结果，禁用代理和自动重定向，防止第二次解析绕过校验。
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10))
            .resolve_to_addrs(&host, &addresses)
            .user_agent("YanMew/0.1 public-page-reader")
            .build()
            .map_err(|_| failed("无法初始化网页读取"))?;
        let response = client
            .get(url.clone())
            .header(reqwest::header::ACCEPT, "text/html,text/plain")
            .send()
            .await
            .map_err(transport)?;
        if response.status().is_redirection() {
            if hop == 3 {
                return Err(failed("网页重定向次数过多"));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| failed("网页重定向地址无效"))?;
            url = page_url(
                url.join(location)
                    .map_err(|_| failed("网页重定向地址无效"))?
                    .as_str(),
            )?;
            continue;
        }
        status(response.status())?;
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !content_type.starts_with("text/html") && !content_type.starts_with("text/plain") {
            return Err(failed("此地址不是可读取的 HTML 或文本网页"));
        }
        let raw = String::from_utf8(bytes(response).await?)
            .map_err(|_| failed("网页不是 UTF-8 编码，本次无法可靠提取文字"))?;
        let html = content_type.starts_with("text/html");
        let (title, content) = tauri::async_runtime::spawn_blocking(move || {
            if html {
                extract_html(&raw)
            } else {
                Ok((String::new(), raw))
            }
        })
        .await
        .map_err(|_| failed("网页解析失败"))??;
        if content.trim().is_empty() {
            return Err(failed(
                "未取得网页正文；可能需要登录或 JavaScript，本工具不会执行",
            ));
        }
        return Ok(Page {
            title: limited(&title, 120),
            url: url.to_string(),
            truncated: content.chars().count() > 6000,
            content: limited(&content, 6000),
            retrieved_at: chrono::Utc::now().to_rfc3339(),
        });
    }
    Err(failed("网页读取失败"))
}
pub(super) fn extract_html(raw: &str) -> AppResult<(String, String)> {
    let html = scraper::Html::parse_document(raw);
    let title_selector = scraper::Selector::parse("title").map_err(|_| format_error())?;
    let title = html
        .select(&title_selector)
        .next()
        .map(|e| e.text().collect::<String>())
        .unwrap_or_default();
    let mut content = String::new();
    for selector in ["main", "article", "body"] {
        let selector = scraper::Selector::parse(selector).map_err(|_| format_error())?;
        let Some(root) = html.select(&selector).next() else {
            continue;
        };
        for node in root.descendants() {
            let Some(text) = node.value().as_text() else {
                continue;
            };
            if node
                .ancestors()
                .filter_map(|n| n.value().as_element())
                .any(|e| {
                    matches!(
                        e.name(),
                        "script"
                            | "style"
                            | "noscript"
                            | "nav"
                            | "footer"
                            | "header"
                            | "aside"
                            | "form"
                    )
                })
            {
                continue;
            }
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                if !content.is_empty() {
                    content.push('\n');
                }
                content.push_str(&text);
            }
        }
        if !content.trim().is_empty() {
            break;
        }
    }
    Ok((title, content))
}
