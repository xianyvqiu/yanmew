//! 最后完整回合重说：流式预览不落盘，回复与所有受影响记忆原子替换。
use super::*;
use crate::{memory::regeneration as memories, models::RoundSnapshot};
use rusqlite::params;

pub(crate) fn regenerate(
    runtime: &Runtime,
    persona: i64,
    session: i64,
    revision: u64,
    round: i64,
) -> AppResult<(PreparedReply, RequestSnapshot, memories::Snapshot)> {
    let snapshot = runtime.read(|store, _| {
        coordinator::require_context(store, persona, session, revision)?;
        if runtime.requests.is_active(&format!("chat:{session}"))? {
            return Err(AppError::new("REQUEST_BUSY", "回复正在生成，请稍后重说"));
        }
        let snapshot = memories::capture(&store.connection, persona, session, round)?;
        runtime
            .requests
            .cancel_key(&format!("short-memory:{round}"));
        runtime.requests.cancel_key(&format!("long-memory:{round}"));
        Ok(snapshot)
    })?;
    let (reply, receipt) = prepare(
        runtime,
        persona,
        session,
        revision,
        snapshot.round.user_message.content.clone(),
        true,
        Some(round),
        true,
    )?;
    Ok((reply, receipt.ok_or_else(AppError::storage)?, snapshot))
}

pub(crate) async fn run_regenerated(
    client: &AiClient,
    prepared: PreparedReply,
    snapshot: memories::Snapshot,
    mut sink: impl FnMut(ChatEvent),
) -> AppResult<RoundSnapshot> {
    let runtime = prepared.runtime.clone();
    let (persona, session, revision) = (
        prepared.persona_id,
        prepared.session_id,
        prepared.context_revision,
    );
    let request = prepared.lease.id.to_string();
    let round = snapshot.round.round.id.clone();
    let mut terminal = None;
    let mut seq = 0;
    let result=prepared.lease.cancel.wait(client.total_timeout(),async {
        let choice=run_reply(client,&prepared,|event| {
            seq=event.seq;
            if matches!(event.kind,ReplyEventKind::Finished{..}|ReplyEventKind::Cancelled|ReplyEventKind::Failed{..}) { terminal=Some(event);return Ok(()); }
            runtime.read(|store,_| {
                prepared.lease.cancel.check()?;
                coordinator::require_context(store,persona,session,revision)?;
                let active=store.chat_requests.get_mut(&session).filter(|a|a.request_id==request).ok_or_else(AppError::not_found)?;
                active.seq=event.seq;
                match &event.kind {
                    ReplyEventKind::Text{text}=>active.partial_content.push_str(text),
                    ReplyEventKind::Expression{expression}=>active.expression=Some(expression.clone()),
                    ReplyEventKind::ContextNotice{message,budget}=>{active.notice=Some(message.clone());active.budget=Some(budget.clone());},
                    ReplyEventKind::WebActivity{phase}=>active.web_activity=phase.clone(),
                    _=>{}
                }
                Ok(())
            })?;
            sink(ChatEvent{round_id:round.clone(),event,saved:None});
            Ok(())
        }).await?;
        if choice.finish!=Finish::Stop {return Err(AppError::new("REGENERATION_INCOMPLETE","重说未完整结束，原回复和记忆已保留"));}
        let replacement=memories::generate(&runtime,&snapshot,&prepared.lease.cancel,&snapshot.round.user_message.content,&choice.content).await?;
        runtime.read(|store,_| {
            runtime.jobs_cancel.check()?;
            prepared.lease.require_scope(&runtime.requests,&format!("chat:{session}"),persona)?;
            coordinator::require_context(store,persona,session,revision)?;
            let active=store.chat_requests.get(&session).filter(|a|a.request_id==request).cloned().ok_or_else(AppError::not_found)?;
            let stamp=runtime.clock.sample()?.utc.to_rfc3339_opts(chrono::SecondsFormat::Millis,true);
            let tx=store.connection.transaction()?;
            memories::apply(&tx,&snapshot,&replacement,&stamp)?;
            let assistant=snapshot.round.assistant_message.as_ref().ok_or_else(AppError::storage)?;
            tx.execute("UPDATE messages SET content=?1,expression=?2 WHERE id=?3 AND session_id=?4",params![choice.content,active.expression,crate::error::parse_id(&assistant.id)?,session])?;
            tx.execute("UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",params![stamp,session])?;
            let saved=crate::sessions::round_snapshot(&tx,persona,session,crate::error::parse_id(&round)?)?;
            prepared.lease.cancel.check()?;
            runtime.jobs_cancel.check()?;
            tx.commit()?;
            store.chat_requests.remove(&session);
            Ok(saved)
        })
    }).await;
    let mut event = terminal.unwrap_or(ReplyEvent {
        request_id: request.clone(),
        persona_id: persona.to_string(),
        session_id: session.to_string(),
        context_revision: revision,
        seq: seq + 1,
        kind: ReplyEventKind::Cancelled,
    });
    let saved = match &result {
        Ok(saved) => Some(saved.clone()),
        Err(error) => {
            event.kind = if error.code == "CANCELLED" {
                ReplyEventKind::Cancelled
            } else {
                ReplyEventKind::Failed {
                    code: error.code.into(),
                    message: format!("{}；原回复和记忆保留。", error.message),
                }
            };
            runtime
                .read(|store, _| {
                    if store
                        .chat_requests
                        .get(&session)
                        .is_some_and(|a| a.request_id == request)
                    {
                        store.chat_requests.remove(&session);
                    }
                    crate::sessions::round_snapshot(
                        &store.connection,
                        persona,
                        session,
                        crate::error::parse_id(&round)?,
                    )
                })
                .ok()
        }
    };
    drop(prepared);
    runtime.memory.wake.notify_one();
    sink(ChatEvent {
        round_id: round,
        event,
        saved,
    });
    result
}

#[cfg(test)]
#[path = "regeneration_tests.rs"]
mod tests;
