use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}
impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }
}
/// 只在后端创建；不允许前端提交整份权威会话上下文。
pub struct GenerateRequest {
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub tools: Vec<Value>,
    pub json: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finish {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ToolCall {
    pub index: u32,
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Choice {
    pub index: u32,
    pub content: String,
    pub tools: Vec<ToolCall>,
    pub finish: Finish,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Delta {
    Text { choice_index: u32, text: String },
}
