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
        .get("data").or_else(||value.get("images"))
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
        let response=client.search_http().post(endpoint(&snapshot.provider.base_url,"generations")).bearer_auth(&snapshot.key)
            .json(&generation_body(snapshot,&prompt,true))
            .send().await.map_err(network)?;
        if !response.status().is_success(){return Err(failed("生图服务拒绝或未完成请求，请检查模型、Key、额度及透明输出支持；本次不自动重试"));}
        let value=serde_json::from_slice(&body(response,MAX_RESPONSE).await?).map_err(|_|failed("生图响应格式无效"))?;
        parse(value,cancel).await
    }).await.map_err(|error|if error.code=="AI_TIMEOUT"{AppError::new("IMAGE_GENERATION_TIMEOUT","生图或图片读取超时，本次不自动重试；服务商可能已经计费")}else{error})
}
#[cfg(test)]
mod tests;

/// 单张参考图编辑；手工构造标准multipart，沿用现有HTTP/超时/响应解码，无额外依赖。
pub(crate) async fn edit(client:&AiClient,snapshot:&Snapshot,image:&[u8],requirement:&str,cancel:&Cancellation)->AppResult<Vec<u8>> {
 let boundary=format!("yanmew-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|_|AppError::storage())?.as_nanos());
 let prompt=format!("保持参考图同一角色、造型、配色和画风，制作单独的桌宠动作/表情：{requirement}。主体完整，保持相近大小和画布位置，真正透明背景，四边留透明空间，不要文字、水印、棋盘格、底板或场景。输出透明PNG。");
 let mut payload=Vec::new();
 for (key,value) in [("model",snapshot.provider.model.as_str()),("prompt",prompt.as_str()),("n","1"),("size","1024x1024"),("background","transparent"),("output_format","png")] {payload.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{key}\"\r\n\r\n{value}\r\n").as_bytes());}
 payload.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"reference.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());payload.extend_from_slice(image);payload.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
 cancel.wait(TIMEOUT,async{cancel.check()?;let response=client.search_http().post(endpoint(&snapshot.provider.base_url,"edits")).bearer_auth(&snapshot.key).header(reqwest::header::CONTENT_TYPE,format!("multipart/form-data; boundary={boundary}")).body(payload).send().await.map_err(network)?;
 if !response.status().is_success(){return Err(failed("参考图生成失败：请确认所选生图服务和模型支持图像编辑及透明输出，检查Key和额度；不会自动付费重试"));}let value=serde_json::from_slice(&body(response,MAX_RESPONSE).await?).map_err(|_|failed("参考图生成响应格式无效"))?;parse(value,cancel).await}).await
}

/// 根地址与已包含完整路径的地址均可用，不改写用户连接记录。
fn endpoint(base:&str,action:&str)->String {
 let base=base.trim_end_matches('/');
 let root=base.strip_suffix("/images/generations").or_else(||base.strip_suffix("/images/edits")).or_else(||base.strip_suffix("/chat/completions")).unwrap_or(base);
 let root=if siliconflow(root)&&url::Url::parse(root).is_ok_and(|u|u.path().is_empty()||u.path()=="/"){format!("{root}/v1")}else{root.into()};
 format!("{root}/images/{action}")
}
fn siliconflow(base:&str)->bool {url::Url::parse(base).ok().and_then(|u|u.host_str().map(str::to_owned)).is_some_and(|h|matches!(h.as_str(),"api.siliconflow.cn"|"api.siliconflow.com"|"api-st.siliconflow.cn"))}
fn generation_body(snapshot:&Snapshot,prompt:&str,transparent:bool)->Value {
 if siliconflow(&snapshot.provider.base_url) {json!({"model":snapshot.provider.model,"prompt":prompt,"image_size":"1024x1024"})}
 else {let mut body=json!({"model":snapshot.provider.model,"prompt":prompt,"n":1,"size":"1024x1024"});if transparent{body["background"]=json!("transparent");body["output_format"]=json!("png");}body}
}
/// 聊天中按用户要求生成普通图片，不套用饰品或透明底提示词。
pub(crate) async fn generate_picture(client:&AiClient,snapshot:&Snapshot,prompt:&str,cancel:&Cancellation)->AppResult<Vec<u8>> {
 cancel.wait(TIMEOUT,async {
  cancel.check()?;
  let response=client.search_http().post(endpoint(&snapshot.provider.base_url,"generations")).bearer_auth(&snapshot.key).json(&generation_body(snapshot,prompt,false)).send().await.map_err(network)?;
  if !response.status().is_success(){return Err(failed(&format!("生图请求失败（HTTP {}），请检查生图用途的模型、额度和服务配置；未自动重试",response.status().as_u16())));}
  let value=serde_json::from_slice(&body(response,MAX_RESPONSE).await?).map_err(|_|failed("生图响应格式无效"))?;parse(value,cancel).await
 }).await.map_err(|e|if e.code=="AI_TIMEOUT"{AppError::new("IMAGE_GENERATION_TIMEOUT","聊天生图超时，服务商可能已经计费；本次没有自动重试")}else{e})
}
