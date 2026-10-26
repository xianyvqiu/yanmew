use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<Value>>,
    pub tool_call_id: Option<String>,
    pub images:Vec<String>,
    /// 仅本轮工具协议回传使用，不进入聊天消息表或前端事件。
    pub reasoning_content: Option<String>,
}
impl Serialize for Message {
    fn serialize<S:serde::Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {
        use serde::ser::SerializeMap;
        let mut map=serializer.serialize_map(None)?;
        map.serialize_entry("role",&self.role)?;
        if self.images.is_empty(){map.serialize_entry("content",&self.content)?;}else{
            let mut content=vec![serde_json::json!({"type":"text","text":self.content})];
            content.extend(self.images.iter().map(|url|serde_json::json!({"type":"image_url","image_url":{"url":url,"detail":"auto"}})));
            map.serialize_entry("content",&content)?;
        }
        if let Some(calls)=&self.tool_calls{map.serialize_entry("tool_calls",calls)?;}
        if let Some(id)=&self.tool_call_id{map.serialize_entry("tool_call_id",id)?;}
        if let Some(reasoning)=&self.reasoning_content{map.serialize_entry("reasoning_content",reasoning)?;}
        map.end()
    }
}
impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            images:vec![],
            reasoning_content: None,
        }
    }
    /// 工具调用参数必须作为 JSON 字符串发送，不能直接序列化内部 ToolCall。
    pub fn tool_request(choice: &Choice) -> Self {
        let mut message = Self::new("assistant", choice.content.clone());
        message.reasoning_content = choice.reasoning_content.clone();
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
#[derive(Clone)]
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
    pub usage: Option<super::usage::TokenUsage>,
    pub index: u32,
    pub content: String,
    pub tools: Vec<ToolCall>,
    pub finish: Finish,
    #[serde(skip_serializing)]
    pub reasoning_content: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Delta {
    Text { choice_index: u32, text: String },
}
