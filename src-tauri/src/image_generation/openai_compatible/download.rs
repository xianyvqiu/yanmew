//! 所有生图入口共用：本机 DNS 优先，异常时用固定引导地址的 HTTPS DNS 获取真实公网 IP。
use super::{body, public_url, MAX_IMAGE};
use crate::{
    error::{AppError, AppResult},
    state::Cancellation,
};
use serde_json::Value;
use std::{
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    time::Duration,
};

const DNS_LIMIT: usize = 64 * 1024;
const DNS_TIMEOUT: Duration = Duration::from_secs(6);
type Resolver<'a> = (&'a str, &'a str, &'a [&'a str]);
const RESOLVERS: [Resolver<'static>; 2] = [
    (
        "https://dns.alidns.com/resolve",
        "dns.alidns.com",
        &[
            "223.5.5.5",
            "223.6.6.6",
            "2400:3200::1",
            "2400:3200:baba::1",
        ],
    ),
    (
        "https://cloudflare-dns.com/dns-query",
        "cloudflare-dns.com",
        &[
            "1.1.1.1",
            "1.0.0.1",
            "2606:4700:4700::1111",
            "2606:4700:4700::1001",
        ],
    ),
];

fn failed(reason: &str) -> AppError {
    AppError::new(
        "IMAGE_DOWNLOAD_FAILED",
        format!("生图服务已返回图片链接，但{reason}；图片可能已生成并计费，本次未自动重新生图"),
    )
}
fn dns_failed() -> AppError {
    failed("未能取得可验证的公网图片地址，请检查 DNS 或代理网络")
}

pub(super) fn public_ip(ip: IpAddr) -> bool {
    let ip = match ip {
        IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        _ => ip,
    };
    crate::web_search::public_ip(ip)
        && !matches!(ip, IpAddr::V4(v) if v.octets()[..3] == [192, 0, 2])
}
fn public_addresses(addresses: &[SocketAddr]) -> bool {
    !addresses.is_empty() && addresses.iter().all(|address| public_ip(address.ip()))
}
fn name(raw: &str) -> AppResult<String> {
    let value = raw.strip_suffix('.').unwrap_or(raw).to_ascii_lowercase();
    if value.len() > 253
        || !value.contains('.')
        || value.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(dns_failed());
    }
    public_url(&format!("https://{value}/")).map_err(|_| dns_failed())?;
    Ok(value)
}

/// 仅接受本次问题及其 CNAME 链中的地址，忽略无关记录；私有地址、截断和错误应答不作为下载目标。
fn answers(value: &Value, host: &str, kind: u16, port: u16) -> AppResult<Vec<SocketAddr>> {
    if value["Status"].as_u64() != Some(0) || value["TC"].as_bool() == Some(true) {
        return Err(dns_failed());
    }
    let question = match &value["Question"] {
        Value::Array(rows) if rows.len() == 1 => &rows[0],
        Value::Object(_) => &value["Question"],
        _ => return Err(dns_failed()),
    };
    let host = name(host)?;
    if question["type"].as_u64() != Some(kind.into())
        || name(question["name"].as_str().ok_or_else(dns_failed)?)? != host
    {
        return Err(dns_failed());
    }
    let rows = match value.get("Answer") {
        None => return Ok(vec![]),
        Some(Value::Array(rows)) if rows.len() <= 128 => rows,
        _ => return Err(dns_failed()),
    };
    let mut names = vec![host];
    loop {
        let aliases = rows
            .iter()
            .filter(|row| {
                row["type"] == 5
                    && row["name"].as_str().is_some_and(|n| {
                        n.trim_end_matches('.')
                            .eq_ignore_ascii_case(names.last().unwrap())
                    })
            })
            .collect::<Vec<_>>();
        if aliases.is_empty() {
            break;
        }
        let next = name(aliases[0]["data"].as_str().ok_or_else(dns_failed)?)?;
        if names.len() >= 8
            || names.contains(&next)
            || aliases.iter().any(|r| {
                r["data"]
                    .as_str()
                    .is_none_or(|n| !n.trim_end_matches('.').eq_ignore_ascii_case(&next))
            })
        {
            return Err(dns_failed());
        }
        names.push(next);
    }
    let mut addresses = Vec::new();
    for row in rows {
        if row["type"].as_u64() != Some(kind.into()) {
            continue;
        }
        if !row["name"].as_str().is_some_and(|n| {
            names
                .iter()
                .any(|v| n.trim_end_matches('.').eq_ignore_ascii_case(v))
        }) {
            continue;
        }
        let ip: IpAddr = row["data"]
            .as_str()
            .ok_or_else(dns_failed)?
            .parse()
            .map_err(|_| dns_failed())?;
        if !public_ip(ip) || (kind == 1) != ip.is_ipv4() {
            return Err(dns_failed());
        }
        let address = SocketAddr::new(ip, port);
        if !addresses.contains(&address) {
            addresses.push(address);
        }
    }
    Ok(addresses)
}

