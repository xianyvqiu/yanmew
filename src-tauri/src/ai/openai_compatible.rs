use super::{
    provider::{Choice, Finish, GenerateRequest},
    streaming::protocol,
};
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};

pub fn request_body(model: &str, input: &GenerateRequest, stream: bool) -> AppResult<Value> {
    if model.trim().is_empty()
        || input.messages.is_empty()
        || input.max_tokens == 0
        || input.max_tokens > 32768
    {
        return Err(AppError::validation(
            "request",
            "模型、消息和输出预算必须有效",
        ));
    }
    if input
        .messages
        .iter()
        .any(|m| !["system", "user", "assistant"].contains(&m.role.as_str()))
    {
        return Err(protocol());
    }
    let mut body = json!({"model":model,"messages":input.messages,"stream":stream,"max_tokens":input.max_tokens,"n":1});
    if input.json {
        body["response_format"] = json!({"type":"json_object"});
    }
    if !input.tools.is_empty() {
        body["tools"] = json!(input.tools);
    }
    Ok(body)
}
pub fn complete_response(bytes: &[u8]) -> AppResult<Choice> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| protocol())?;
    let items = value
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(protocol)?;
    if items.len() != 1 {
        return Err(protocol());
    }
    let choice = &items[0];
    let finish: Finish =
        serde_json::from_value(choice.get("finish_reason").cloned().ok_or_else(protocol)?)
            .map_err(|_| protocol())?;
    if finish != Finish::Stop {
        return Err(AppError::new(
            if finish == Finish::Length {
                "AI_TRUNCATED"
            } else {
                "AI_PROTOCOL_ERROR"
            },
            "模型未完整生成所需内容，原数据未修改",
        ));
    }
    let content = choice
        .pointer("/message/content")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(protocol)?
        .to_owned();
    Ok(Choice {
        index: 0,
        content,
        tools: vec![],
        finish,
    })
}
