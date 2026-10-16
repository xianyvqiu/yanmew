//! 模型能力是可重建的端点/模型缓存，不能用相似模型名猜容量。
use crate::{
    error::{AppError, AppResult},
    providers::ConnectionTest,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capacity {
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub source: String,
    pub estimated: bool,
    pub reference: Option<String>,
    pub checked_at: Option<String>,
}
#[derive(Clone)]
struct Entry {
    capacity: Capacity,
    fetched: Instant,
    parameter: &'static str,
}
static CACHE: OnceLock<Mutex<HashMap<(String, String), Entry>>> = OnceLock::new();
fn key(base: &str, model: &str) -> (String, String) {
    (base.trim_end_matches('/').to_owned(), model.to_owned())
}
fn entry(base: &str, model: &str) -> Option<Entry> {
    CACHE
        .get()?
        .lock()
        .ok()?
        .get(&key(base, model))
        .filter(|v| v.fetched.elapsed() < Duration::from_secs(86400))
        .cloned()
}
pub fn resolve(base: &str, model: &str, manual: Option<u32>) -> Capacity {
    let mut result = entry(base, model).map(|v| v.capacity).unwrap_or(Capacity {
        context_window: None,
        max_output_tokens: None,
        source: "unknown".into(),
        estimated: true,
        reference: None,
        checked_at: None,
    });
    if result.source == "unknown"
        && base.trim_end_matches('/') == "https://api.openai.com/v1"
        && matches!(model, "gpt-4.1" | "gpt-4.1-2025-04-14")
    {
        result = Capacity {
            context_window: Some(1_047_576),
            max_output_tokens: Some(32768),
            source: "builtin".into(),
            estimated: true,
            reference: Some("https://developers.openai.com/api/docs/models/gpt-4.1".into()),
            checked_at: Some("2026-10-05".into()),
        };
    }
    if manual.is_some() {
        result.context_window = manual;
        result.source = "manual".into();
    }
    result
}
pub fn output_parameter(config: &ConnectionTest) -> &'static str {
    if config.base_url.trim_end_matches('/') == "https://api.openai.com/v1" {
        return "max_completion_tokens";
    }
    entry(&config.base_url, &config.model)
        .map(|v| v.parameter)
        .unwrap_or("max_tokens")
}
fn positive(value: Option<&Value>) -> Option<u32> {
    value?
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
}
/// 只接受确切模型 ID 和明确数值；供应商不提供这些字段时仍然未知。
pub fn remember(base: &str, model: &str, value: &Value) {
    let item = if value.get("id").and_then(Value::as_str) == Some(model) {
        Some(value)
    } else {
        value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find(|v| v.get("id").and_then(Value::as_str) == Some(model))
            })
    };
    let Some(item) = item else {
        return;
    };
    let context = positive(item.get("context_window"))
        .or_else(|| positive(item.get("context_length")))
        .or_else(|| positive(item.get("max_context_length")));
    let output = positive(item.get("max_output_tokens"));
    let parameter = match item.get("max_output_parameter").and_then(Value::as_str) {
        Some("max_completion_tokens") => "max_completion_tokens",
        _ => "max_tokens",
    };
    if let Ok(mut cache) = CACHE.get_or_init(Default::default).lock() {
        // 配置切换天然使用另一个键；缓存最多保留当前已测试过的 128 组。
        if cache.len() >= 128 {
            cache.retain(|_, v| v.fetched.elapsed() < Duration::from_secs(3600));
        }
        if cache.len() >= 128 {
            cache.clear();
        }
        cache.insert(
            key(base, model),
            Entry {
                capacity: Capacity {
                    context_window: context,
                    max_output_tokens: output,
                    source: if context.is_some() || output.is_some() {
                        "provider"
                    } else {
                        "unknown"
                    }
                    .into(),
                    estimated: true,
                    reference: Some(format!("{}/models", base.trim_end_matches('/'))),
                    checked_at: Some(chrono::Utc::now().to_rfc3339()),
                },
                fetched: Instant::now(),
                parameter,
            },
        );
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Budget {
    pub context_window: Option<u32>,
    pub configured_max_output_tokens: u32,
    pub effective_max_output_tokens: u32,
    pub estimated_input_tokens: usize,
    pub capacity_source: String,
    pub estimated: bool,
    pub reduced: bool,
}
pub const SAFETY: usize = 256;
/// 所有生成任务共用用途配置及确切模型能力，不按预期正文长度压缩推理预算。
pub fn prepare_request(
    config: &ConnectionTest,
    input: &crate::ai::provider::GenerateRequest,
) -> AppResult<crate::ai::provider::GenerateRequest> {
    if config.max_output_tokens == 0 || input.max_tokens == 0 {
        return Err(AppError::validation("maxOutputTokens", "最大输出必须为正整数"));
    }
    let capacity = resolve(&config.base_url, &config.model, config.context_window_override);
    let mut output = input.max_tokens.min(config.max_output_tokens)
        .min(capacity.max_output_tokens.unwrap_or(u32::MAX));
    if let Some(window) = capacity.context_window {
        let estimated = estimate(&input.messages, &input.tools)?;
        let available = (window as usize).saturating_sub(estimated.saturating_add(SAFETY));
        if available == 0 {
            return Err(too_large());
        }
        output = output.min(available.min(u32::MAX as usize) as u32);
    }
    let mut request = input.clone();
    request.max_tokens = output;
    Ok(request)
}
pub fn too_large() -> AppError {
    AppError::new(
        "CONTEXT_TOO_LARGE",
        "必要的人设、当前输入或工具结果超过模型容量，请更换模型或调整内容",
    )
}
pub fn estimate(messages: &[crate::ai::provider::Message], tools: &[Value]) -> AppResult<usize> {
    // 无精确 tokenizer 时，完整 JSON 的 UTF-8 字节数 + 消息/协议余量；不是服务端真实 token 数。
    // Base64不是文字token；图片使用保守估算，服务端容量错误仍复用一次裁剪重试。
    let image_count=messages.iter().map(|m|m.images.len()).sum::<usize>();
    let text_messages=messages.iter().cloned().map(|mut m|{m.images.clear();m}).collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&serde_json::json!({"messages":text_messages,"tools":tools}))
        .map_err(|_| AppError::storage())?
        .len();
    Ok(bytes
        .saturating_add(image_count.saturating_mul(4096))
        .saturating_add(messages.len().saturating_mul(32))
        .saturating_add(128))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_model_metadata_is_scoped_validated_and_manual_override_wins() {
        let base = "https://capacity-test.invalid/v1";
        remember(
            base,
            "exact-model",
            &serde_json::json!({"data":[{"id":"exact-model","context_window":100000,"max_output_tokens":5000,"max_output_parameter":"max_completion_tokens"}]}),
        );
        let known = resolve(base, "exact-model", None);
        assert_eq!(known.context_window, Some(100000));
        assert_eq!(known.max_output_tokens, Some(5000));
        assert_eq!(known.source, "provider");
        assert_eq!(
            resolve(base, "exact-model", Some(150000)).context_window,
            Some(150000)
        );
        assert_eq!(
            resolve("https://other.invalid/v1", "exact-model", None).source,
            "unknown"
        );
        assert_eq!(resolve(base, "exact-model-variant", None).source, "unknown");
        remember(
            base,
            "bad",
            &serde_json::json!({"id":"bad","context_window":-1,"max_output_tokens":0}),
        );
        assert_eq!(resolve(base, "bad", None).source, "unknown");
        let config = ConnectionTest {
            max_output_tokens: crate::providers::default_max_output_tokens(),
            context_window_override: None,
            base_url: base.into(),
            model: "exact-model".into(),
            api_key: None,
        };
        assert_eq!(output_parameter(&config), "max_completion_tokens");
        assert_eq!(
            resolve("https://api.openai.com/v1", "gpt-4.1", None).source,
            "builtin"
        );
        assert_eq!(
            resolve("https://proxy.invalid/v1", "gpt-4.1", None).source,
            "unknown"
        );
    }
}

