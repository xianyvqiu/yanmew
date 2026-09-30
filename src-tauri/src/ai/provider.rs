use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    /// 工具调用参数必须作为 JSON 字符串发送，不能直接序列化内部 ToolCall。
    pub fn tool_request(choice: &Choice) -> Self {
        let mut message = Self::new("assistant", choice.content.clone());
        message.tool_calls = Some(choice.tools.iter().map(|call| serde_json::json!({
            "id":call.id,"type":"function","function":{"name":call.name,"arguments":call.arguments.to_string()}
        })).collect());
        message
    }
    pub fn tool_result(id: &str, result: Value) -> Self {
        let mut message = Self::new("tool", result.to_string());
        message.tool_call_id = Some(id.to_owned());
        message
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
