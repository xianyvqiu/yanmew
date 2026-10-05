//! 历史原文编辑与底部删除；不自动重写已有记忆。
use super::*;
use crate::runtime::Runtime;

fn current(
    runtime: &Runtime,
    store: &crate::runtime::Store,
    persona: i64,
    session: i64,
    revision: u64,
) -> AppResult<()> {
    runtime.jobs_cancel.check()?;
    let context = crate::persona::context(&store.connection, store.context_revision)?;
    if context.persona_id != persona.to_string()
        || context.session_id != Some(session.to_string())
        || context.context_revision != revision
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设或会话已经改变，请刷新后重试",
        ));
    }
    require_session(&store.connection, persona, session)
}
fn read(connection: &Connection, session: i64, message: i64) -> AppResult<Message> {
    connection.query_row("SELECT id,session_id,round_id,role,content,expression,created_at FROM messages WHERE id=?1 AND session_id=?2", params![message,session], message_from_row).optional()?.ok_or_else(AppError::not_found)
}
fn expected(message: &Message, content: &str) -> AppResult<()> {
    if message.content != content {
        return Err(AppError::new(
            "MESSAGE_CONFLICT",
            "消息已改变，请重新加载后再操作",
        ));
    }
    Ok(())
}
pub(crate) fn edit(
    runtime: &Runtime,
    persona: i64,
    session: i64,
    revision: u64,
    message: i64,
    old: &str,
    content: &str,
) -> AppResult<Message> {
    if content.trim().is_empty() {
        return Err(AppError::validation("content", "消息正文不能为空"));
    }
    let result = runtime.read(|store,_| {
        current(runtime,store,persona,session,revision)?;
        if runtime.requests.is_active(&format!("chat:{session}"))? { return Err(AppError::new("REQUEST_BUSY", "回复正在生成，请暂停后再编辑消息")); }
        let tx = store.connection.transaction()?;
        expected(&read(&tx,session,message)?,old)?;
        tx.execute("UPDATE messages SET content=?1 WHERE id=?2 AND session_id=?3",params![content,message,session])?;
        // 手动编辑后的长度停止回合不再满足原前缀续写资格。
        tx.execute("UPDATE chat_rounds SET stop_reason='user_cancelled' WHERE id=(SELECT round_id FROM messages WHERE id=?1) AND status='interrupted' AND stop_reason='length_limit'",[message])?;
        tx.execute("UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",params![now(),session])?;
        let saved=read(&tx,session,message)?;
        tx.commit()?;
        runtime.requests.cancel_key(&format!("session-title:{session}"));
        Ok(saved)
    })?;
    runtime.memory.wake.notify_one();
    Ok(result)
}
pub(crate) fn delete_tail(
    runtime: &Runtime,
    persona: i64,
    session: i64,
    revision: u64,
    message: i64,
    old: &str,
) -> AppResult<()> {
    runtime.read(|store, _| {
        current(runtime, store, persona, session, revision)?;
        let tx = store.connection.transaction()?;
        let target = read(&tx, session, message)?;
        expected(&target, old)?;
        let tail: i64 = tx.query_row(
            "SELECT id FROM messages WHERE session_id=?1 ORDER BY id DESC LIMIT 1",
            [session],
            |r| r.get(0),
        )?;
        if tail != message {
            return Err(AppError::new(
                "ROUND_NOT_TAIL",
                "只能删除聊天底部消息或回合，请重新加载",
            ));
        }
        let round = target
            .round_id
            .as_deref()
            .map(crate::error::parse_id)
            .transpose()?;
        let stamp = now();
        if let Some(id) = round {
            let snapshot = round_snapshot(&tx, persona, session, id)?;
            if snapshot.round.status == ChatRoundStatus::Completed {
                crate::memory::rollback_trigger(&tx, session, id, &stamp)?;
            }
            // 已通过目标/尾部校验才取消；同一锁内禁止迟到保存复活被删除回合。
            runtime.requests.cancel_key(&format!("chat:{session}"));
            tx.execute(
                "DELETE FROM chat_rounds WHERE id=?1 AND session_id=?2",
                params![id, session],
            )?;
        } else {
            tx.execute(
                "DELETE FROM messages WHERE id=?1 AND session_id=?2",
                params![message, session],
            )?;
        }
        tx.execute(
            "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
            params![stamp, session],
        )?;
        tx.commit()?;
        if let Some(id) = round {
            if store
                .chat_requests
                .get(&session)
                .is_some_and(|active| active.round_id == id.to_string())
            {
                store.chat_requests.remove(&session);
            }
            runtime.requests.cancel_key(&format!("short-memory:{id}"));
            runtime.requests.cancel_key(&format!("long-memory:{id}"));
        }
        runtime
            .requests
            .cancel_key(&format!("session-title:{session}"));
        Ok(())
    })?;
    runtime.memory.wake.notify_one();
    Ok(())
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
