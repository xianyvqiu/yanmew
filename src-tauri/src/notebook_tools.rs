//! 会话记事本受限工具；由后续聊天工具循环调用，不向前端开放任意工具执行。
use crate::{
    ai::provider::ToolCall,
    error::{AppError, AppResult},
    models::Notebook,
    notebook::{self, Edit},
    runtime::{Runtime, Store},
    state::RequestLease,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

pub const READ_TOOL: &str = "read_notebook";
pub const UPDATE_TOOL: &str = "update_notebook";

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{
            "name":READ_TOOL,
            "description":"读取当前会话整份记事本及从1开始的段落编号。修改前必须读取。limit_chars是整份记事本的建议上限，超限内容仍完整保留；接近或超过上限时先合并重复内容和精简旧段落。",
            "parameters":{"type":"object","properties":{},"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name":UPDATE_TOOL,
            "description":"修改当前会话记事本。先read_notebook，段落以空行分隔；append追加，replace替换指定段，delete删除指定段。每次成功后使用返回的新段落编号；冲突时重新读取再决定。字数上限是整份记事本的软限制，不是每次新增额度；超限时优先整理。仅返回成功才表示已保存。",
            "parameters":{"type":"object","properties":{
                "operation":{"type":"string","enum":["append","replace","delete"]},
                "section_index":{"type":["integer","null"],"minimum":1,"description":"replace/delete使用从1开始的段落号；append必须为null"},
                "content":{"type":["string","null"],"description":"append/replace的非空正文；delete必须为null"}
            },"required":["operation","section_index","content"],"additionalProperties":false}
        }})
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    operation: String,
    section_index: Option<usize>,
    content: Option<String>,
}

fn edit(arguments: &Value) -> AppResult<Edit> {
    let invalid = || AppError::validation("arguments", "记事本参数无效，请按操作填写段落编号和内容");
    if ["operation", "section_index", "content"].iter().any(|key| arguments.get(key).is_none()) {
        return Err(invalid());
    }
    let args: Arguments = serde_json::from_value(arguments.clone()).map_err(|_| invalid())?;
    match (args.operation.as_str(), args.section_index, args.content) {
        ("append", None, Some(content)) => Ok(Edit::Append(content)),
        ("replace", Some(section_index), Some(content)) if section_index > 0 => Ok(Edit::Replace { section_index, content }),
        ("delete", Some(section_index), None) if section_index > 0 => Ok(Edit::Delete { section_index }),
        _ => Err(invalid()),
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Section {
    pub section_index: usize,
    pub content: String,
}

/// 工具协议采用 snake_case；字数按 Unicode 标量统计，负剩余额度显示为零。
#[derive(Clone, Debug, Serialize)]
pub struct NotebookResult {
    pub request_id: String,
    pub tool_call_id: String,
    pub already_applied: bool,
    pub content: String,
    pub sections: Vec<Section>,
    pub limit_chars: usize,
    pub current_chars: usize,
    pub remaining_chars: usize,
    pub over_limit: bool,
}

/// 每次聊天请求一个处理器，跨工具轮保留；工具提交独立于后续聊天正文终态。
pub struct NotebookTools {
    runtime: Runtime,
    request: Arc<RequestLease>,
    persona_id: i64,
    session_id: i64,
    context_revision: u64,
    /// 外层 None 表示尚未读取；内层 None 表示该会话尚无记事本行。
    observed: Option<Option<Notebook>>,
    applied: HashMap<String, (String, Value, NotebookResult)>,
}

impl NotebookTools {
    pub fn new(runtime: &Runtime, request: &Arc<RequestLease>, persona_id: i64, session_id: i64, context_revision: u64) -> AppResult<Self> {
        let handler = Self {
            runtime: runtime.clone(), request: request.clone(), persona_id, session_id, context_revision,
            observed: None, applied: HashMap::new(),
        };
        runtime.read(|store, _| handler.require_context(store))?;
        Ok(handler)
    }

    fn require_context(&self, store: &Store) -> AppResult<()> {
        self.request.require_conversation_scope(&self.runtime.requests, self.session_id, self.persona_id)?;
        self.runtime.jobs_cancel.check()?;
        let context = crate::persona::context(&store.connection, store.context_revision)?;
        if context.persona_id != self.persona_id.to_string()
            || context.session_id.as_deref() != Some(self.session_id.to_string().as_str())
            || context.context_revision != self.context_revision {
            return Err(AppError::new("CONTEXT_CHANGED", "当前人设或会话已经改变，不能继续操作记事本"));
        }
        crate::sessions::get(&store.connection, self.persona_id, self.session_id)?;
        let away: bool = store.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')", [self.session_id], |row| row.get(0))?;
        if away { return Err(AppError::new("CONTEXT_CHANGED", "当前会话已出门，不能继续执行聊天记事本工具")); }
        Ok(())
    }

    /// 放在后台阻塞执行器调用；短锁内检查、读取、修改，不包含网络等待。
    pub fn execute(&mut self, call: &ToolCall) -> AppResult<NotebookResult> {
        if call.id.trim().is_empty() || !matches!(call.name.as_str(), READ_TOOL | UPDATE_TOOL) {
            return Err(AppError::validation("toolCall", "记事本工具名称或调用 ID 无效"));
        }
        let (result, snapshot) = self.runtime.read(|store, _| {
            self.require_context(store)?;
            if let Some((name, arguments, saved)) = self.applied.get(&call.id) {
                if name != &call.name || arguments != &call.arguments {
                    return Err(AppError::new("TOOL_CALL_CONFLICT", "同一工具调用 ID 的名称或参数发生变化，未重复执行"));
                }
                // 写调用幂等返回原回执；只读调用重新读取，避免旧编号被当成最新快照。
                if call.name == UPDATE_TOOL {
                    let mut result = saved.clone();
                    result.already_applied = true;
                    return Ok((result, None));
                }
            }
            // 在任何写入前读取合法配置，避免写入已成功但返回指标失败。
            let settings = crate::config::get(&store.connection)?;
            let limit = settings["notebook_max_chars"].parse::<usize>().map_err(|_| AppError::storage())?;
            let snapshot = if call.name == READ_TOOL {
                if !call.arguments.as_object().is_some_and(|object| object.is_empty()) {
                    return Err(AppError::validation("arguments", "read_notebook 不接受会话或其他参数"));
                }
                notebook::read(&store.connection, self.session_id)?
            } else {
                let expected = self.observed.as_ref().ok_or_else(|| AppError::new("NOTEBOOK_READ_REQUIRED", "修改前请先读取当前会话记事本"))?;
                let content = notebook::apply_edit(expected.as_ref().map_or("", |value| value.content.as_str()), edit(&call.arguments)?)?;
                Some(notebook::save(&mut store.connection, self.session_id, expected, &content, self.runtime.clock.sample()?.utc)?)
            };
            let content = snapshot.as_ref().map_or_else(String::new, |value| value.content.clone());
            let count = content.chars().count();
            let sections = notebook::sections(&content).into_iter().enumerate()
                .map(|(index, content)| Section { section_index: index + 1, content }).collect();
            Ok((NotebookResult {
                request_id: self.request.id.to_string(), tool_call_id: call.id.clone(), already_applied: false,
                content, sections, limit_chars: limit, current_chars: count,
                remaining_chars: limit.saturating_sub(count), over_limit: count > limit,
            }, Some(snapshot)))
        })?;
        if let Some(snapshot) = snapshot {
            self.observed = Some(snapshot);
            self.applied.insert(call.id.clone(), (call.name.clone(), call.arguments.clone(), result.clone()));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
