//! 聊天持久化协调：活动片段仅保存在内存，终态事务成功后才通知窗口。
use super::*;
use crate::{models::RoundSnapshot, runtime::Store};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSnapshot {
    pub request_id: String,
    pub persona_id: String,
    pub session_id: String,
    pub context_revision: u64,
    pub round_id: String,
    pub seq: u64,
    pub partial_content: String,
    pub expression: Option<String>,
    pub round: RoundSnapshot,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub round_id: String,
    #[serde(flatten)]
    pub event: ReplyEvent,
    pub saved: Option<RoundSnapshot>,
}

pub(super) fn require_context(
    store: &Store,
    persona_id: i64,
    session_id: i64,
    revision: u64,
) -> AppResult<()> {
    let context = crate::persona::context(&store.connection, store.context_revision)?;
    if context.persona_id != persona_id.to_string()
        || context.session_id.as_deref() != Some(session_id.to_string().as_str())
        || context.context_revision != revision
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设或会话已经改变，请刷新后重试",
        ));
    }
    Ok(())
}

pub fn submit(
    runtime: &Runtime,
    persona_id: i64,
    session_id: i64,
    revision: u64,
    input: String,
) -> AppResult<(PreparedReply, RequestSnapshot)> {
    let (reply, snapshot) = prepare(runtime, persona_id, session_id, revision, input, true)?;
    Ok((reply, snapshot.ok_or_else(AppError::storage)?))
}

pub fn current(
    runtime: &Runtime,
    persona_id: i64,
    session_id: i64,
    revision: u64,
) -> AppResult<Option<RequestSnapshot>> {
    runtime.read(|store, _| {
        require_context(store, persona_id, session_id, revision)?;
        Ok(store.chat_requests.get(&session_id).cloned())
    })
}

/// 取消只发信号；租约保留至终态保存，重复取消不会产生第二条 AI 消息。
/// 已结束的回合直接返回数据库终态，不依赖运行期缓存。
pub fn cancel(
    runtime: &Runtime,
    persona_id: i64,
    session_id: i64,
    request_id: u64,
    round_id: i64,
) -> AppResult<RoundSnapshot> {
    runtime.read(|store, _| {
        let round =
            crate::sessions::round_snapshot(&store.connection, persona_id, session_id, round_id)?;
        if let Some(active) = store.chat_requests.get(&session_id) {
            if active.round_id == round_id.to_string()
                && active.request_id == request_id.to_string()
            {
                runtime.requests.cancel_request(
                    &format!("chat:{session_id}"),
                    request_id,
                    persona_id,
                )?;
            }
        }
        Ok(round)
    })
}

/// 窗口关闭或应用退出时同步收束已收到片段，避免等待网络任务导致数据丢失。
pub fn interrupt_active(runtime: &Runtime) -> AppResult<()> {
    runtime.read(|store, _| {
        let active: Vec<_> = store.chat_requests.values().cloned().collect();
        for snapshot in active {
            let persona_id = crate::error::parse_id(&snapshot.persona_id)?;
            let session_id = crate::error::parse_id(&snapshot.session_id)?;
            runtime.requests.cancel_key(&format!("chat:{session_id}"));
            crate::sessions::interrupt_round(
                &mut store.connection,
                persona_id,
                session_id,
                crate::error::parse_id(&snapshot.round_id)?,
                Some(&snapshot.partial_content),
                snapshot.expression.as_deref(),
            )?;
        }
        Ok(())
    })
}

pub async fn run_persisted(
    client: &AiClient,
    prepared: PreparedReply,
    mut sink: impl FnMut(ChatEvent),
) -> AppResult<RoundSnapshot> {
    let runtime = prepared.runtime.clone();
    let session_id = prepared.session_id;
    let persona_id = prepared.persona_id;
    let request_id = prepared.lease.id.to_string();
    let mut terminal = None;
    let result = run_reply(client, &prepared, |event| {
        if matches!(
            event.kind,
            ReplyEventKind::Finished { .. }
                | ReplyEventKind::Cancelled
                | ReplyEventKind::Failed { .. }
        ) {
            terminal = Some(event);
            return Ok(());
        }
        let round_id = runtime.read(|store, _| {
            prepared.lease.cancel.check()?;
            require_context(store, persona_id, session_id, prepared.context_revision)?;
            let active = store
                .chat_requests
                .get_mut(&session_id)
                .ok_or_else(AppError::not_found)?;
            if active.request_id != request_id {
                return Err(AppError::new("CONTEXT_CHANGED", "聊天请求已经改变"));
            }
            active.seq = event.seq;
            match &event.kind {
                ReplyEventKind::Text { text } => active.partial_content.push_str(text),
                ReplyEventKind::Expression { expression } => {
                    active.expression = Some(expression.clone())
                }
                _ => {}
            }
            Ok(active.round_id.clone())
        })?;
        sink(ChatEvent {
            round_id,
            event,
            saved: None,
        });
        Ok(())
    })
    .await;

    let mut event = terminal.ok_or_else(AppError::storage)?;
    let saved = runtime.read(|store, _| {
        let active = store
            .chat_requests
            .get(&session_id)
            .cloned()
            .ok_or_else(AppError::not_found)?;
        let round_id = crate::error::parse_id(&active.round_id)?;
        let complete = matches!(&result, Ok(choice) if choice.finish == Finish::Stop)
            && prepared.lease.cancel.check().is_ok()
            && require_context(store, persona_id, session_id, prepared.context_revision).is_ok();
        let saved = if complete {
            crate::sessions::complete_round(
                &mut store.connection,
                persona_id,
                session_id,
                round_id,
                &active.partial_content,
                active.expression.as_deref(),
            )?
        } else {
            crate::sessions::interrupt_round(
                &mut store.connection,
                persona_id,
                session_id,
                round_id,
                Some(&active.partial_content),
                active.expression.as_deref(),
            )?
        };
        if saved.round.status == crate::models::ChatRoundStatus::Interrupted {
            if matches!(
                event.kind,
                ReplyEventKind::Finished {
                    finish: Finish::Stop
                }
            ) {
                event.kind = ReplyEventKind::Cancelled;
            }
        }
        store.chat_requests.remove(&session_id);
        Ok((active.round_id, saved))
    });
    // 完成事件到达后允许立即发送下一轮；互斥一直覆盖到持久化结束。
    drop(prepared);
    match saved {
        Ok((round_id, saved)) => {
            if saved.round.status == crate::models::ChatRoundStatus::Completed {
                runtime.memory.wake.notify_one();
            }
            sink(ChatEvent {
                round_id,
                event,
                saved: Some(saved.clone()),
            });
            Ok(saved)
        }
        Err(error) => {
            event.kind = ReplyEventKind::Failed {
                code: error.code.into(),
                message: error.message.clone(),
            };
            let round_id = runtime
                .read(|s, _| {
                    Ok(s.chat_requests
                        .remove(&session_id)
                        .map(|r| r.round_id)
                        .unwrap_or_default())
                })
                .unwrap_or_default();
            sink(ChatEvent {
                round_id,
                event,
                saved: None,
            });
            Err(error)
        }
    }
}
