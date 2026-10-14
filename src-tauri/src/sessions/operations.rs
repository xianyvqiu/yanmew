//! 会话删除的运行时协调，使用同一 store 锁隔离提交与迟到生成结果。
use super::*;
use crate::{models::SessionDeleteResult, persona::Context, runtime::Runtime};

pub(crate) struct DeleteOutcome {
    pub result: SessionDeleteResult,
    pub context: Context,
    pub context_changed: bool,
    /// 删除已提交后恢复桌宠失败只报告错误，不把成功删除误报为失败。
    pub pet_error: Option<AppError>,
}

pub(crate) fn delete_in_context(
    runtime: &Runtime,
    persona_id: i64,
    session_id: i64,
    context_revision: u64,
) -> AppResult<DeleteOutcome> {
    runtime.read(|store, secrets| {
        runtime.jobs_cancel.check()?;
        let before = crate::persona::context(&store.connection, store.context_revision)?;
        if before.persona_id != persona_id.to_string()
            || before.context_revision != context_revision
        {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前人设或会话已经改变，请刷新后重试",
            ));
        }
        require_session(&store.connection, persona_id, session_id)?;
        let changed = before.session_id.as_deref() == Some(session_id.to_string().as_str());
        let next_revision = if changed {
            store
                .context_revision
                .checked_add(1)
                .ok_or_else(AppError::storage)?
        } else {
            store.context_revision
        };
        // 删除当前会话前结算它的运行时间；不相关删除不结算、重置或中断当前会话。
        if changed {
            runtime.settle_pet(store, secrets)?;
        }
        let round_ids = {
            let mut statement = store
                .connection
                .prepare("SELECT id FROM chat_rounds WHERE session_id=?1")?;
            let ids = statement
                .query_map([session_id], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        runtime.requests.cancel_key(&format!("chat:{session_id}"));
        let current_session_id = delete(&mut store.connection, persona_id, session_id)?;

        // 事务已提交；仅清理目标任务，不使用 cancel_persona 波及同人设其他会话。
        store.chat_requests.remove(&session_id);
        runtime
            .requests
            .cancel_key(&format!("session-title:{session_id}"));
        for id in round_ids {
            runtime.requests.cancel_key(&format!("short-memory:{id}"));
            runtime.requests.cancel_key(&format!("long-memory:{id}"));
        }
        if changed {
            crate::memory::management::discard(runtime);
            store.accessory_crafts.discard();
            store.context_revision = next_revision;
            store.pet.reset_context();
        }
        runtime.memory.wake.notify_one();
        let context = Context {
            persona_id: before.persona_id,
            session_id: current_session_id.clone(),
            context_revision: store.context_revision,
        };
        let pet_error = if changed {
            runtime.settle_pet(store, secrets).err()
        } else {
            None
        };
        Ok(DeleteOutcome {
            result: SessionDeleteResult {
                current_session_id,
                context_revision: store.context_revision,
            },
            context,
            context_changed: changed,
            pet_error,
        })
    })
}

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;