#[cfg(test)]
mod generation_budget_tests {
    use super::*;
    use crate::ai::provider::{GenerateRequest, Message};

    #[test]
    fn generation_uses_configured_budget_for_any_model_and_respects_actual_limits() {
        let mut config = ConnectionTest {
            base_url: "https://generation-budget.invalid/v1".into(),
            model: "unlisted-reasoning-model".into(),
            api_key: None,
            max_output_tokens: 32768,
            context_window_override: None,
        };
        let mut input = GenerateRequest {
            messages: vec![Message::new("user", "hi")],
            max_tokens: config.max_output_tokens,
            tools: vec![],
            json: true,
        };
        assert_eq!(prepare_request(&config, &input).unwrap().max_tokens, 32768);
        remember(&config.base_url, &config.model, &serde_json::json!({
            "id":config.model,"max_output_tokens":8192,"context_window":100000
        }));
        assert_eq!(prepare_request(&config, &input).unwrap().max_tokens, 8192);
        config.max_output_tokens = 6000;
        assert_eq!(prepare_request(&config, &input).unwrap().max_tokens, 6000);
        config.context_window_override = Some(2048);
        let available = 2048 - estimate(&input.messages, &input.tools).unwrap() - SAFETY;
        assert_eq!(prepare_request(&config, &input).unwrap().max_tokens, available as u32);
        input.messages[0].content = "x".repeat(4096);
        assert_eq!(prepare_request(&config, &input).err().unwrap().code, "CONTEXT_TOO_LARGE");
    }
}
