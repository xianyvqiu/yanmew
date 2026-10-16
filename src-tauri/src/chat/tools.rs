//! 单个聊天请求的有限工具循环；数据库操作在阻塞线程中执行，网络等待不持锁。
use super::*;
use crate::{ai::provider::ToolCall, events::{ChatToolEventSink, ReminderEventSink}, notebook_tools::NotebookTools,
    reminder_extract::ReminderCreation, travel_journal_tools::TravelJournalTools};
use serde_json::{json, Value};
use std::collections::HashMap;

struct SilentEvents;
impl ReminderEventSink for SilentEvents {
    fn reminders_changed(&self) {}
    fn reminder_failed(&self, _: &AppError) {}
}
impl ChatToolEventSink for SilentEvents { fn notebook_changed(&self) {} }

struct Session {
    notebook: NotebookTools,
    reminders: ReminderCreation,
    journals: TravelJournalTools,
    festivals: crate::festivals::tools::Tools,
    web: Option<crate::web_search::SearchSession>,
    events: Arc<dyn ChatToolEventSink>,
    runtime: Runtime,
    lease: Arc<RequestLease>,
    persona_id: i64,
    session_id: i64,
    revision: u64,
    evolution: Option<Arc<Mutex<evolution::Evolution>>>,
    // 跨工具类型检查 ID，防止同一个 ID 被改成另一种有副作用的调用。
    calls: HashMap<String, (String, Value)>,
}
impl Session {
    fn check_call(&mut self, call: &ToolCall) -> AppResult<()> {
        self.runtime.read(|store, _| {
            self.lease.require_conversation_scope(&self.runtime.requests, self.session_id, self.persona_id)?;
            self.runtime.jobs_cancel.check()?;
            coordinator::require_context(store, self.persona_id, self.session_id, self.revision)
        })?;
        if let Some((name, arguments)) = self.calls.get(&call.id) {
            if name != &call.name || arguments != &call.arguments {
                return Err(AppError::new("TOOL_CALL_CONFLICT", "调用 ID 已被其他名称或参数使用，未执行"));
            }
        }
        self.calls.insert(call.id.clone(), (call.name.clone(), call.arguments.clone()));
        Ok(())
    }
    fn execute(&mut self, call: &ToolCall) -> AppResult<Value> {
        self.check_call(call)?;
        match call.name.as_str() {
            "update_persona_prompt" => {
                let evolution = self.evolution.as_ref().ok_or_else(|| AppError::new("PERMISSION_DENIED", "本请求未开放人设自调"))?;
                evolution.lock().map_err(|_| AppError::storage())?.stage(call)
            }
            "read_notebook" | "update_notebook" => {
                let result = self.notebook.execute(call)?;
                if call.name == "update_notebook" && !result.already_applied { self.events.notebook_changed(); }
                serde_json::to_value(result).map_err(|_| AppError::storage())
            }
            "create_reminder" => serde_json::to_value(self.reminders.execute(call, &chrono::Local, self.events.as_ref())?)
                .map_err(|_| AppError::storage()),
            "request_accessory_craft" => serde_json::to_value(crate::accessories::craft::request(&self.runtime,&self.lease,
                crate::memory::management::Context {persona_id:self.persona_id.to_string(),session_id:self.session_id.to_string(),context_revision:self.revision},call,self.events.clone())?)
                .map_err(|_|AppError::storage()),
            "list_travel_journals" | "search_travel_journals" | "open_travel_journal" => self.journals.execute(call),
            "list_festival_catalog" | "list_session_festivals" | "get_session_festival" | "create_session_festival" | "update_session_festival" | "delete_session_festival" => {
                let result=self.festivals.execute(call)?;
                if crate::festivals::tools::is_write(&call.name) && result["already_applied"]!=true {self.events.festivals_changed(&self.session_id.to_string());}
                Ok(result)
            }
            _ => Err(AppError::new("UNKNOWN_TOOL", "未注册此工具，未执行任何操作")),
        }
    }
}

