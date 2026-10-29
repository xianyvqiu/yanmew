//! 官方生图协议映射。只按已知官方域名识别，不按模型名猜测第三方网关。
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Gemini,
    Dashscope,
    DashscopeCompatible,
    Ark,
    OpenRouter,
}
pub(crate) fn detect(base: &str) -> Option<Kind> {
    let url = Url::parse(base).ok()?;
    let host = url.host_str()?.trim_end_matches('.');
    match host {
        "generativelanguage.googleapis.com" => Some(Kind::Gemini),
        "dashscope.aliyuncs.com" | "dashscope-intl.aliyuncs.com" | "dashscope-us.aliyuncs.com" => {
            Some(if url.path().contains("/compatible-mode") {
                Kind::DashscopeCompatible
            } else {
                Kind::Dashscope
            })
        }
        "ark.cn-beijing.volces.com" | "ark.ap-southeast.bytepluses.com" => Some(Kind::Ark),
        "openrouter.ai" => Some(Kind::OpenRouter),
        _ if host.ends_with(".maas.aliyuncs.com") => {
            Some(if url.path().contains("/compatible-mode") {
                Kind::DashscopeCompatible
            } else {
                Kind::Dashscope
            })
        }
        _ => None,
    }
}
fn url_at(base: &str, path: &str) -> AppResult<Url> {
    let mut url = Url::parse(base).map_err(|_| failed("生图服务地址无效"))?;
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}
fn gemini_version(base: &str) -> &'static str {
    if Url::parse(base).is_ok_and(|u| u.path() == "/v1" || u.path().starts_with("/v1/")) {
        "v1"
    } else {
        "v1beta"
    }
}
fn target(kind: Kind, snapshot: &Snapshot) -> AppResult<Url> {
    let base = &snapshot.provider.base_url;
    let path = match kind {
        Kind::Gemini => {
            let model = snapshot
                .provider
                .model
                .strip_prefix("models/")
                .unwrap_or(&snapshot.provider.model);
            if model.is_empty()
                || !model
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
            {
                return Err(failed("Gemini生图模型名称无效，请填写模型ID而非请求地址"));
            }
            format!("/{}/models/{model}:generateContent", gemini_version(base))
        }
        Kind::Dashscope => "/api/v1/services/aigc/multimodal-generation/generation".into(),
        Kind::DashscopeCompatible => "/compatible-mode/v1/images/generations".into(),
        Kind::Ark => "/api/v3/images/generations".into(),
        Kind::OpenRouter => "/api/v1/images".into(),
    };
    url_at(base, &path)
}
pub(crate) fn catalog(base: &str) -> Option<AppResult<(String, bool)>> {
    let kind = detect(base)?;
    let path = match kind {
        Kind::Gemini => format!("/{}/models", gemini_version(base)),
        Kind::Dashscope | Kind::DashscopeCompatible => "/compatible-mode/v1/models".into(),
        Kind::Ark => "/api/v3/models".into(),
        Kind::OpenRouter => "/api/v1/images/models".into(),
    };
    Some(url_at(base, &path).map(|url| (url.into(), kind == Kind::Gemini)))
}
pub(super) fn request(
    client: &AiClient,
    snapshot: &Snapshot,
    prompt: &str,
    image: Option<&[u8]>,
) -> AppResult<Option<reqwest::RequestBuilder>> {
    detect(&snapshot.provider.base_url)
        .map(|kind| request_for(kind, client, snapshot, prompt, image))
        .transpose()
}
fn request_for(
    mut kind: Kind,
    client: &AiClient,
    snapshot: &Snapshot,
    prompt: &str,
    image: Option<&[u8]>,
) -> AppResult<reqwest::RequestBuilder> {
    let model = &snapshot.provider.model;
    // 旧Qwen编辑模型走已公布的原生同步协议，同一官方域名内切换路径。
    if kind == Kind::DashscopeCompatible && model.starts_with("qwen-image-edit") {
        kind = Kind::Dashscope;
    }
    let data_url = image.map(|bytes| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    });
    let data = match kind {
        Kind::Gemini => {
            let mut parts = vec![json!({"text":prompt})];
            if let Some(bytes) = image {
                parts.push(json!({"inlineData":{"mimeType":"image/png","data":base64::engine::general_purpose::STANDARD.encode(bytes)}}));
            }
            json!({"contents":[{"role":"user","parts":parts}],"generationConfig":{"responseModalities":["TEXT","IMAGE"],"candidateCount":1}})
        }
        Kind::Dashscope => {
            let mut content = Vec::new();
            if let Some(url) = data_url {
                content.push(json!({"image":url}));
            }
            content.push(json!({"text":prompt}));
            json!({"model":model,"input":{"messages":[{"role":"user","content":content}]},"parameters":{"n":1}})
        }
        Kind::DashscopeCompatible => {
            let mut data = json!({"model":model,"prompt":prompt,"n":1});
            if let Some(url) = data_url {
                data["image"] = json!(url);
            }
            data
        }
        Kind::Ark => {
            let mut data = json!({"model":model,"prompt":prompt,"size":"2K","response_format":"b64_json","stream":false});
            if let Some(url) = data_url {
                data["image"] = json!(url);
            }
            // 5.0 pro/flash不接收组图参数；未知Endpoint ID沿用服务默认单图。已知组图版本显式关闭。
            if model.contains("seedream-4-") || model.contains("seedream-5-0-lite") {
                data["sequential_image_generation"] = json!("disabled");
            }
            data
        }
        Kind::OpenRouter => {
            let mut data = json!({"model":model,"prompt":prompt,"n":1,"stream":false,"provider":{"allow_fallbacks":false}});
            if let Some(url) = data_url {
                data["input_references"] = json!([{"type":"image_url","image_url":{"url":url}}]);
            }
            data
        }
    };
    let request = client
        .search_http()
        .post(target(kind, snapshot)?)
        .json(&data);
    Ok(if kind == Kind::Gemini {
        request.header("x-goog-api-key", &snapshot.key)
    } else {
        request.bearer_auth(&snapshot.key)
    })
}
pub(super) fn normalize(snapshot: &Snapshot, value: Value) -> AppResult<Value> {
    match detect(&snapshot.provider.base_url) {
        Some(kind) => normalize_for(kind, value),
        None => Ok(value),
    }
}
fn normalize_for(kind: Kind, value: Value) -> AppResult<Value> {
    match kind {
        Kind::Gemini => gemini_image(value),
        Kind::Dashscope | Kind::DashscopeCompatible
            if value.get("output").is_some() || value.get("code").is_some() =>
        {
            dashscope_image(value)
        }
        _ => {
            if value.get("error").is_some_and(|e| !e.is_null())
                || value.pointer("/data/0/error").is_some_and(|e| !e.is_null())
            {
                return Err(failed("生图服务返回了生成错误，本次没有自动重试"));
            }
            Ok(value)
        }
    }
}
fn image_value(raw: &str) -> AppResult<Value> {
    if raw.starts_with("data:") {
        let (header, encoded) = raw
            .split_once(',')
            .ok_or_else(|| failed("服务返回的图片数据格式无效"))?;
        if !matches!(
            header,
            "data:image/png;base64" | "data:image/jpeg;base64" | "data:image/webp;base64"
        ) {
            return Err(failed("服务返回不支持的图片格式，仅支持PNG、JPEG、WebP"));
        }
        Ok(json!({"data":[{"b64_json":encoded}]}))
    } else {
        Ok(json!({"data":[{"url":raw}]}))
    }
}
fn gemini_image(value: Value) -> AppResult<Value> {
    if value
        .pointer("/promptFeedback/blockReason")
        .is_some_and(|v| !v.is_null() && v.as_str() != Some("BLOCK_REASON_UNSPECIFIED"))
    {
        return Err(failed(
            "Gemini拒绝了本次绘图请求，没有返回图片；不会自动重试",
        ));
    }
    let candidate = value
        .pointer("/candidates/0")
        .ok_or_else(|| failed("Gemini未返回图片候选，请确认模型支持生图及当前输入"))?;
    if candidate
        .get("finishReason")
        .and_then(Value::as_str)
        .is_some_and(|reason| reason != "STOP")
    {
        return Err(failed(
            "Gemini图片生成未正常完成或被拦截，本次没有保存图片，也不会自动重试",
        ));
    }
    let parts = candidate
        .pointer("/content/parts")
        .and_then(Value::as_array)
        .ok_or_else(|| failed("Gemini未返回可识别的图片内容"))?;
    for part in parts {
        if part.get("thought").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if let Some(data) = part.get("inlineData").or_else(|| part.get("inline_data")) {
            let mime = data
                .get("mimeType")
                .or_else(|| data.get("mime_type"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if !matches!(mime, "image/png" | "image/jpeg" | "image/webp") {
                return Err(failed("Gemini返回的图片格式不受支持"));
            }
            return Ok(
                json!({"data":[{"b64_json":data.get("data").and_then(Value::as_str).ok_or_else(|| failed("Gemini图片数据缺失"))?}]}),
            );
        }
    }
    Err(failed(
        "Gemini只返回了文字，没有返回最终图片；请确认模型支持图片输出",
    ))
}
fn dashscope_image(value: Value) -> AppResult<Value> {
    if value
        .get("code")
        .and_then(Value::as_str)
        .is_some_and(|code| !code.is_empty())
    {
        return Err(failed(
            "百炼返回生成错误，请检查所选模型的输入能力、地域与权限；未自动重试",
        ));
    }
    if value.pointer("/output/task_id").is_some() && value.pointer("/output/choices").is_none() {
        return Err(failed(
            "百炼只返回了异步任务，尚未返回可保存的图片；本次没有保存图片，不会重复提交生成请求",
        ));
    }
    let content = value
        .pointer("/output/choices/0/message/content")
        .and_then(Value::as_array)
        .ok_or_else(|| failed("百炼未返回可识别的图片内容"))?;
    if value
        .pointer("/output/choices/0/finish_reason")
        .and_then(Value::as_str)
        .is_some_and(|reason| reason != "stop")
    {
        return Err(failed("百炼图片生成未完整结束，未保存本次图片"));
    }
    let raw = content
        .iter()
        .find_map(|part| part.get("image").and_then(Value::as_str))
        .ok_or_else(|| failed("百炼未返回图片，不能把文字当作绘图结果"))?;
    image_value(raw)
}
#[cfg(test)]
mod tests;
