//! 会话桌宠状态仓储。调用者测量有效运行区间；这里不读取墙上时钟补算离线时间。
//! 后续调度器和互动服务共用本模块，不允许界面提交计时增量或直接写状态表。

use crate::error::{AppError, AppResult};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleBranch {
    None,
    Bored,
    Sleeping,
}

impl IdleBranch {
    fn stored(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bored => "bored",
            Self::Sleeping => "sleeping",
        }
    }

    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "none" => Ok(Self::None),
            "bored" => Ok(Self::Bored),
            "sleeping" => Ok(Self::Sleeping),
            _ => Err(AppError::storage()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetSnapshot {
    pub session_id: String,
    pub hunger_elapsed_ms: i64,
    pub idle_elapsed_ms: i64,
    pub idle_branch: IdleBranch,
    pub sleep_until: Option<String>,
    pub last_fed_at: Option<String>,
    pub last_interacted_at: Option<String>,
    pub state_revision: i64,
    pub updated_at: String,
}

/// 后端在取得当前上下文后创建，不能由前端自行指定 persona/session 归属。
#[derive(Clone, Copy)]
pub struct PetWriteScope {
    pub persona_id: i64,
    pub session_id: i64,
    pub context_revision: u64,
    pub state_revision: i64,
}

fn time_text(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn read(connection: &Connection, session_id: i64) -> AppResult<PetSnapshot> {
    let stored = connection
        .query_row(
            "SELECT session_id,hunger_elapsed_ms,idle_elapsed_ms,idle_branch,sleep_until,
                last_fed_at,last_interacted_at,state_revision,updated_at
         FROM session_pet_states WHERE session_id=?1",
            [session_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, String>(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    Ok(PetSnapshot {
        session_id: stored.0.to_string(),
        hunger_elapsed_ms: stored.1,
        idle_elapsed_ms: stored.2,
        idle_branch: IdleBranch::parse(&stored.3)?,
        sleep_until: stored.4,
        last_fed_at: stored.5,
        last_interacted_at: stored.6,
        state_revision: stored.7,
        updated_at: stored.8,
    })
}

pub(crate) fn validate_scope(
    connection: &Connection,
    actual_context_revision: u64,
    scope: PetWriteScope,
) -> AppResult<PetSnapshot> {
    let context = crate::persona::context(connection, actual_context_revision)?;
    if context.context_revision != scope.context_revision
        || context.persona_id != scope.persona_id.to_string()
        || context.session_id.as_deref() != Some(scope.session_id.to_string().as_str())
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设或会话已变化，请重新读取桌宠状态",
        ));
    }
    let snapshot = read(connection, scope.session_id)?;
    if snapshot.state_revision != scope.state_revision {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "桌宠状态已更新，旧操作未写入",
        ));
    }
    Ok(snapshot)
}

fn require_at_home(connection: &Connection, session_id: i64) -> AppResult<()> {
    let away = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",
        [session_id],
        |row| row.get::<_, bool>(0),
    )?;
    if away {
        return Err(AppError::new("PET_AWAY", "它出门啦，回来以后再互动吧"));
    }
    Ok(())
}

fn save(connection: &Connection, snapshot: &mut PetSnapshot, now: DateTime<Utc>) -> AppResult<()> {
    let next_revision = snapshot
        .state_revision
        .checked_add(1)
        .ok_or_else(AppError::storage)?;
    let updated_at = time_text(now);
    let changed = connection.execute(
        "UPDATE session_pet_states SET hunger_elapsed_ms=?1,idle_elapsed_ms=?2,
         idle_branch=?3,sleep_until=?4,last_fed_at=?5,last_interacted_at=?6,
         state_revision=?7,updated_at=?8 WHERE session_id=?9 AND state_revision=?10",
        params![
            snapshot.hunger_elapsed_ms,
            snapshot.idle_elapsed_ms,
            snapshot.idle_branch.stored(),
            snapshot.sleep_until,
            snapshot.last_fed_at,
            snapshot.last_interacted_at,
            next_revision,
            updated_at,
            snapshot.session_id,
            snapshot.state_revision
        ],
    )?;
    if changed != 1 {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "桌宠状态已变化，旧操作未写入",
        ));
    }
    snapshot.state_revision = next_revision;
    snapshot.updated_at = updated_at;
    Ok(())
}

/// 只接受后端已测量的有效增量；无互动区间不能长于本次有效运行区间。
/// 零增量不写盘；同一版本重复提交被拒绝，睡眠中不能增加无互动周期。
pub fn checkpoint(
    connection: &mut Connection,
    actual_context_revision: u64,
    scope: PetWriteScope,
    effective_elapsed: Duration,
    idle_elapsed: Duration,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    let transaction = connection.transaction()?;
    let snapshot = checkpoint_in_transaction(
        &transaction,
        actual_context_revision,
        scope,
        effective_elapsed,
        idle_elapsed,
        now,
    )?;
    transaction.commit()?;
    Ok(snapshot)
}