pub(super) async fn run(client: &AiClient, prepared: &PreparedReply,
    emit: &mut impl FnMut(ReplyEventKind) -> AppResult<()>) -> AppResult<Choice> {
    let runtime = prepared.runtime.clone();
    let lease = prepared.lease.clone();
    let evolution = prepared.evolution.clone();
    let (persona_id, session_id, revision) = (prepared.persona_id, prepared.session_id, prepared.context_revision);
    let events = prepared.tool_events.clone().unwrap_or_else(|| Arc::new(SilentEvents));
    let allow_web = !prepared.proactive && !prepared.continuation;
    let (mut session, rounds) = tauri::async_runtime::spawn_blocking(move || -> AppResult<_> {
        let rounds = runtime.read(|store, _| {
            let settings = crate::config::get(&store.connection)?;
            Ok(settings["tool_call_rounds"].parse::<usize>().map_err(|_| AppError::storage())?)
        })?;
        Ok((Session {
            notebook: NotebookTools::new(&runtime, &lease, persona_id, session_id, revision)?,
            reminders: ReminderCreation::new(&runtime, &lease, persona_id, session_id, revision)?,
            journals: TravelJournalTools::new(&runtime, &lease, persona_id, session_id, revision)?,
            festivals: crate::festivals::tools::Tools::new(&runtime, &lease, persona_id, session_id, revision),
            web: allow_web.then(||crate::web_search::SearchSession::new(&runtime)),
            events, runtime, lease, persona_id, session_id, revision, evolution, calls: HashMap::new(),
        }, rounds))
    }).await.map_err(|_| AppError::storage())??;
    let mut context=prepared.context.clone();
    super::media::enrich(client,prepared,&mut context).await?;
    if !prepared.continuation { context.enrich(prepared).await?; }
    let mut history = Vec::new();
    let mut definitions = crate::notebook_tools::definitions();
    definitions.extend(crate::travel_journal_tools::definitions());
    definitions.extend(crate::festivals::tools::definitions());
    history.push(Message::new("system",crate::festivals::tools::RULES));
    definitions.push(crate::reminder_extract::definition());
    definitions.extend(memory_tools::definitions());
    definitions.push(crate::accessories::craft::definition());
    history.push(Message::new("system",crate::accessories::craft::RULES));
    if session.web.as_ref().is_some_and(|web|web.available()) {
        definitions.extend(crate::web_search::definitions());
        history.push(Message::new("system",crate::web_search::RULES));
    }
    if prepared.evolution.is_some() { definitions.push(evolution::definition()); }
    if prepared.regenerating { definitions.retain(|tool| !matches!(tool["function"]["name"].as_str(),Some("update_notebook"|"create_reminder"|"request_accessory_craft"|"create_session_festival"|"update_session_festival"|"delete_session_festival"))); }
    if prepared.continuation {definitions.clear();}
    let mut content = String::new();
    let rounds=if prepared.continuation {0} else {rounds};
    let mut retried_capacity=false;
    let mut text_guard = guards::TextGuard::default();
    let mut tool_guard = guards::ToolGuard::default();
    for round in 0..=rounds {
        prepared.lease.cancel.check()?;
        if round == rounds && !prepared.continuation {
            history.push(Message::new("system", "本次工具轮数已达上限。不要再调用工具，只依据已经取得的结果回复；未完成的操作必须如实说明。"));
        }
        let mut choices = loop {
            prepared.lease.require_conversation_scope(&prepared.runtime.requests,prepared.session_id,prepared.persona_id)?;
            prepared.runtime.read(|store,_|coordinator::require_context(store,prepared.persona_id,prepared.session_id,prepared.context_revision))?;
            let (request,budget)=context.request(if round<rounds {definitions.clone()} else {vec![]},&history)?;
            emit(ReplyEventKind::ContextNotice {message:if budget.reduced {context::NOTICE.into()} else {String::new()},budget})?;
            let response=client.stream(&prepared.config, &request, &prepared.lease.cancel, |delta| match delta {
                Delta::Text { choice_index: 0, text } => {
                    // 先交给唯一协调器保存已收到片段，守卫错误再统一收束为中断。
                    let outcome = text_guard.observe(&text);
                    emit(ReplyEventKind::Text { text })?;
                    outcome
                },
                _ => Err(protocol()),
            }).await;
            match response {
                Err(error) if error.code=="CONTEXT_TOO_LARGE" && !retried_capacity && context.reduce_unknown() => {retried_capacity=true;}
                result=>break result?,
            }
        };
        if choices.len() != 1 { return Err(protocol()); }
        let mut choice = choices.remove(0);
        if choice.index != 0 { return Err(protocol()); }
        content.push_str(&choice.content);
        if choice.finish != Finish::ToolCalls {
            if choice.finish == Finish::Stop && choice.content.trim().is_empty() { return Err(protocol()); }
            prepared.lease.cancel.check()?;
            if choice.finish == Finish::Stop {
                let footer=session.web.as_ref().map(|web|web.footer(&content)).unwrap_or_default();
                if !footer.is_empty(){emit(ReplyEventKind::Text{text:footer.clone()})?;content.push_str(&footer);}
            }
            choice.content = content;
            return Ok(choice);
        }
        if round == rounds { return Err(AppError::new("TOOL_LIMIT_REACHED", "已达到工具轮数上限，模型仍要求继续调用；本轮回复未完成")); }
        history.push(Message::tool_request(&choice));
        let mut results=Vec::new();
        let mut observations = Vec::new();
        for call in choice.tools {
                if prepared.regenerating && matches!(call.name.as_str(),"update_notebook"|"create_reminder"|"request_accessory_craft"|"create_session_festival"|"update_session_festival"|"delete_session_festival") {return Err(AppError::new("PERMISSION_DENIED","重说不执行记事本写入、提醒创建、饰品制作或日期记录修改，旧操作保持不变"));}
                let result=if matches!(call.name.as_str(),"search_web"|"open_web_page") {
                    match session.check_call(&call) {
                        Ok(())=>{
                            let phase=if call.name=="search_web"{"searching"}else{"reading"};
                            emit(ReplyEventKind::WebActivity{phase:Some(phase.into())})?;
                            let result=match session.web.as_mut() {
                                Some(web)=>web.execute(client,&session.runtime,&call,&session.lease.cancel).await,
                                None=>Err(AppError::new("PERMISSION_DENIED","本请求不允许联网搜索或网页读取")),
                            };
                            emit(ReplyEventKind::WebActivity{phase:None})?;
                            session.check_call(&call)?;
                            result
                        },
                        Err(error)=>Err(error),
                    }
                } else if matches!(call.name.as_str(),"search_long_memories"|"open_long_memory"|"open_long_memory_sources"|"open_chat_round") {
                    match session.check_call(&call) {
                        Ok(())=>memory_tools::execute(&session.runtime,&session.lease,session.persona_id,session.session_id,session.revision,&call).await,
                        Err(error)=>Err(error),
                    }
                } else {
                    let owned=call.clone();
                    let (next,result)=tauri::async_runtime::spawn_blocking(move || {let result=session.execute(&owned);(session,result)}).await.map_err(|_|AppError::storage())?;
                    session=next;result
                };
                let value = match result {
                    Ok(value) => json!({"ok":true,"result":value}),
                    Err(error) if matches!(error.code, "CANCELLED" | "CONTEXT_CHANGED") => return Err(error),
                    Err(error) => json!({"ok":false,"error":error}),
                };
                observations.push((call.name.clone(), call.arguments.clone(), value.clone()));
                results.push(Message::tool_result(&call.id, value));
        }
        tool_guard.observe(observations)?;
        history.extend(results);
    }
    unreachable!("工具循环必须在上限轮返回")
}
