//! 无聊主动消息：唯一循环调度、持久化抽签位置、异步生成与原子发送。
use crate::{error::{AppError, AppResult, parse_id}, events::{ProactiveEventSink, ProactiveMessage},
    pet_store::{self, IdleBranch}, runtime::Runtime, state::RequestLease};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::params;
use std::sync::Arc;

pub(crate) const CHECK_INTERVAL_MS: i64 = 300_000;
const COOLDOWN_MINUTES: i64 = 30;

struct Work {
    prepared: crate::chat::PreparedReply,
    lease: Arc<RequestLease>,
    persona: i64,
    session: i64,
    revision: u64,
    next_check: i64,
    last_interacted: Option<String>,
}

/// 检查点已保存实际空闲时间。跨过多个时点只尝试一次，不补发历史抽签。
fn prepare(runtime: &Runtime) -> AppResult<Option<Work>> {
    runtime.read(|store, secrets| {
        runtime.jobs_cancel.check()?;
        let view = runtime.settle_pet(store, secrets)?;
        let Some(session) = view.session_id.as_deref() else { return Ok(None) };
        if !view.can_wander { return Ok(None) }
        let (persona, session) = (parse_id(&view.persona_id)?, parse_id(session)?);
        let key = format!("proactive:{session}");
        if runtime.requests.is_active(&key)? { return Ok(None) }
        let snapshot = pet_store::read(&store.connection, session)?;
        if snapshot.idle_branch != IdleBranch::Bored { return Ok(None) }
        let Some(next) = snapshot.proactive_next_check_idle_ms else { return Ok(None) };
        if snapshot.idle_elapsed_ms < next { return Ok(None) }
        let now = runtime.clock.sample()?.utc;
        let cooldown_ready = match &snapshot.last_proactive_sent_at {
            Some(value) => now.signed_duration_since(DateTime::parse_from_rfc3339(value).map_err(|_|AppError::storage())?.with_timezone(&Utc)) >= chrono::Duration::minutes(COOLDOWN_MINUTES),
            None => true,
        };
        let next_check = snapshot.idle_elapsed_ms.checked_add(CHECK_INTERVAL_MS).ok_or_else(AppError::storage)?;
        let hit = cooldown_ready && runtime.random.lock().map_err(|_|AppError::storage())?.below(100)? < 30;
        // 抽签没中、冷却未到、后续生成失败都保留下次时点；NULL 只在成功后写入。
        store.connection.execute("UPDATE session_pet_states SET proactive_next_check_idle_ms=?1,state_revision=state_revision+1,updated_at=?2 WHERE session_id=?3",
            params![next_check, now.to_rfc3339_opts(SecondsFormat::Millis,true), session])?;
        if !hit { return Ok(None) }
        let lease = Arc::new(runtime.requests.start(key, persona)?);
        let prepared = crate::chat::prepare_proactive(runtime, store, secrets, persona, session, lease.clone())?;
        Ok(Some(Work {prepared, lease, persona, session, revision:view.context_revision, next_check, last_interacted:snapshot.last_interacted_at}))
    })
}

fn save(runtime: &Runtime, work: &Work, content: &str, expression: Option<&str>) -> AppResult<ProactiveMessage> {
    runtime.read(|store, secrets| {
        runtime.jobs_cancel.check()?;
        let view = runtime.settle_pet(store, secrets)?;
        work.lease.require_scope(&runtime.requests, &format!("proactive:{}",work.session), work.persona)?;
        if !view.can_wander || view.context_revision != work.revision || view.session_id.as_deref() != Some(work.session.to_string().as_str()) || view.persona_id != work.persona.to_string() {
            return Err(AppError::new("CONTEXT_CHANGED", "主动消息上下文已变化，结果未保存"));
        }
        let snapshot = pet_store::read(&store.connection, work.session)?;
        if snapshot.idle_branch != IdleBranch::Bored || snapshot.proactive_next_check_idle_ms != Some(work.next_check) || snapshot.last_interacted_at != work.last_interacted {
            return Err(AppError::new("CONTEXT_CHANGED", "无聊周期已变化，结果未保存"));
        }
        let now = runtime.clock.sample()?.utc.to_rfc3339_opts(SecondsFormat::Millis,true);
        let tx = store.connection.transaction()?;
        let message = crate::sessions::append_proactive_in_transaction(&tx, work.persona, work.session, content, expression, &now)?;
        tx.execute("UPDATE session_pet_states SET proactive_next_check_idle_ms=NULL,last_proactive_sent_at=?1,state_revision=state_revision+1,updated_at=?1 WHERE session_id=?2", params![now,work.session])?;
        tx.commit()?;
        Ok(ProactiveMessage {persona_id:work.persona.to_string(), session_id:work.session.to_string(), context_revision:work.revision, message})
    })
}

pub async fn tick(runtime: &Runtime, events: Arc<dyn ProactiveEventSink>) {
    let worker = runtime.clone();
    let result = tauri::async_runtime::spawn_blocking(move || prepare(&worker)).await;
    match result {
        Ok(Ok(Some(mut work))) => {
            work.prepared.set_tool_events(events.clone());
            let runtime = runtime.clone();
            tauri::async_runtime::spawn(async move {
                let mut expression = None;
                let result = match runtime.ai.as_ref() {
                    Ok(client) => crate::chat::run_reply(client, &work.prepared, |event| {
                        if let crate::chat::ReplyEventKind::Expression {expression: value} = event.kind { expression=Some(value); }
                        Ok(())
                    }).await,
                    Err(error) => Err(error.clone()),
                };
                // 截断和部分结果不算成功，不写成一个完整主动消息。
                if let Ok(choice) = result {
                    if choice.finish == crate::ai::provider::Finish::Stop && !choice.content.trim().is_empty() {
                        let worker=runtime.clone();
                        let saved=tauri::async_runtime::spawn_blocking(move || save(&worker,&work,&choice.content,expression.as_deref())).await;
                        match saved {
                            Ok(Ok(message)) => events.proactive_message(&message),
                            Ok(Err(error)) if !matches!(error.code,"CANCELLED"|"CONTEXT_CHANGED") => events.proactive_failed(&error),
                            Err(_) => events.proactive_failed(&AppError::storage()),
                            _ => {}
                        }
                    }
                }
            });
        }
        Ok(Err(error)) if !matches!(error.code,"CANCELLED"|"CONTEXT_CHANGED"|"REQUEST_BUSY") => events.proactive_failed(&error),
        _ => {}
    }
}

#[cfg(test)]
mod tests;
