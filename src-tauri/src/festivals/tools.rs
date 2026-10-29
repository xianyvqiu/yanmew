//! 聊天中的全局日期工具；与已有工具一样按请求保留快照和写入回执。
use super::custom;
use crate::{
    ai::provider::ToolCall,
    error::{parse_id, AppError, AppResult},
    runtime::Runtime,
    state::RequestLease,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

pub const RULES:&str="自定义日期是全局共用的年度固定公历/农历月日，不复制内置节日。它与定时提醒不同；需要到点提醒时使用提醒工具。月日、历法或闰月信息不明确时询问用户，不猜测。新增/修改须填写祝福词；纪念、悼念及isMemorial=true的日期用纪念向文案，不说快乐。编辑/删除前先读取目标记录。";
pub fn is_write(name: &str) -> bool {
    matches!(
        name,
        "create_custom_festival" | "update_custom_festival" | "delete_custom_festival"
    )
}
pub fn definitions() -> Vec<Value> {
    let input = json!({"type":"object","properties":{
        "kind":{"type":"string","enum":["festival","birthday","anniversary","custom"]},
        "name":{"type":"string","minLength":1},"calendarType":{"type":"string","enum":["solar","lunar"]},
        "month":{"type":"integer","minimum":1,"maximum":12},"day":{"type":"integer","minimum":1,"maximum":31},
        "isLeapMonth":{"type":"boolean","description":"仅农历闰月为true，公历必须false"},
        "blessing":{"type":"string","minLength":1},"note":{"type":["string","null"]}
    },"required":["kind","name","calendarType","month","day","isLeapMonth","blessing","note"],"additionalProperties":false});
    [
        ("list_festival_catalog","查询本年的内置节日。",json!({"type":"object","properties":{},"additionalProperties":false})),
        ("list_custom_festivals","分页读取全局共享的固定日期，返回ID、完整定义及当年全部匹配日期。",json!({"type":"object","properties":{"before_id":{"type":["string","null"]},"limit":{"type":"integer","minimum":1,"maximum":50}},"additionalProperties":false})),
        ("get_custom_festival","读取一条全局自定义日期的完整内容。",json!({"type":"object","properties":{"festival_id":{"type":"string","pattern":"^[1-9][0-9]*$"}},"required":["festival_id"],"additionalProperties":false})),
        ("create_custom_festival","新增年度固定日期，不填写出生年份。",json!({"type":"object","properties":{"input":input.clone()},"required":["input"],"additionalProperties":false})),
        ("update_custom_festival","修改自定义日期；冲突时重新读取，不强行覆盖。",json!({"type":"object","properties":{"festival_id":{"type":"string","pattern":"^[1-9][0-9]*$"},"input":input},"required":["festival_id","input"],"additionalProperties":false})),
        ("delete_custom_festival","删除不再需要的自定义日期。",json!({"type":"object","properties":{"festival_id":{"type":"string","pattern":"^[1-9][0-9]*$"}},"required":["festival_id"],"additionalProperties":false})),
    ].into_iter().map(|(name,description,parameters)|json!({"type":"function","function":{"name":name,"description":description,"parameters":parameters}})).collect()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    before_id: Option<String>,
    limit: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    festival_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    input: custom::Input,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    festival_id: String,
    input: custom::Input,
}
fn parse<T: serde::de::DeserializeOwned>(value: &Value) -> AppResult<T> {
    serde_json::from_value(value.clone())
        .map_err(|_| AppError::validation("arguments", "日期工具参数不符合定义"))
}
pub struct Tools {
    runtime: Runtime,
    lease: Arc<RequestLease>,
    context: crate::memory::management::Context,
    observed: HashMap<String, String>,
    applied: HashMap<String, (String, Value, Value)>,
}
impl Tools {
    pub fn new(
        runtime: &Runtime,
        lease: &Arc<RequestLease>,
        persona: i64,
        session: i64,
        revision: u64,
    ) -> Self {
        Self {
            runtime: runtime.clone(),
            lease: lease.clone(),
            context: crate::memory::management::Context {
                persona_id: persona.to_string(),
                session_id: session.to_string(),
                context_revision: revision,
            },
            observed: HashMap::new(),
            applied: HashMap::new(),
        }
    }
    pub fn execute(&mut self, call: &ToolCall) -> AppResult<Value> {
        if call.id.trim().is_empty() {
            return Err(AppError::validation("toolCall", "调用ID不能为空"));
        }
        let result=self.runtime.read(|store,_| {
            let session=super::current_scope(store,&self.context)?;
            self.lease.require_conversation_scope(&self.runtime.requests,session,parse_id(&self.context.persona_id)?)?;
            self.runtime.jobs_cancel.check()?;
            if store.connection.query_row("SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",[session],|r|r.get::<_,bool>(0))? {return Err(AppError::new("CONTEXT_CHANGED","当前会话已出门，不能继续执行日期工具"));}
            if let Some((name,args,result))=self.applied.get(&call.id) {
                if name!=&call.name||args!=&call.arguments {return Err(AppError::new("TOOL_CALL_CONFLICT","同一调用ID参数已变化，未重复执行"));}
                if is_write(&call.name) {let mut result=result.clone();result["already_applied"]=Value::Bool(true);return Ok(result);}
            }
            let now=self.runtime.clock.sample()?.utc;
            let today=now.with_timezone(&chrono::Local).date_naive();
            super::synchronize(&mut store.connection,today)?;
            match call.name.as_str() {
                "list_festival_catalog" => {
                    if call.arguments.as_object().is_none_or(|o|!o.is_empty()) {return Err(AppError::validation("arguments","母表查询不接受参数"));}
                    Ok(json!({"records":custom::catalog(&store.connection,today)?}))
                }
                "list_custom_festivals" => {let args:Page=parse(&call.arguments)?;let before=args.before_id.as_deref().map(parse_id).transpose()?;let records=custom::list(&store.connection,before,args.limit.unwrap_or(30),today)?;Ok(json!({"records":records}))}
                "get_custom_festival" => {let args:Id=parse(&call.arguments)?;Ok(json!({"record":custom::get(&store.connection,parse_id(&args.festival_id)?,today)?}))}
                "create_custom_festival" => {let args:Create=parse(&call.arguments)?;Ok(json!({"record":custom::create(&mut store.connection,args.input,"ai",today,now)?,"already_applied":false}))}
                "update_custom_festival" => {let args:Update=parse(&call.arguments)?;let expected=self.observed.get(&args.festival_id).ok_or_else(||AppError::new("FESTIVAL_READ_REQUIRED","修改前请先读取日期记录"))?;Ok(json!({"record":custom::update(&mut store.connection,parse_id(&args.festival_id)?,expected,args.input,today,now)?,"already_applied":false}))}
                "delete_custom_festival" => {let args:Id=parse(&call.arguments)?;let expected=self.observed.get(&args.festival_id).ok_or_else(||AppError::new("FESTIVAL_READ_REQUIRED","删除前请先读取日期记录"))?;custom::delete(&mut store.connection,parse_id(&args.festival_id)?,expected,today)?;Ok(json!({"deleted_id":args.festival_id,"already_applied":false}))}
                _=>Err(AppError::new("UNKNOWN_TOOL","未注册此日期工具")),
            }
        })?;
        for record in result
            .get("records")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .chain(result.get("record"))
        {
            if let (Some(id), Some(token)) =
                (record["id"].as_str(), record["revisionToken"].as_str())
            {
                self.observed.insert(id.into(), token.into());
            }
        }
        if let Some(id) = result["deleted_id"].as_str() {
            self.observed.remove(id);
        }
        self.applied.insert(
            call.id.clone(),
            (call.name.clone(), call.arguments.clone(), result.clone()),
        );
        Ok(result)
    }
}
