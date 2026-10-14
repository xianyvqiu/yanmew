use super::*;
use reqwest::{
    header::{HeaderName, HeaderValue},
    Method,
};
use std::{
    collections::BTreeMap,
    net::{SocketAddr, ToSocketAddrs},
};
use url::Url;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub url: String,
    #[serde(default = "get")]
    pub method: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
}
fn get() -> String {
    "GET".into()
}
const MAX_BODY: usize = 1024 * 1024;
fn failed() -> AppError {
    AppError::new(
        "PLUGIN_NETWORK_FAILED",
        "插件网络请求失败、超时或内容超过限制",
    )
}
fn url(raw: &str) -> AppResult<Url> {
    let u = Url::parse(raw).map_err(|_| plugins::denied())?;
    if raw.len() > 4096
        || u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err(plugins::denied());
    }
    Ok(u)
}
pub fn checked_addresses(rows: Vec<SocketAddr>) -> AppResult<Vec<SocketAddr>> {
    if rows.is_empty() || rows.iter().any(|a| !crate::web_search::public_ip(a.ip())) {
        return Err(plugins::denied());
    }
    Ok(rows)
}
pub fn headers(input: &BTreeMap<String, String>) -> AppResult<reqwest::header::HeaderMap> {
    if input.len() > 32 {
        return Err(plugins::denied());
    }
    let mut out = reqwest::header::HeaderMap::new();
    let mut size = 0;
    for (k, v) in input {
        size += k.len() + v.len();
        if size > 16 * 1024 {
            return Err(plugins::denied());
        }
        let name = HeaderName::from_bytes(k.as_bytes()).map_err(|_| plugins::denied())?;
        if [
            "host",
            "cookie",
            "cookie2",
            "proxy-authorization",
            "proxy-connection",
            "connection",
            "content-length",
            "transfer-encoding",
            "te",
            "trailer",
            "upgrade",
            "accept-encoding",
            "forwarded",
        ]
        .contains(&name.as_str())
            || name.as_str().starts_with("sec-")
            || name.as_str().starts_with("proxy-")
            || name.as_str().starts_with("x-forwarded-")
        {
            return Err(plugins::denied());
        }
        out.insert(
            name,
            HeaderValue::from_str(v).map_err(|_| plugins::denied())?,
        );
    }
    Ok(out)
}
fn authorize(runtime: &Runtime, i: &Instance, u: &Url) -> AppResult<()> {
    i.cancel.check()?;
    runtime.read(|s, _| {
        let m = &i.package.manifest;
        let p = plugins::authorize(&s.connection, &m.id, &m.version)?;
        let o = u.origin().ascii_serialization();
        if !p.network_origins.contains(&o)
            || !plugins::allowed(&s.connection, &m.id, &m.version, "network.fetch", &o)?
        {
            return Err(plugins::denied());
        }
        Ok(())
    })
}
pub async fn fetch(runtime: &Runtime, i: Arc<Instance>, r: Request) -> AppResult<Value> {
    let permit = i
        .network_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::new("PLUGIN_NETWORK_BUSY", "插件同时联网请求不能超过 4 个"))?;
    let result=i.cancel.wait(Duration::from_secs(30),async{
        let mut u=url(&r.url)?;let mut method=Method::from_bytes(r.method.as_bytes()).map_err(|_|plugins::denied())?;
        if ![Method::GET,Method::POST,Method::PUT,Method::PATCH,Method::DELETE,Method::HEAD,Method::OPTIONS].contains(&method)||r.body.as_ref().is_some_and(|s|s.len()>MAX_BODY){return Err(plugins::denied())}
        let mut h=headers(&r.headers)?;let mut body=r.body;
        for hop in 0..=3{
            authorize(runtime,&i,&u)?;let host=u.host_str().ok_or_else(plugins::denied)?.trim_matches(['[',']']).to_owned();let port=u.port_or_known_default().ok_or_else(plugins::denied)?;let dns=host.clone();
            let addresses=tauri::async_runtime::spawn_blocking(move||(dns.as_str(),port).to_socket_addrs().map(|s|s.collect::<Vec<_>>())).await.map_err(|_|failed())?.map_err(|_|failed())?;let addresses=checked_addresses(addresses)?;
            let client=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).retry(reqwest::retry::never()).connect_timeout(Duration::from_secs(10)).resolve_to_addrs(&host,&addresses).build().map_err(|_|failed())?;
            let mut req=client.request(method.clone(),u.clone()).headers(h.clone());if let Some(b)=&body{req=req.body(b.clone())}let mut response=req.send().await.map_err(|_|failed())?;
            authorize(runtime,&i,&u)?;
            if matches!(response.status().as_u16(),301|302|303|307|308){if hop==3{return Err(failed())}let location=response.headers().get(reqwest::header::LOCATION).and_then(|s|s.to_str().ok()).ok_or_else(failed)?;let next=url(u.join(location).map_err(|_|failed())?.as_str())?;authorize(runtime,&i,&next)?;
                if next.origin()!=u.origin(){if body.is_some()&&response.status().as_u16()!=303{return Err(plugins::denied())}h=reqwest::header::HeaderMap::new();}
                if response.status().as_u16()==303||(matches!(response.status().as_u16(),301|302)&&method==Method::POST){method=Method::GET;body=None;h.remove(reqwest::header::CONTENT_TYPE);}u=next;continue}
            if response.content_length().is_some_and(|n|n>MAX_BODY as u64){return Err(failed())}
            let status=response.status().as_u16();let response_headers:BTreeMap<String,String>=response.headers().iter().filter(|(k,_)|!["set-cookie","set-cookie2","content-length","content-encoding"].contains(&k.as_str())).filter_map(|(k,v)|v.to_str().ok().map(|v|(k.as_str().into(),v.into()))).collect();
            let mut bytes=vec![];while let Some(chunk)=response.chunk().await.map_err(|_|failed())?{i.cancel.check()?;if bytes.len()+chunk.len()>MAX_BODY{return Err(failed())}bytes.extend_from_slice(&chunk)}authorize(runtime,&i,&u)?;
            use base64::Engine;return Ok(json!({"url":u.as_str(),"status":status,"headers":response_headers,"bodyBase64":base64::engine::general_purpose::STANDARD.encode(bytes)}));
        }Err(failed())
    }).await;
    drop(permit);
    result.map_err(|e| if e.code == "AI_TIMEOUT" { failed() } else { e })
}