async fn query(
    client: &reqwest::Client,
    endpoint: &str,
    host: &str,
    kind: u16,
    port: u16,
) -> AppResult<Vec<SocketAddr>> {
    let mut url = url::Url::parse(endpoint).map_err(|_| dns_failed())?;
    url.query_pairs_mut()
        .append_pair("name", host)
        .append_pair("type", &kind.to_string());
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/dns-json")
        .send()
        .await
        .map_err(|_| dns_failed())?;
    if !response.status().is_success() {
        return Err(dns_failed());
    }
    let bytes = body(response, DNS_LIMIT).await.map_err(|_| dns_failed())?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| dns_failed())?;
    answers(&value, host, kind, port)
}

async fn public_dns(
    host: &str,
    port: u16,
    cancel: &Cancellation,
    resolvers: &[Resolver<'_>],
) -> AppResult<Vec<SocketAddr>> {
    let host = name(host)?;
    for &(endpoint, resolver_host, bootstrap) in resolvers {
        cancel.check()?;
        let pins = bootstrap
            .iter()
            .map(|ip| SocketAddr::new(ip.parse().expect("固定 DNS 引导 IP"), 443))
            .collect::<Vec<_>>();
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(3))
            .timeout(DNS_TIMEOUT)
            .resolve_to_addrs(resolver_host, &pins)
            .build()
            .map_err(|_| dns_failed())?;
        let result = cancel
            .wait(DNS_TIMEOUT, async {
                let (v4, v6) = tokio::join!(
                    query(&client, endpoint, &host, 1, port),
                    query(&client, endpoint, &host, 28, port)
                );
                let mut addresses = v4?;
                addresses.extend(v6?);
                if !public_addresses(&addresses) {
                    return Err(dns_failed());
                }
                Ok(addresses)
            })
            .await;
        match result {
            Ok(addresses) => return Ok(addresses),
            Err(error) if error.code == "CANCELLED" => return Err(error),
            Err(_) => continue,
        }
    }
    Err(dns_failed())
}

async fn resolve(host: &str, port: u16, cancel: &Cancellation) -> AppResult<Vec<SocketAddr>> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if public_ip(ip) {
            Ok(vec![SocketAddr::new(ip, port)])
        } else {
            Err(dns_failed())
        };
    }
    let dns = host.to_owned();
    let system = cancel
        .wait(DNS_TIMEOUT, async {
            tauri::async_runtime::spawn_blocking(move || {
                (dns.as_str(), port)
                    .to_socket_addrs()
                    .map(|rows| rows.collect::<Vec<_>>())
            })
            .await
            .map_err(|_| dns_failed())?
            .map_err(|_| dns_failed())
        })
        .await;
    match system {
        Ok(addresses) if public_addresses(&addresses) => Ok(addresses),
        Err(error) if error.code == "CANCELLED" => Err(error),
        _ => public_dns(host, port, cancel, &RESOLVERS).await,
    }
}

/// 每跳固定验证后的真实地址，保留原 HTTPS 主机名/SNI 和证书校验，不接受虚拟 IP、不携带生图 Key。
pub(super) async fn read(raw: &str, cancel: &Cancellation) -> AppResult<Vec<u8>> {
    let mut url = public_url(raw).map_err(|_| failed("图片链接不是允许的公网 HTTPS 地址"))?;
    for hop in 0..=3 {
        cancel.check()?;
        let host = url.host_str().unwrap().trim_matches(['[', ']']).to_string();
        let port = url
            .port_or_known_default()
            .ok_or_else(|| failed("图片端口无效"))?;
        let addresses = resolve(&host, port, cancel).await?;
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
            .header(reqwest::header::ACCEPT, "image/png,image/jpeg,image/webp")
            .send()
            .await
            .map_err(|_| failed("无法连接图片资源"))?;
        if response.status().is_redirection() {
            if hop == 3 {
                return Err(failed("图片重定向次数过多"));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| failed("图片重定向地址无效"))?;
            url = public_url(
                url.join(location)
                    .map_err(|_| failed("图片重定向地址无效"))?
                    .as_str(),
            )
            .map_err(|_| failed("图片重定向目标不是允许的公网 HTTPS 地址"))?;
            continue;
        }
        if !response.status().is_success() {
            return Err(failed(&format!(
                "图片资源读取失败（HTTP {}）",
                response.status().as_u16()
            )));
        }
        return body(response, MAX_IMAGE)
            .await
            .map_err(|_| failed("图片内容读取失败或超过16MiB"));
    }
    Err(failed("图片读取失败"))
}

#[cfg(test)]
mod tests;
