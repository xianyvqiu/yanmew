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
    skills: Option<crate::skills::tools::Tools>,
    skills_read_only: bool,
    real_money: Option<crate::real_money::tools::Tools>,
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
    visual_results: HashMap<String, AppResult<Value>>,
    base_image_results:HashMap<String,AppResult<(Value,Option<Message>)>>,
    selected_expression:bool,
    image_mode:bool,
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
            "get_weather_warnings"|"get_typhoon_tracking" => super::weather_tools::read(&self.runtime,call),
            "query_meow_ledger" => {
                if self.skills.is_none() { return Err(AppError::new("PERMISSION_DENIED", "只有普通聊天开放喵币账单查询")); }
                let input: crate::currency::ledger::Query = serde_json::from_value(call.arguments.clone())
                    .map_err(|_| AppError::new("INVALID_INPUT", "喵币账单查询参数不完整或格式不正确"))?;
                self.runtime.read(|store, _| {
                    let report = crate::currency::ledger::read(&store.connection, input)?;
                    let value = serde_json::to_value(report).map_err(|_| AppError::storage())?;
                    Ok(json!({"currency":"喵币（非现实钱）", "balance":value["balance"], "summary":value["summary"],
                        "entries":value["entries"], "page":value["page"], "totalPages":value["totalPages"],
                        "rangeStart":value["rangeStart"], "rangeEnd":value["rangeEnd"]}))
                })
            }
            "list_accessories"|"list_pet_expressions"|"set_pet_expression"=>{let value=super::visual_tools::local(&self.runtime,self.persona_id,self.session_id,self.revision,call)?;if call.name=="set_pet_expression"{self.selected_expression=true;}Ok(value)},
            "list_real_money_assets"|"list_real_money_categories"|"save_real_money_category"|"delete_real_money_category"|"record_real_money"|"query_real_money_entries"=>{
                let value=self.real_money.as_mut().ok_or_else(||AppError::new("PERMISSION_DENIED","只有普通聊天开放现实记账"))?.execute(call)?;
                if crate::real_money::tools::is_write(&call.name)&&value["alreadyApplied"]!=true{self.events.real_money_changed();}Ok(value)
            }
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
            "list_skills"|"get_skill"|"create_skill"|"update_skill"|"set_skill_binding"|"preview_shared_skill_update" => {
                if self.skills_read_only {
                    return self.runtime.read(|s,_| {
                        self.lease.require_conversation_scope(&self.runtime.requests,self.session_id,self.persona_id)?;
                        coordinator::require_context(s,self.persona_id,self.session_id,self.revision)?;
                        crate::skills::tools::read_bound(&s.connection,self.persona_id,call)
                    });
                }
                let result=self.skills.as_mut().ok_or_else(||AppError::new("PERMISSION_DENIED","只有普通聊天开放Skill工具"))?.execute(call)?;
                if crate::skills::tools::is_write(&call.name)&&result["already_applied"]!=true{self.events.skills_changed();}
                Ok(result)
            }
            "list_festival_catalog" | "list_custom_festivals" | "get_custom_festival" | "create_custom_festival" | "update_custom_festival" | "delete_custom_festival" => {
                let result=self.festivals.execute(call)?;
                if crate::festivals::tools::is_write(&call.name) && result["already_applied"]!=true {self.events.festivals_changed();}
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
    let allow_skills = !prepared.proactive && !prepared.continuation;
    let skills_read_only = prepared.proactive;
    let user_input = prepared.messages.iter().rev().find(|message| message.role == "user")
        .map(|message| message.content.clone()).unwrap_or_default();
    let native_config=crate::providers::ConnectionTest{base_url:prepared.config.base_url.clone(),model:prepared.config.model.clone(),api_key:prepared.config.api_key.clone(),max_output_tokens:prepared.config.max_output_tokens,context_window_override:prepared.config.context_window_override};
    let (mut session, rounds) = tauri::async_runtime::spawn_blocking(move || -> AppResult<_> {
        let rounds = runtime.read(|store, _| {
            let settings = crate::config::get(&store.connection)?;
            Ok(settings["tool_call_rounds"].parse::<usize>().map_err(|_| AppError::storage())?)
        })?;
        let image_mode=runtime.read(|s,_|Ok(crate::pet_creation::chat_choices(&s.connection,persona_id)?.is_some()))?;
        Ok((Session {
            notebook: NotebookTools::new(&runtime, &lease, persona_id, session_id, revision)?,
            reminders: ReminderCreation::new(&runtime, &lease, persona_id, session_id, revision)?,
            journals: TravelJournalTools::new(&runtime, &lease, persona_id, session_id, revision)?,
            festivals: crate::festivals::tools::Tools::new(&runtime, &lease, persona_id, session_id, revision),
            skills: allow_skills.then(||crate::skills::tools::Tools::new(&runtime,&lease,persona_id,session_id,revision)),
            skills_read_only,
            real_money: allow_skills.then(||crate::real_money::tools::Tools::new(&runtime,&lease,persona_id,session_id,revision)),
            web: allow_web.then(||crate::web_search::SearchSession::new(&runtime, &user_input).with_native(native_config)),
            events, runtime, lease, persona_id, session_id, revision, evolution, calls: HashMap::new(),visual_results:HashMap::new(),base_image_results:HashMap::new(),selected_expression:false,image_mode,
        }, rounds))
    }).await.map_err(|_| AppError::storage())??;
    let mut context=prepared.context.clone();
    super::media::enrich(client,prepared,&mut context).await?;
    if !prepared.continuation { context.enrich(prepared).await?; }
    let mut history = Vec::new();
    let mut definitions = crate::notebook_tools::definitions();
    if !prepared.continuation {definitions.extend(super::weather_tools::definitions());history.push(Message::new("system",super::weather_tools::RULES));}
    if allow_skills {
        definitions.push(json!({"type":"function","function":{"name":"query_meow_ledger",
            "description":"只读查询所有人设共用的喵币余额及账单，可按日期、收入/支出、分类、小游戏和关键词分页。不修改钱包，不查询现实人民币账本。",
            "parameters":{"type":"object","additionalProperties":false,"properties":{
                "start":{"type":["string","null"],"description":"开始日期YYYY-MM-DD，null不限"},
                "end":{"type":["string","null"],"description":"结束日期YYYY-MM-DD，null不限"},
                "direction":{"type":"string","enum":["all","income","expense"]},
                "reason":{"type":"string","description":"分类代码daily_sign_in/sign_in_festival/food_purchase/accessory_craft/accessory_sell/plugin_game，空字符串为全部"},
                "pluginId":{"type":"string","description":"小游戏ID，空字符串为全部"},"search":{"type":"string"},
                "ascending":{"type":"boolean"},"page":{"type":"integer","minimum":1}},
                "required":["start","end","direction","reason","pluginId","search","ascending","page"]}}}));
    }
    if allow_skills {definitions.extend(super::visual_tools::definitions(session.image_mode));history.push(Message::new("system",super::visual_tools::RULES));history.push(Message::new("system",if session.image_mode {"当前为图片桌宠模式。需要了解自己实际的默认外观时可调用view_pet_base_image查看正式初始图，不凭人设文字臆测图片细节。聊天时先通过list_pet_expressions查询已有表情，再调用set_pet_expression按本次语境和情绪选择；也可在本轮随情绪继续切换。候选来自当前人设整个已启用且有正式图的表情库，包含用户新增表情，不受固定情绪或动作类别限制；按需分页查询。不要默认选自然交谈。自然交谈仅在没有其他更合适表情时兜底。不要用进食图虚构真实投喂。"}else{"当前为颜文字桌宠。聊天表情须结合当前语境临时生成，不查询或抽取固定颜文字表情库；可通过set_pet_expression在本轮改变临时表情，不写回人设库。"}));}
    if session.real_money.is_some(){definitions.extend(crate::real_money::tools::definitions());history.push(Message::new("system",crate::real_money::tools::RULES));}
    definitions.extend(crate::travel_journal_tools::definitions());
    definitions.extend(crate::festivals::tools::definitions());
    if session.skills.is_some(){definitions.extend(crate::skills::tools::definitions());history.push(Message::new("system",crate::skills::tools::RULES));}
    else if session.skills_read_only {definitions.extend(crate::skills::tools::read_definitions());history.push(Message::new("system",crate::skills::tools::READ_RULES));}
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
    if prepared.regenerating { definitions.retain(|tool| {let name=tool["function"]["name"].as_str().unwrap_or("");!crate::skills::tools::is_write(name)&&!crate::real_money::tools::is_write(name)&&!matches!(name,"update_notebook"|"create_reminder"|"request_accessory_craft"|"generate_chat_image"|"remove_image_background"|"create_custom_festival"|"update_custom_festival"|"delete_custom_festival")}); }
    if prepared.continuation {definitions.clear();}
    context.add_stable_rules(std::mem::take(&mut history));
    let mut usage = crate::ai::usage::UsageTotals::default();
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
        let (mut choices, mut budget) = loop {
            prepared.lease.require_conversation_scope(&prepared.runtime.requests,prepared.session_id,prepared.persona_id)?;
            prepared.runtime.read(|store,_|coordinator::require_context(store,prepared.persona_id,prepared.session_id,prepared.context_revision))?;
            let (request,mut budget)=context.request(if round<rounds {definitions.clone()} else {vec![]},&history)?;
            budget.provider_usage=Some(usage.clone());
            emit(ReplyEventKind::ContextNotice {message:if budget.reduced {context::NOTICE.into()} else {String::new()},budget:budget.clone()})?;
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
                result=>break (result?,budget),
            }
        };
        if choices.len() != 1 { return Err(protocol()); }
        let mut choice = choices.remove(0);
        if choice.index != 0 { return Err(protocol()); }
        usage.add(choice.usage.as_ref());
        budget.provider_usage=Some(usage.clone());
        emit(ReplyEventKind::ContextNotice {message:if budget.reduced {context::NOTICE.into()} else {String::new()},budget})?;
        content.push_str(&choice.content);
        if choice.finish != Finish::ToolCalls {
            if choice.finish == Finish::Stop && choice.content.trim().is_empty() { return Err(protocol()); }
            prepared.lease.cancel.check()?;
            if choice.finish == Finish::Stop {
                if session.image_mode && !session.selected_expression && !prepared.proactive {if let Some(expression)=super::visual_tools::fallback(&session.runtime,session.persona_id,session.session_id,session.revision)?{emit(ReplyEventKind::Expression{expression})?;}}
                let footer=session.web.as_ref().map(|web|web.footer(&content)).unwrap_or_default();
                if !footer.is_empty(){emit(ReplyEventKind::Text{text:footer.clone()})?;content.push_str(&footer);}
            }
            choice.content = content;
            return Ok(choice);
        }
        if round == rounds { return Err(AppError::new("TOOL_LIMIT_REACHED", "已达到工具轮数上限，模型仍要求继续调用；本轮回复未完成")); }
        history.push(Message::tool_request(&choice));
        let mut results=Vec::new();
        let mut reference_images=Vec::new();
        let mut observations = Vec::new();
        for call in choice.tools {
                if prepared.regenerating && (crate::real_money::tools::is_write(&call.name)||crate::skills::tools::is_write(&call.name)||matches!(call.name.as_str(),"update_notebook"|"create_reminder"|"request_accessory_craft"|"generate_chat_image"|"remove_image_background"|"create_custom_festival"|"update_custom_festival"|"delete_custom_festival")) {return Err(AppError::new("PERMISSION_DENIED","重说不执行Skill修改、记事本写入、提醒创建、饰品制作或日期记录修改，旧操作保持不变"));}
                let result=if call.name=="view_pet_base_image" {
                    if !allow_skills||!session.image_mode{Err(AppError::new("PERMISSION_DENIED","只有图片模式的普通聊天开放默认形象查看"))}else{
                        session.check_call(&call)?;
                        let value=if let Some(saved)=session.base_image_results.get(&call.id){saved.clone()}else{let value=super::self_image::view(client,&session.runtime,&session.lease,session.persona_id,session.session_id,session.revision,&prepared.config,&call).await;session.base_image_results.insert(call.id.clone(),value.clone());value};
                        value.map(|(value,image)|{if let Some(image)=image{reference_images.push(image);}value})
                    }
                } else if matches!(call.name.as_str(),"list_generated_images"|"view_generated_image"|"inspect_image_transparency"|"remove_image_background") {
                    if !allow_skills {Err(AppError::new("PERMISSION_DENIED","仅普通聊天开放生成图片工具"))}else{
                        session.check_call(&call)?;
                        let value=if let Some(saved)=session.base_image_results.get(&call.id){saved.clone()}else{let value=super::image_tools::execute(client,&session.runtime,&session.lease,session.persona_id,session.session_id,session.revision,&prepared.config,&call).await;session.base_image_results.insert(call.id.clone(),value.clone());value};
                        value.map(|(value,image)|{if let Some(image)=image{reference_images.push(image);}value})
                    }
                } else if call.name=="generate_chat_image" {
                    if !allow_skills {Err(AppError::new("PERMISSION_DENIED","仅普通聊天允许按用户要求生图"))}else{session.check_call(&call)?;if let Some(value)=session.visual_results.get(&call.id){value.clone()}else{let result=super::visual_tools::generate(client,&session.runtime,&session.lease,session.persona_id,session.session_id,session.revision,&call).await;session.visual_results.insert(call.id.clone(),result.clone());result}}
                } else if matches!(call.name.as_str(),"search_web"|"open_web_page") {
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
                    Ok(value) => {if call.name=="set_pet_expression"{if let Some(expression)=value["expression"].as_str(){emit(ReplyEventKind::Expression{expression:expression.into()})?;}}json!({"ok":true,"result":value})},
                    Err(error) if matches!(error.code, "CANCELLED" | "CONTEXT_CHANGED") => return Err(error),
                    Err(error) => json!({"ok":false,"error":error}),
                };
                observations.push((call.name.clone(), call.arguments.clone(), value.clone()));
                results.push(Message::tool_result(&call.id, value));
        }
        tool_guard.observe(observations)?;
        history.extend(results);
        history.extend(reference_images);
    }
    unreachable!("工具循环必须在上限轮返回")
}
