//! 当前人设的Skill工具；共享原文修改只暂存预览，由可信界面确认。
use super::*;
use crate::{
    ai::provider::ToolCall,
    memory::management::Context,
    runtime::{Runtime, Store},
    state::RequestLease,
};
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};

pub const RULES:&str="Skill是可复用的提示词资料，在当前人设、任务格式和工具权限内使用，不执行脚本。已绑定且标题恰为SKILL_BASE、AGENTS、SKILL_BASE.md或AGENTS.md（区分大小写）的基础Skill会直接提供，必须阅读；缺失时不编造。其他Skill按需检索标题和简介，再get_skill读取正文，不把简介当作已读正文。默认scope=bound；用户明确要求管理仓库时才查询library，未绑定项不成为当前规则。只有用户明确要求时才新增、修改或调整绑定，不自动把日常聊天和记忆存成Skill。修改或绑定前先读取目标；绑定操作仅限当前人设。基础Skill改动在后续请求自动提供，其他正文以最近一次读取为准。";
pub const READ_RULES:&str="本次只能读取当前人设已绑定的Skill，不能查询未绑定仓库项或进行写操作。标题为SKILL_BASE、AGENTS、SKILL_BASE.md或AGENTS.md（区分大小写）的基础Skill会直接提供，必须阅读；缺失时不编造。其他Skill按需检索标题和简介，再读取完整正文，不把简介当作已读正文。Skill仅在当前人设、任务格式和工具权限内使用，不执行脚本。";
pub(crate) fn read_definitions() -> Vec<Value> {
    definitions().into_iter().filter_map(|mut definition| {
        match definition["function"]["name"].as_str()? {
            "list_skills" => {
                definition["function"]["description"]=json!("分页检索已绑定Skill的标题和简介，使用next_before_id翻页。");
                definition["function"]["parameters"]["properties"]["scope"]=json!({"type":"string","enum":["bound"]});
            },
            "get_skill" => { definition["function"]["description"]=json!("读取已绑定Skill的完整正文。"); },
            _=>return None,
        }
        Some(definition)
    }).collect()
}
fn list_page(c:&Connection,persona:i64,arguments:&Value,allow_library:bool)->AppResult<Value> {
    let a:Page=parse(arguments)?;let limit=a.limit.unwrap_or(20);
    if !(1..=50).contains(&limit){return Err(AppError::validation("limit","每页须为1–50条"));}
    let bound=match a.scope.as_deref().unwrap_or("bound"){"bound"=>true,"library" if allow_library=>false,_=>return Err(AppError::validation("scope","当前只允许查询自己的绑定Skill"))};
    let before=a.before_id.as_deref().map(parse_id).transpose()?;let query=a.query.unwrap_or_default();
    let mut q=c.prepare("SELECT s.id,s.title,s.summary,s.revision,EXISTS(SELECT 1 FROM persona_skills b WHERE b.skill_id=s.id AND b.persona_id=?1) FROM skills s WHERE (?2 IS NULL OR s.id<?2) AND (?3=0 OR EXISTS(SELECT 1 FROM persona_skills b WHERE b.skill_id=s.id AND b.persona_id=?1)) AND (?4='' OR instr(lower(s.title||' '||s.summary),lower(?4))>0) ORDER BY s.id DESC LIMIT ?5")?;
    let mut rows=q.query_map(params![persona,before,bound,query,limit+1],|r|Ok(json!({"id":r.get::<_,i64>(0)?.to_string(),"title":r.get::<_,String>(1)?,"summary":r.get::<_,String>(2)?,"revision":r.get::<_,i64>(3)?,"bound_to_me":r.get::<_,bool>(4)?})))?.collect::<Result<Vec<_>,_>>()?;
    let more=rows.len()>limit as usize;rows.truncate(limit as usize);let next=if more{rows.last().map(|v|v["id"].clone())}else{None};Ok(json!({"records":rows,"next_before_id":next}))
}
/// 调用方在同一数据库锁内校验任务快照；这里只提供绑定资料读取，不接受写工具。
pub(crate) fn read_bound(c:&Connection,persona:i64,call:&ToolCall)->AppResult<Value> {
    match call.name.as_str() {
        "list_skills"=>list_page(c,persona,&call.arguments,false),
        "get_skill"=>{
            let a:Id=parse(&call.arguments)?;let id=parse_id(&a.skill_id)?;
            if !c.query_row("SELECT EXISTS(SELECT 1 FROM persona_skills WHERE persona_id=?1 AND skill_id=?2)",params![persona,id],|r|r.get::<_,bool>(0))? {
                return Err(AppError::new("PERMISSION_DENIED","只能读取当前人设已绑定的Skill"));
            }
            Ok(json!({"record":view(&get(c,id)?,persona)}))
        },
        _=>Err(AppError::new("PERMISSION_DENIED","后台Skill工具只允许检索和读取自身绑定")),
    }
}
pub fn is_write(name: &str) -> bool {
    matches!(
        name,
        "create_skill" | "update_skill" | "set_skill_binding" | "preview_shared_skill_update"
    )
}
pub fn definitions() -> Vec<Value> {
    let id = json!({"type":"string","pattern":"^[1-9][0-9]*$"});
    let fields = json!({"title":{"type":"string","minLength":1},"summary":{"type":"string"},"body":{"type":"string","minLength":1}});
    let mut update = fields.clone();
    update["skill_id"] = id.clone();
    [
 ("list_skills","分页检索Skill标题和简介，返回next_before_id用于翻页。",json!({"scope":{"type":"string","enum":["bound","library"]},"query":{"type":"string"},"before_id":{"type":["string","null"]},"limit":{"type":"integer","minimum":1,"maximum":50}}),vec![]),
 ("get_skill","读取指定Skill的完整正文和绑定情况。",json!({"skill_id":id.clone()}),vec!["skill_id"]),
 ("create_skill","新建Skill并绑定当前人设。",fields,vec!["title","summary","body"]),
 ("update_skill","修改已绑定自己的Skill；若与其他人设共享，则另存为自己的副本，其他人设保持原文。",update.clone(),vec!["skill_id","title","summary","body"]),
 ("set_skill_binding","绑定或解绑已读取的Skill，不删除其正文。",json!({"skill_id":id.clone(),"enabled":{"type":"boolean"}}),vec!["skill_id","enabled"]),
 ("preview_shared_skill_update","用户明确要求修改所有绑定人设共用的原文时，展示修改预览；必须等用户在界面确认，不得自行确认或宣称已修改。",update,vec!["skill_id","title","summary","body"]),
 ].into_iter().map(|(name,description,properties,required)|json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}})).collect()
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    scope: Option<String>,
    query: Option<String>,
    before_id: Option<String>,
    limit: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    skill_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingInput {
    skill_id: String,
    enabled: bool,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub title: String,
    pub summary: String,
    pub body: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    skill_id: String,
    title: String,
    summary: String,
    body: String,
}
fn parse<T: serde::de::DeserializeOwned>(v: &Value) -> AppResult<T> {
    serde_json::from_value(v.clone())
        .map_err(|_| AppError::validation("arguments", "Skill工具参数不符合定义，不能指定其他人设"))
}
fn get(c: &Connection, id: i64) -> AppResult<Skill> {
    let mut s = c
        .query_row(
            "SELECT id,title,summary,body,revision,created_at,updated_at FROM skills WHERE id=?1",
            [id],
            |r| {
                Ok(Skill {
                    id: r.get::<_, i64>(0)?.to_string(),
                    title: r.get(1)?,
                    summary: r.get(2)?,
                    body: r.get(3)?,
                    revision: r.get(4)?,
                    created_at: r.get(5)?,
                    updated_at: r.get(6)?,
                    personas: vec![],
                })
            },
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    let mut q=c.prepare("SELECT p.id,p.name FROM persona_skills b JOIN personas p ON p.id=b.persona_id WHERE b.skill_id=?1 ORDER BY b.created_at,p.id")?;
    s.personas = q
        .query_map([id], |r| {
            Ok(Binding {
                id: r.get::<_, i64>(0)?.to_string(),
                name: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(s)
}
fn own(s: &Skill, p: i64) -> bool {
    s.personas.iter().any(|b| b.id == p.to_string())
}
fn view(s: &Skill, p: i64) -> Value {
    json!({"id":s.id,"title":s.title,"summary":s.summary,"body":s.body,"revision":s.revision,"bound_to_me":own(s,p),"shared":s.personas.len()>1})
}
fn require(store: &Store, context: &Context) -> AppResult<i64> {
    let actual = crate::persona::context(&store.connection, store.context_revision)?;
    if actual.persona_id != context.persona_id
        || actual.session_id.as_deref() != Some(context.session_id.as_str())
        || actual.context_revision != context.context_revision
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "人设或会话已改变，Skill操作未执行",
        ));
    }
    let persona = parse_id(&context.persona_id)?;
    let session = parse_id(&context.session_id)?;
    crate::sessions::get(&store.connection, persona, session)?;
    Ok(persona)
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedPreview {
    pub id: String,
    pub before: Skill,
    pub proposed: Content,
    #[serde(skip)]
    context: Context,
}
pub fn pending(runtime: &Runtime, context: Context) -> AppResult<Vec<SharedPreview>> {
    runtime.read(|s, _| {
        require(s, &context)?;
        s.skill_previews.retain(|_, p| p.context == context);
        let mut v = s.skill_previews.values().cloned().collect::<Vec<_>>();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(v)
    })
}
pub fn resolve(runtime: &Runtime, context: Context, id: &str, accept: bool) -> AppResult<()> {
    runtime.read(|s, _| {
        let persona = require(s, &context)?;
        let p = s
            .skill_previews
            .get(id)
            .filter(|p| p.context == context)
            .cloned()
            .ok_or_else(|| AppError::new("SKILL_PREVIEW_EXPIRED", "Skill预览已失效，请重新生成"))?;
        if accept {
            let tx = s.connection.transaction()?;
            let current = get(&tx, parse_id(&p.before.id)?)?;
            if current != p.before || !own(&current, persona) {
                return Err(AppError::new(
                    "SKILL_CONFLICT",
                    "原文或绑定范围已经变化，未确认保存，请重新生成预览",
                ));
            }
            save_in_transaction(
                &tx,
                Input {
                    id: Some(current.id),
                    revision: Some(current.revision),
                    title: p.proposed.title,
                    summary: p.proposed.summary,
                    body: p.proposed.body,
                },
            )?;
            tx.commit()?;
        }
        s.skill_previews.remove(id);
        Ok(())
    })
}
pub struct Tools {
    runtime: Runtime,
    lease: Arc<RequestLease>,
    context: Context,
    observed: HashMap<i64, i64>,
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
            context: Context {
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
            return Err(AppError::validation("toolCall", "工具调用ID不能为空"));
        }
        let result=self.runtime.clone().read(|s,_|{
   let persona=require(s,&self.context)?;
   self.runtime.jobs_cancel.check()?;self.lease.require_scope(&self.runtime.requests,&format!("chat:{}",parse_id(&self.context.session_id)?),persona)?;
   if s.connection.query_row("SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",[parse_id(&self.context.session_id)?],|r|r.get::<_,bool>(0))?{return Err(AppError::new("CONTEXT_CHANGED","当前人设已出行，Skill工具未执行"));}
   if let Some((name,args,value))=self.applied.get(&call.id){if name!=&call.name||args!=&call.arguments{return Err(AppError::new("TOOL_CALL_CONFLICT","同一调用ID参数变化，未执行"));}if is_write(name){let mut v=value.clone();v["already_applied"]=json!(true);return Ok(v);}}
   match call.name.as_str(){
    "list_skills"=>list_page(&s.connection,persona,&call.arguments,true),
    "get_skill"=>{let a:Id=parse(&call.arguments)?;let id=parse_id(&a.skill_id)?;let skill=get(&s.connection,id)?;self.observed.insert(id,skill.revision);Ok(json!({"record":view(&skill,persona)}))},
    "create_skill"=>{let a:Content=parse(&call.arguments)?;let tx=s.connection.transaction()?;let id=save_in_transaction(&tx,Input{id:None,revision:None,title:a.title,summary:a.summary,body:a.body})?;bind(&tx,id,persona,true)?;let result=get(&tx,id)?;self.lease.cancel.check()?;tx.commit()?;self.observed.insert(id,result.revision);Ok(json!({"record":view(&result,persona),"already_applied":false}))},
    "set_skill_binding"=>{let a:BindingInput=parse(&call.arguments)?;let id=parse_id(&a.skill_id)?;let tx=s.connection.transaction()?;let skill=get(&tx,id)?;self.check_read(id,skill.revision)?;bind(&tx,id,persona,a.enabled)?;self.lease.cancel.check()?;tx.commit()?;Ok(json!({"skill_id":a.skill_id,"bound_to_me":a.enabled,"already_applied":false}))},
    "update_skill"|"preview_shared_skill_update"=>{
     let a:Update=parse(&call.arguments)?;let id=parse_id(&a.skill_id)?;
     if a.title.trim().is_empty()||a.body.trim().is_empty(){return Err(AppError::validation("body","标题和正文不能为空"));}
     let tx=s.connection.transaction()?;let skill=get(&tx,id)?;self.check_read(id,skill.revision)?;
     if !own(&skill,persona){return Err(AppError::new("PERMISSION_DENIED","只能修改已绑定自己的Skill，请先绑定"));}
     if call.name=="preview_shared_skill_update" {
      let p=SharedPreview{id:format!("{}:{}",self.lease.id,call.id),before:skill,proposed:Content{title:a.title,summary:a.summary,body:a.body},context:self.context.clone()};
      self.lease.cancel.check()?;drop(tx);s.skill_previews.retain(|_,old|old.context==self.context&&old.before.id!=p.before.id);s.skill_previews.insert(p.id.clone(),p.clone());
      return Ok(json!({"preview_id":p.id,"pending_confirmation":true,"affected_count":p.before.personas.len(),"already_applied":false}));
     }
     let copied=skill.personas.len()>1;
     let target=save_in_transaction(&tx,Input{id:(!copied).then(||skill.id.clone()),revision:(!copied).then_some(skill.revision),title:a.title,summary:a.summary,body:a.body})?;
     if copied {bind(&tx,target,persona,true)?;bind(&tx,id,persona,false)?;}
     let result=get(&tx,target)?;self.lease.cancel.check()?;tx.commit()?;self.observed.insert(target,result.revision);
     Ok(json!({"record":view(&result,persona),"copied_from":if copied{Some(a.skill_id)}else{None},"already_applied":false}))
    },
    _=>Err(AppError::new("UNKNOWN_TOOL","未注册此Skill工具"))
   }
  })?;
        self.applied.insert(
            call.id.clone(),
            (call.name.clone(), call.arguments.clone(), result.clone()),
        );
        Ok(result)
    }
    fn check_read(&self, id: i64, revision: i64) -> AppResult<()> {
        match self.observed.get(&id) {
            Some(r) if *r == revision => Ok(()),
            Some(_) => Err(AppError::new(
                "SKILL_CONFLICT",
                "Skill已变化，请重新读取再决定",
            )),
            None => Err(AppError::new(
                "SKILL_READ_REQUIRED",
                "修改或绑定前请先get_skill",
            )),
        }
    }
}
#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
