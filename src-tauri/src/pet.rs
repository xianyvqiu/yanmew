//! 桌宠状态业务核心。只处理调用者已测量的有效区间，不补算离线时间。
//! 定时器、窗口和平台暂停事件应共用此入口；这里不调用 AI、不操作窗口。

use crate::{
    error::{AppError, AppResult},
    pet_store::{self, IdleBranch, Interaction, PetSnapshot, PetWriteScope},
};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::time::Duration;
pub(crate) mod controller;
pub(crate) mod state_bubbles;
pub(crate) mod wander;

/// 会话亲密度固定上限；互动和后续聊天完成奖励共用，不作为用户设置项。
pub const MAX_INTIMACY: i64 = 999;

pub fn rewarded_intimacy(current: i64) -> AppResult<i64> {
    if !(0..=MAX_INTIMACY).contains(&current) {
        return Err(AppError::storage());
    }
    Ok((current + 1).min(MAX_INTIMACY))
}

/// 平台随机源返回 [0, upper_exclusive) 内均匀分布的整数；测试可注入固定序列。
pub trait RandomSource {
    fn below(&mut self, upper_exclusive: u64) -> AppResult<u64>;
}

/// 仅由后端构造，绝不能作为前端可提交的计时参数。
/// effective_elapsed 排除关闭、系统暂停、非当前会话和出门区间。
/// idle_elapsed 还须排除互动、聊天、隐藏、拖动、睡眠和故障休眠区间。
pub struct Tick {
    pub now: DateTime<Utc>,
    pub effective_elapsed: Duration,
    pub idle_elapsed: Duration,
    /// 当前是否允许开始无聊/睡眠分支；暂停、忙碌及故障休眠时为 false。
    pub allow_idle_transition: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Transition {
    None,
    EnteredBored,
    EnteredSleep,
    Woke,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvanceResult {
    pub snapshot: PetSnapshot,
    pub hungry: bool,
    /// 从同一无互动累计派生的待机显示资格，不增加持久化状态或计时器。
    pub standby: bool,
    /// 仅成功提交本次状态变化时返回事件，重复 tick 不会重复发进入事件。
    pub transition: Transition,
    /// 计时器事务已提交出发；调用者此后显示出门状态，不再处理在家分支。
    pub away: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionResult {
    pub snapshot: PetSnapshot,
    pub intimacy: i64,
}

struct Thresholds {
    hungry_ms: i64,
    standby_ms: i64,
    idle_event_ms: i64,
    sleep_min_minutes: u64,
    sleep_max_minutes: u64,
}

impl Thresholds {
    fn read(connection: &Connection) -> AppResult<Self> {
        // 复用全局默认值及校验，设置改变后下一次调用立即采用新值。
        let settings = crate::config::get(connection)?;
        let minutes = |key: &str| -> AppResult<i64> {
            settings
                .get(key)
                .and_then(|value| value.parse().ok())
                .ok_or_else(AppError::storage)
        };
        let milliseconds = |key: &str| -> AppResult<i64> {
            minutes(key)?
                .checked_mul(60_000)
                .ok_or_else(AppError::storage)
        };
        Ok(Self {
            hungry_ms: milliseconds("hungry_minutes")?,
            standby_ms: milliseconds("standby_minutes")?,
            idle_event_ms: milliseconds("idle_event_minutes")?,
            sleep_min_minutes: minutes("sleep_min_minutes")? as u64,
            sleep_max_minutes: minutes("sleep_max_minutes")? as u64,
        })
    }
}

fn draw(random: &mut dyn RandomSource, upper_exclusive: u64) -> AppResult<u64> {
    let value = random.below(upper_exclusive)?;
    if value >= upper_exclusive {
        return Err(AppError::new(
            "RANDOM_SOURCE_ERROR",
            "随机结果无效，桌宠状态未保存",
        ));
    }
    Ok(value)
}

fn next_scope(mut scope: PetWriteScope, snapshot: &PetSnapshot) -> PetWriteScope {
    scope.state_revision = snapshot.state_revision;
    scope
}

/// 保存有效增量并最多执行一次分支变化。调用方必须在提交成功后才发布状态事件。
/// 已开始的睡眠先按 UTC 恢复；本次原先在睡眠中时不把整段间隔算作醒后的空闲。
/// 调用者仍须在上下文切换前收束旧区间，不能把旧会话时间传入新 scope。
pub fn advance(
    connection: &mut Connection,
    context_revision: u64,
    scope: PetWriteScope,
    tick: Tick,
    random: &mut dyn RandomSource,
) -> AppResult<AdvanceResult> {
    advance_impl(connection, context_revision, scope, tick, random, None)
}

/// 自动调度共用原桌宠事务，旅行空闲秒数只能由控制器测量。
pub(crate) fn advance_with_travel(
    connection: &mut Connection, context_revision: u64, scope: PetWriteScope,
    tick: Tick, random: &mut dyn RandomSource, travel_seconds: u64,
) -> AppResult<AdvanceResult> {
    advance_impl(connection, context_revision, scope, tick, random, Some(travel_seconds))
}

fn advance_impl(
    connection: &mut Connection, context_revision: u64, scope: PetWriteScope,
    tick: Tick, random: &mut dyn RandomSource, travel_seconds: Option<u64>,
) -> AppResult<AdvanceResult> {
    if tick.idle_elapsed > tick.effective_elapsed
        || (!tick.allow_idle_transition && !tick.idle_elapsed.is_zero())
    {
        return Err(AppError::validation(
            "idleElapsed",
            "当前运行许可与无互动增量不匹配",
        ));
    }
    let transaction = connection.transaction()?;
    let thresholds = Thresholds::read(&transaction)?;
    let before = pet_store::validate_scope(&transaction, context_revision, scope)?;
    let was_sleeping = before.idle_branch == IdleBranch::Sleeping;
    let restored =
        pet_store::restore_sleep_in_transaction(&transaction, scope.session_id, tick.now)?;
    let woke = was_sleeping && restored.idle_branch == IdleBranch::None;
    let mut snapshot = pet_store::checkpoint_in_transaction(
        &transaction,
        context_revision,
        next_scope(scope, &restored),
        tick.effective_elapsed,
        if was_sleeping {
            Duration::ZERO
        } else {
            tick.idle_elapsed
        },
        tick.now,
    )?;
    let mut transition = if woke {
        Transition::Woke
    } else {
        Transition::None
    };
    let away = if let Some(seconds) = travel_seconds {
        crate::travel::advance_in_transaction(&transaction, scope.session_id,
            if was_sleeping { 0 } else { seconds }, tick.now, random)?
    } else { false };
    if !away && !woke && tick.allow_idle_transition && snapshot.idle_branch == IdleBranch::None {
        if snapshot.idle_elapsed_ms >= thresholds.idle_event_ms {
            // 统一阈值每个无互动周期只随机一次，分支仍由既有仓储保存。
            let sleeping = draw(random, 2)? == 1;
            let (branch, deadline) = if sleeping {
                let range = thresholds.sleep_max_minutes - thresholds.sleep_min_minutes;
                let extra = if range == 0 {
                    0
                } else {
                    draw(random, range + 1)?
                };
                let minutes = i64::try_from(thresholds.sleep_min_minutes + extra)
                    .map_err(|_| AppError::storage())?;
                let deadline = tick
                    .now
                    .checked_add_signed(chrono::Duration::minutes(minutes))
                    .ok_or_else(AppError::storage)?;
                transition = Transition::EnteredSleep;
                (IdleBranch::Sleeping, Some(deadline))
            } else {
                transition = Transition::EnteredBored;
                (IdleBranch::Bored, None)
            };
            snapshot = pet_store::choose_idle_branch_in_transaction(
                &transaction,
                context_revision,
                next_scope(scope, &snapshot),
                branch,
                deadline,
                tick.now,
            )?;
        }
    }
    let hungry = snapshot.hunger_elapsed_ms >= thresholds.hungry_ms;
    let standby = !away && snapshot.idle_branch == IdleBranch::None
        && snapshot.idle_elapsed_ms >= thresholds.standby_ms;
    transaction.commit()?;
    Ok(AdvanceResult {
        snapshot,
        hungry,
        standby,
        transition,
        away,
    })
}

/// 互动状态与奖励原子保存：点击10%概率+1，摸摸/喂食+1，纯唤醒不奖励。
/// 非当前、旧版本和出门中的请求均由仓储拒绝，失败不会留下部分互动结果。
pub fn interact(
    connection: &mut Connection,
    context_revision: u64,
    scope: PetWriteScope,
    interaction: Interaction,
    now: DateTime<Utc>,
    random: &mut dyn RandomSource,
) -> AppResult<InteractionResult> {
    let transaction = connection.transaction()?;
    let snapshot = pet_store::interact_in_transaction(
        &transaction,
        context_revision,
        scope,
        interaction,
        now,
    )?;
    let intimacy: i64 = transaction.query_row(
        "SELECT intimacy FROM sessions WHERE id=?1",
        [scope.session_id],
        |row| row.get(0),
    )?;
    let rewarded = match interaction {
        Interaction::Wake => false,
        Interaction::Click => intimacy < MAX_INTIMACY && random.below(100)? < 10,
        _ => true,
    };
    let intimacy = if rewarded {
        rewarded_intimacy(intimacy)?
    } else {
        intimacy
    };
    if rewarded {
        transaction.execute(
            "UPDATE sessions SET intimacy=?1,updated_at=?2 WHERE id=?3",
            params![
                intimacy,
                now.to_rfc3339_opts(SecondsFormat::Millis, true),
                scope.session_id
            ],
        )?;
    }
    transaction.commit()?;
    Ok(InteractionResult { snapshot, intimacy })
}

#[cfg(test)]
mod tests;
