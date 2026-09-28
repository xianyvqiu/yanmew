//! 聊天创建提醒的受限处理器；阶段 9 的工具循环复用此入口，不另写提醒 SQL。
use crate::{
    ai::provider::{Message, ToolCall},
    error::{AppError, AppResult},
    events::ReminderEventSink,
    models::Reminder,
    reminders::{
        self,
        schedule::{Repeat, Schedule},
        ReminderInput,
    },
    runtime::{Runtime, Store},
    state::RequestLease,
};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

pub const TOOL_NAME: &str = "create_reminder";
pub const READ_ONLY: bool = false;

/// 与界面输入共用 name/content/fireAt/schedule；不向模型开放来源、ID 或内部游标。
pub fn definition() -> Value {
    let clock = json!({"type":"string","pattern":"^([01][0-9]|2[0-3]):[0-5][0-9]$","description":"电脑当前时区的 24 小时制 HH:MM"});
    let mut schedules = Vec::new();
    for (repeat, field, maximum) in [
        ("daily", None, 0),
        ("weekly", Some("weekdays"), 7),
        ("monthly", Some("days"), 31),
    ] {
        let mut properties = json!({"version":{"type":"integer","enum":[1]},"repeat":{"type":"string","enum":[repeat]},"time":clock.clone()});
        let mut required = vec!["version", "repeat", "time"];
        if let Some(field) = field {
            properties[field] = json!({"type":"array","minItems":1,"maxItems":maximum,"uniqueItems":true,
                "items":{"type":"integer","minimum":1,"maximum":maximum}});
            required.push(field);
        }
        schedules.push(json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}));
    }
    schedules.push(json!({"type":"null"}));
    json!({"type":"function","function":{
        "name":TOOL_NAME,
        "description":"仅当用户明确要求设置提醒且已给出明确时间时调用。缺少日期、时间或循环条件时先向用户询问，不猜测。不用于完成、修改或删除已有提醒。一次性使用 fireAt 并令 schedule=null；循环使用 schedule 并令 fireAt=null。每周 1=周一、7=周日；每月不存在日期跳过。成功以返回的提醒 ID 为准，失败不能声称已经设置。",
        "parameters":{"type":"object","properties":{
            "name":{"type":"string","minLength":1,"description":"简短、非空的提醒名称"},
            "content":{"type":"string","minLength":1,"description":"用户要提醒的具体事情，不添加未经要求的内容"},
            "fireAt":{"type":["string","null"],"description":"一次性未来时间，RFC3339，必须含 Z 或 UTC 偏移；循环为 null"},
            "schedule":{"anyOf":schedules}
        },"required":["name","content","fireAt","schedule"],"additionalProperties":false}
    }})
}