pub fn checkpoint_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    actual_context_revision: u64,
    scope: PetWriteScope,
    effective_elapsed: Duration,
    idle_elapsed: Duration,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    if idle_elapsed > effective_elapsed {
        return Err(AppError::validation(
            "idleElapsed",
            "无互动增量不能超过有效运行增量",
        ));
    }
    let effective_ms =
        i64::try_from(effective_elapsed.as_millis()).map_err(|_| AppError::storage())?;
    let idle_ms = i64::try_from(idle_elapsed.as_millis()).map_err(|_| AppError::storage())?;
    let mut snapshot = validate_scope(transaction, actual_context_revision, scope)?;
    require_at_home(transaction, scope.session_id)?;
    if snapshot.idle_branch == IdleBranch::Sleeping && idle_ms != 0 {
        return Err(AppError::validation(
            "idleElapsed",
            "睡眠期间不能累计无互动周期",
        ));
    }
    if effective_ms != 0 {
        snapshot.hunger_elapsed_ms = snapshot
            .hunger_elapsed_ms
            .checked_add(effective_ms)
            .ok_or_else(AppError::storage)?;
        snapshot.idle_elapsed_ms = snapshot
            .idle_elapsed_ms
            .checked_add(idle_ms)
            .ok_or_else(AppError::storage)?;
        save(transaction, &mut snapshot, now)?;
    }
    Ok(snapshot)
}

#[derive(Clone, Copy)]
pub enum Interaction {
    Click,
    Stroke,
    Feed,
    Wake,
}

/// 所有合法互动开始新的无互动周期；只有喂食清除饥饿累计。
/// 亲密度发放由 pet::interact 与本方法在同一事务内组合，不在此仓储重复实现。
pub fn interact(
    connection: &mut Connection,
    actual_context_revision: u64,
    scope: PetWriteScope,
    interaction: Interaction,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    let transaction = connection.transaction()?;
    let snapshot = interact_in_transaction(
        &transaction,
        actual_context_revision,
        scope,
        interaction,
        now,
    )?;
    transaction.commit()?;
    Ok(snapshot)
}

pub fn interact_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    actual_context_revision: u64,
    scope: PetWriteScope,
    interaction: Interaction,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    let mut snapshot = validate_scope(transaction, actual_context_revision, scope)?;
    require_at_home(transaction, scope.session_id)?;
    snapshot.idle_elapsed_ms = 0;
    snapshot.idle_branch = IdleBranch::None;
    snapshot.sleep_until = None;
    snapshot.last_interacted_at = Some(time_text(now));
    if matches!(interaction, Interaction::Feed) {
        snapshot.hunger_elapsed_ms = 0;
        snapshot.last_fed_at = Some(time_text(now));
    }
    save(transaction, &mut snapshot, now)?;
    Ok(snapshot)
}

/// 状态机决定分支和睡眠持续时间后仅提交一次；读取及恢复不会再次抽随机数。
pub fn choose_idle_branch(
    connection: &mut Connection,
    actual_context_revision: u64,
    scope: PetWriteScope,
    branch: IdleBranch,
    sleep_until: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    let transaction = connection.transaction()?;
    let snapshot = choose_idle_branch_in_transaction(
        &transaction,
        actual_context_revision,
        scope,
        branch,
        sleep_until,
        now,
    )?;
    transaction.commit()?;
    Ok(snapshot)
}

pub fn choose_idle_branch_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    actual_context_revision: u64,
    scope: PetWriteScope,
    branch: IdleBranch,
    sleep_until: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    if branch == IdleBranch::None
        || (branch == IdleBranch::Sleeping) != sleep_until.is_some()
        || sleep_until.is_some_and(|deadline| deadline <= now)
    {
        return Err(AppError::validation(
            "idleBranch",
            "分支与睡眠结束时刻不匹配",
        ));
    }
    let mut snapshot = validate_scope(transaction, actual_context_revision, scope)?;
    require_at_home(transaction, scope.session_id)?;
    if snapshot.idle_branch != IdleBranch::None {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "本周期已经确定无聊或睡眠分支",
        ));
    }
    snapshot.idle_branch = branch;
    snapshot.sleep_until = sleep_until.map(time_text);
    save(transaction, &mut snapshot, now)?;
    Ok(snapshot)
}

/// 可恢复非当前会话已经到期的睡眠，不增加任何有效运行计时。
/// 未来时刻不改写；重复恢复幂等；无效时间按存储错误报告，不能猜测醒来。
pub fn restore_sleep(
    connection: &mut Connection,
    session_id: i64,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    let transaction = connection.transaction()?;
    let snapshot = restore_sleep_in_transaction(&transaction, session_id, now)?;
    transaction.commit()?;
    Ok(snapshot)
}

pub fn restore_sleep_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    session_id: i64,
    now: DateTime<Utc>,
) -> AppResult<PetSnapshot> {
    let mut snapshot = read(transaction, session_id)?;
    if let Some(deadline) = &snapshot.sleep_until {
        let deadline = DateTime::parse_from_rfc3339(deadline).map_err(|_| AppError::storage())?;
        if deadline <= now {
            snapshot.sleep_until = None;
            snapshot.idle_branch = IdleBranch::None;
            snapshot.idle_elapsed_ms = 0;
            save(transaction, &mut snapshot, now)?;
        }
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests;
