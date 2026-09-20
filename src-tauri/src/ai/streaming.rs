use super::provider::{Choice, Delta, Finish, ToolCall};
use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn protocol() -> AppError {
    AppError::new(
        "AI_PROTOCOL_ERROR",
        "模型响应格式无效或不完整，请检查接口兼容性",
    )
}
const MAX_FRAME: usize = 1024 * 1024;
/// 字节缓冲到完整行才进行 UTF-8 解码，不会在网络分块处替换中文字符。
#[derive(Default)]
pub struct SseDecoder {
    pending: Vec<u8>,
    data: Vec<String>,
    frame_size: usize,
}
impl SseDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> AppResult<Vec<String>> {
        let mut events = Vec::new();
        for byte in bytes {
            if *byte != b'\n' {
                self.pending.push(*byte);
                if self.pending.len() + self.frame_size > MAX_FRAME {
                    return Err(protocol());
                }
                continue;
            }
            let mut raw = std::mem::take(&mut self.pending);
            if raw.last() == Some(&b'\r') {
                raw.pop();
            }
            let line = String::from_utf8(raw).map_err(|_| protocol())?;
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.join("\n"));
                    self.data.clear();
                }
                self.frame_size = 0;
            } else if let Some(data) = line.strip_prefix("data:") {
                let data = data.strip_prefix(' ').unwrap_or(data);
                self.frame_size += data.len() + 1;
                if self.frame_size > MAX_FRAME {
                    return Err(protocol());
                }
                self.data.push(data.into());
            }
        }
        Ok(events)
    }
}
#[derive(Default)]
struct PartialTool {
    id: String,
    name: String,
    arguments: String,
}
#[derive(Default)]
struct PartialChoice {
    content: String,
    tools: BTreeMap<u32, PartialTool>,
    finish: Option<Finish>,
}
#[derive(Default)]
pub struct StreamAccumulator {
    choices: BTreeMap<u32, PartialChoice>,
    done: bool,
}
impl StreamAccumulator {
    pub fn push(&mut self, data: &str) -> AppResult<Vec<Delta>> {
        if self.done {
            return Err(protocol());
        }
        if data.trim() == "[DONE]" {
            if self.choices.is_empty() || self.choices.values().any(|c| c.finish.is_none()) {
                return Err(protocol());
            }
            self.done = true;
            return Ok(vec![]);
        }
        let value: Value = serde_json::from_str(data).map_err(|_| protocol())?;
        if value.get("error").is_some() {
            return Err(AppError::new(
                "PROVIDER_UNAVAILABLE",
                "提供商在流式回复中报告错误",
            ));
        }
        let choices = value
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(protocol)?;
        let mut deltas = vec![];
        for item in choices {
            let index = u32::try_from(
                item.get("index")
                    .and_then(Value::as_u64)
                    .ok_or_else(protocol)?,
            )
            .map_err(|_| protocol())?;
            if index > 15 {
                return Err(protocol());
            }
            let choice = self.choices.entry(index).or_default();
            if choice.finish.is_some() {
                return Err(protocol());
            }
            let delta = item
                .get("delta")
                .filter(|v| v.is_object())
                .ok_or_else(protocol)?;
            if let Some(text) = delta.get("content").filter(|v| !v.is_null()) {
                let text = text.as_str().ok_or_else(protocol)?;
                choice.content.push_str(text);
                if !text.is_empty() {
                    deltas.push(Delta::Text {
                        choice_index: index,
                        text: text.into(),
                    });
                }
            }
            if let Some(tools) = delta.get("tool_calls").filter(|v| !v.is_null()) {
                for call in tools.as_array().ok_or_else(protocol)? {
                    let index = u32::try_from(
                        call.get("index")
                            .and_then(Value::as_u64)
                            .ok_or_else(protocol)?,
                    )
                    .map_err(|_| protocol())?;
                    if index > 63 {
                        return Err(protocol());
                    }
                    let tool = choice.tools.entry(index).or_default();
                    if let Some(kind) = call.get("type") {
                        if kind != "function" {
                            return Err(protocol());
                        }
                    }
                    if let Some(id) = call.get("id").filter(|v| !v.is_null()) {
                        let id = id.as_str().ok_or_else(protocol)?;
                        if !tool.id.is_empty() && tool.id != id {
                            return Err(protocol());
                        }
                        tool.id = id.into();
                    }
                    if let Some(function) = call.get("function") {
                        if let Some(name) = function.get("name").filter(|v| !v.is_null()) {
                            tool.name.push_str(name.as_str().ok_or_else(protocol)?);
                        }
                        if let Some(args) = function.get("arguments").filter(|v| !v.is_null()) {
                            tool.arguments.push_str(args.as_str().ok_or_else(protocol)?);
                        }
                    }
                }
            }
            if let Some(reason) = item.get("finish_reason").filter(|v| !v.is_null()) {
                choice.finish =
                    Some(serde_json::from_value(reason.clone()).map_err(|_| protocol())?);
            }
        }
        Ok(deltas)
    }
    pub fn is_done(&self) -> bool {
        self.done
    }
    pub fn finish(self) -> AppResult<Vec<Choice>> {
        if !self.done {
            return Err(AppError::new(
                "AI_STREAM_INTERRUPTED",
                "模型连接提前结束，回复尚未完整接收",
            ));
        }
        self.choices
            .into_iter()
            .map(|(index, choice)| {
                let finish = choice.finish.ok_or_else(protocol)?;
                let mut tools = vec![];
                // 截断/过滤状态保留正文，但绝不交付不完整工具调用。
                if finish == Finish::ToolCalls {
                    for (index, tool) in choice.tools {
                        if tool.id.is_empty()
                            || tool.name.is_empty()
                            || tool.name.len() > 128
                            || !tool
                                .name
                                .bytes()
                                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
                        {
                            return Err(protocol());
                        }
                        let arguments: Value =
                            serde_json::from_str(&tool.arguments).map_err(|_| protocol())?;
                        if !arguments.is_object()
                            || tools.iter().any(|t: &ToolCall| t.id == tool.id)
                        {
                            return Err(protocol());
                        }
                        tools.push(ToolCall {
                            index,
                            id: tool.id,
                            name: tool.name,
                            arguments,
                        });
                    }
                    if tools.is_empty() {
                        return Err(protocol());
                    }
                } else if finish == Finish::Stop && !choice.tools.is_empty() {
                    return Err(protocol());
                }
                Ok(Choice {
                    index,
                    content: choice.content,
                    tools,
                    finish,
                })
            })
            .collect()
    }
}