/// 由后端提供给主请求，不接受模型或界面自报“当前时间”；不额外发起一次 AI 提取请求。
pub fn time_context<T: TimeZone>(now: DateTime<Utc>, zone: &T) -> Message {
    let local = now.with_timezone(zone).fixed_offset();
    Message::new("system", format!(
        "提醒时间依据：电脑当前本地时间 {}，星期{}，当前 UTC 偏移 {}。\n\
         今天、明天、多久以后都以这个时刻为依据；时间不明确先询问用户。\n\
         循环提醒跟随电脑当前时区，跳过不存在的日期或夏令时时刻，重复时刻只采用第一次。\n\
         一次性提醒使用包含时区的未来 RFC3339 时间。只在 create_reminder 返回成功后告知用户已创建；不得重新执行已成功的调用。",
        local.to_rfc3339(), local.format("%u"), local.format("%:z")
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Arguments {
    name: String,
    content: String,
    fire_at: Option<String>,
    schedule: Option<ScheduleInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScheduleInput {
    version: u8,
    repeat: Repeat,
    time: String,
    weekdays: Option<Vec<u32>>,
    days: Option<Vec<u32>>,
}

fn parse_arguments(arguments: &Value) -> AppResult<ReminderInput> {
    if ["name", "content", "fireAt", "schedule"]
        .iter()
        .any(|key| arguments.get(key).is_none())
    {
        return Err(AppError::validation(
            "arguments",
            "请补齐名称、详情和明确时间；未使用的 fireAt 或 schedule 须为 null",
        ));
    }
    let parsed: Arguments = serde_json::from_value(arguments.clone()).map_err(|_| {
        AppError::validation(
            "arguments",
            "提醒参数不完整或格式不正确，请补充名称、详情和明确时间",
        )
    })?;
    if let Some(schedule) = &parsed.schedule {
        let raw_schedule = &arguments["schedule"];
        let wrong_fields = match schedule.repeat {
            Repeat::Daily => {
                raw_schedule.get("weekdays").is_some() || raw_schedule.get("days").is_some()
            }
            Repeat::Weekly => raw_schedule.get("days").is_some(),
            Repeat::Monthly => raw_schedule.get("weekdays").is_some(),
        };
        if wrong_fields {
            return Err(AppError::validation(
                "schedule",
                "循环规则只允许填写对应的星期或月日字段",
            ));
        }
    }
    if parsed.name.trim().is_empty() || parsed.content.trim().is_empty() {
        return Err(AppError::validation(
            "content",
            "请先向用户确认提醒名称和具体事情",
        ));
    }
    let schedule = parsed.schedule.map(|schedule| Schedule {
        version: schedule.version,
        repeat: schedule.repeat,
        time: schedule.time,
        weekdays: schedule.weekdays,
        days: schedule.days,
        next_local_at: None,
    });
    if let Some(schedule) = &schedule {
        schedule.validate()?;
        if parsed.fire_at.is_some() {
            return Err(AppError::validation(
                "fireAt",
                "循环提醒不能同时指定一次性时间",
            ));
        }
    } else if parsed.fire_at.is_none() {
        return Err(AppError::validation(
            "fireAt",
            "请先向用户询问明确的提醒时间，本次未创建提醒",
        ));
    }
    Ok(ReminderInput {
        name: parsed.name,
        content: parsed.content,
        fire_at: parsed.fire_at,
        schedule,
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationResult {
    pub request_id: String,
    pub tool_call_id: String,
    pub reminder: Reminder,
    pub already_applied: bool,
}

/// 每个普通聊天请求只建立一个实例，跨工具轮保留；重说不注册此写入工具。
/// 保留同一请求租约以便移交后台执行器；请求终止先取消再释放处理器，不注册为前端 command。
pub struct ReminderCreation {
    runtime: Runtime,
    request: Arc<RequestLease>,
    persona_id: i64,
    session_id: i64,
    context_revision: u64,
    applied: HashMap<String, (Value, Reminder)>,
}

impl ReminderCreation {
    pub fn new(
        runtime: &Runtime,
        request: &Arc<RequestLease>,
        persona_id: i64,
        session_id: i64,
        context_revision: u64,
    ) -> AppResult<Self> {
        let handler = Self {
            runtime: runtime.clone(),
            request: request.clone(),
            persona_id,
            session_id,
            context_revision,
            applied: HashMap::new(),
        };
        runtime.read(|store, _| handler.require_context(store))?;
        Ok(handler)
    }

    fn require_context(&self, store: &Store) -> AppResult<()> {
        self.request.require_scope(
            &self.runtime.requests,
            &format!("chat:{}", self.session_id),
            self.persona_id,
        )?;
        self.runtime.jobs_cancel.check()?;
        let context = crate::persona::context(&store.connection, store.context_revision)?;
        if context.persona_id != self.persona_id.to_string()
            || context.session_id.as_deref() != Some(self.session_id.to_string().as_str())
            || context.context_revision != self.context_revision
        {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前人设或会话已经改变，未创建提醒",
            ));
        }
        crate::sessions::get(&store.connection, self.persona_id, self.session_id)?;
        let away = store.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",
            [self.session_id],
            |row| row.get::<_, bool>(0),
        )?;
        if away {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前会话已出门，不能继续执行聊天提醒工具",
            ));
        }
        Ok(())
    }

    /// 同步短事务入口；工具循环放在阻塞执行器调用，禁止持有锁进行模型请求。
    /// 已提交提醒独立于后续聊天正文；正文中断不回滚提醒，也不自动重放。
    pub fn execute<T: TimeZone>(
        &mut self,
        call: &ToolCall,
        zone: &T,
        events: &dyn ReminderEventSink,
    ) -> AppResult<CreationResult> {
        if call.name != TOOL_NAME || call.id.trim().is_empty() {
            return Err(AppError::validation(
                "toolCall",
                "提醒工具名称或调用 ID 无效",
            ));
        }
        let (reminder, already_applied) = self.runtime.read(|store, _| {
            self.require_context(store)?;
            if let Some((arguments, reminder)) = self.applied.get(&call.id) {
                if arguments != &call.arguments {
                    return Err(AppError::new(
                        "TOOL_CALL_CONFLICT",
                        "同一工具调用 ID 的参数发生变化，未重复创建提醒",
                    ));
                }
                return Ok((reminder.clone(), true));
            }
            let input = parse_arguments(&call.arguments)?;
            let reminder = reminders::save_rule(
                &mut store.connection,
                None,
                input,
                "chat",
                self.runtime.clock.sample()?.utc,
                zone,
            )?;
            Ok((reminder, false))
        })?;
        if !already_applied {
            self.applied
                .insert(call.id.clone(), (call.arguments.clone(), reminder.clone()));
            events.reminders_changed();
        }
        Ok(CreationResult {
            request_id: self.request.id.to_string(),
            tool_call_id: call.id.clone(),
            reminder,
            already_applied,
        })
    }
}

#[cfg(test)]
mod tests;
