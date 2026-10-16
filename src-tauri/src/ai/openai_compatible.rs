use super::{
    provider::{Choice, Finish, GenerateRequest},
    streaming::protocol,
};
use crate::error::{AppError, AppResult};
use serde_json::{json, Value};

/// DeepSeek 工具协议要求历史 assistant 消息显式带推理字段。
/// 本轮真实推理由消息自身携带；数据库恢复的历史没有推理，明确以空串表示。
/// 这里只适配消息格式，不改变思考模式、模型选择或输出预算。
pub(super) fn apply_message_protocol(base_url: &str, body: &mut Value) {
    let official_deepseek = reqwest::Url::parse(base_url)
        .ok()
        .is_some_and(|url| url.host_str() == Some("api.deepseek.com"));
    let has_tools = body.get("tools").and_then(Value::as_array)
        .is_some_and(|tools| !tools.is_empty());
    if official_deepseek && has_tools {
        if let Some(messages) = body.get_mut("messages").and_then(Value::as_array_mut) {
            for message in messages {
                if message.get("role").and_then(Value::as_str) == Some("assistant")
                    && message.get("reasoning_content").is_none()
                {
                    message["reasoning_content"] = json!("");
                }
            }
        }
    }
}

pub fn request_body(model: &str, input: &GenerateRequest, stream: bool) -> AppResult<Value> {
    if model.trim().is_empty()
        || input.messages.is_empty()
        || input.max_tokens == 0
    {
        return Err(AppError::validation(
            "request",
            "模型、消息和输出预算必须有效",
        ));
    }
    if input
        .messages
        .iter()
        .any(|m| !["system", "user", "assistant", "tool"].contains(&m.role.as_str())
            || (m.role == "tool") != m.tool_call_id.as_ref().is_some_and(|id| !id.is_empty())
            || (m.tool_calls.is_some() && m.role != "assistant"))
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
        reasoning_content: None,
    })
}

#[cfg(test)]
mod protocol_tests {
    use super::*;

    #[test]
    fn tool_protocol_preserves_live_reasoning_and_marks_history_without_it_explicitly() {
        let mut body = json!({"tools":[{"type":"function"}],"messages":[
            {"role":"assistant","content":"历史答案"},
            {"role":"assistant","content":"","tool_calls":[],"reasoning_content":"本轮完整推理"},
            {"role":"tool","tool_call_id":"call-1","content":"结果"}
        ]});
        apply_message_protocol("https://api.deepseek.com/v1", &mut body);
        assert_eq!(body["messages"][0]["reasoning_content"], "");
        assert_eq!(body["messages"][1]["reasoning_content"], "本轮完整推理");
        assert!(body["messages"][2].get("reasoning_content").is_none());
        assert!(body.get("thinking").is_none());
        let original = json!({"tools":[{}],"messages":[{"role":"assistant","content":"历史"}]});
        for base in ["https://api.openai.com/v1", "https://api.deepseek.com.example.org/v1"] {
            let mut unchanged = original.clone();
            apply_message_protocol(base, &mut unchanged);
            assert_eq!(unchanged, original);
        }
    }
}
