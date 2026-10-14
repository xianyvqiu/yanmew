//! 出行生命周期。所有计时由后端测量；本模块不接收前端秒数，不调用窗口或网络。
use crate::{
    error::{AppError, AppResult},
    pet::RandomSource,
};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub(crate) mod facts;
pub(crate) mod queries;
pub use facts::GenerationStatus as FactsGenerationStatus;
pub use queries::Travel;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TravelStatus {
    Away,
    Returned,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub elapsed_minutes: i64,
    pub description: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    pub destination: String,
    pub activities: Vec<Activity>,
}
impl Facts {
    /// 事实一次完整校验；未来活动只能作为行程计划，不能当作已经发生。
    pub fn validate(&self, duration_minutes: i64) -> AppResult<()> {
        let valid_text = |text: &str, max| {
            !text.trim().is_empty()
                && text.chars().count() <= max
                && !text.chars().any(|c| c.is_control())
        };
        let mut previous = -1;
        if !valid_text(&self.destination, 120)
            || self.activities.is_empty()
            || self.activities.len() > 32
        {
            return Err(invalid_facts());
        }
        for activity in &self.activities {
            if activity.elapsed_minutes < 0
                || activity.elapsed_minutes > duration_minutes
                || activity.elapsed_minutes <= previous
                || !valid_text(&activity.description, 500)
            {
                return Err(invalid_facts());
            }
            previous = activity.elapsed_minutes;
        }
        Ok(())
    }
    pub fn happened(&self, elapsed: Duration) -> Vec<&Activity> {
        self.activities
            .iter()
            .filter(|a| a.elapsed_minutes as u64 <= elapsed.as_secs() / 60)
            .collect()
    }
}
fn invalid_facts() -> AppError {
    AppError::new(
        "TRAVEL_INVALID_FACTS",
        "旅行事实格式或活动时间线无效，未保存",
    )
}
pub(crate) fn time(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn setting(connection: &Connection, key: &str) -> AppResult<u64> {
    crate::config::get(connection)?
        .get(key)
        .and_then(|s| s.parse().ok())
        .ok_or_else(AppError::storage)
}
fn draw(random: &mut dyn RandomSource, upper: u64) -> AppResult<u64> {
    let value = random.below(upper)?;
    if value >= upper {
        return Err(AppError::new(
            "RANDOM_SOURCE_ERROR",
            "旅行随机数无效，状态未保存",
        ));
    }
    Ok(value)
}
fn range(
    connection: &Connection,
    random: &mut dyn RandomSource,
    min: &str,
    max: &str,
) -> AppResult<u64> {
    let low = setting(connection, min)?;
    let high = setting(connection, max)?;
    let width = high
        .checked_sub(low)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(AppError::storage)?;
    Ok(low + if width == 1 { 0 } else { draw(random, width)? })
}
pub(crate) fn draw_wait(connection: &Connection, random: &mut dyn RandomSource) -> AppResult<i64> {
    // 秒级均匀抽取，包含设置的两个端点，不只随机几个整小时。
    let low = setting(connection, "travel_min_online_hours")?
        .checked_mul(3600)
        .ok_or_else(AppError::storage)?;
    let high = setting(connection, "travel_max_online_hours")?
        .checked_mul(3600)
        .ok_or_else(AppError::storage)?;
    let width = high
        .checked_sub(low)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(AppError::storage)?;
    i64::try_from(low + if width == 1 { 0 } else { draw(random, width)? })
        .map_err(|_| AppError::storage())
}

/// 无运行时的建库/会话事务使用 SQLite 自带随机源，避免业务依赖 Windows。
/// 调度器仍使用注入的系统随机源；两者共用相同抽取及范围校验。
struct SqlRandom<'a>(&'a Connection);
impl RandomSource for SqlRandom<'_> {
    fn below(&mut self, upper: u64) -> AppResult<u64> {
        if upper == 0 {
            return Err(AppError::storage());
        }
        let rejected = upper.wrapping_neg() % upper;
        loop {
            let value = self
                .0
                .query_row("SELECT random()", [], |r| r.get::<_, i64>(0))?
                as u64;
            if value >= rejected {
                return Ok(value % upper);
            }
        }
    }
}
pub(crate) fn reset_wait(connection: &Connection, session: i64) -> AppResult<()> {
    let wait = draw_wait(connection, &mut SqlRandom(connection))?;
    connection.execute(
        "UPDATE sessions SET travel_trigger_remaining_seconds=?1 WHERE id=?2",
        params![wait, session],
    )?;
    Ok(())
}

/// 调用方已验证当前会话并开启事务，与桌宠检查点共用提交边界。
/// 空闲秒数跨过零时直接出发，绝不保存零或负数；重复零增量不能重复出发。
pub(crate) fn advance_in_transaction(
    connection: &Connection,
    session: i64,
    seconds: u64,
    now: DateTime<Utc>,
    random: &mut dyn RandomSource,
) -> AppResult<bool> {
    let away = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",
        [session],
        |r| r.get::<_, bool>(0),
    )?;
    if away {
        return Ok(true);
    }
    let remaining: Option<i64> = connection.query_row(
        "SELECT travel_trigger_remaining_seconds FROM sessions WHERE id=?1",
        [session],
        |r| r.get(0),
    )?;
    // 兼容尚未启用计时的已有会话；首次访问只初始化，不补算旧在线时间。
    let Some(remaining) = remaining else {
        let wait = draw_wait(connection, random)?;
        connection.execute(
            "UPDATE sessions SET travel_trigger_remaining_seconds=?1 WHERE id=?2",
            params![wait, session],
        )?;
        return Ok(false);
    };
    if seconds == 0 {
        return Ok(false);
    }
    let seconds = i64::try_from(seconds).map_err(|_| AppError::storage())?;
    if remaining > seconds {
        connection.execute("UPDATE sessions SET travel_trigger_remaining_seconds=travel_trigger_remaining_seconds-?1 WHERE id=?2",params![seconds,session])?;
        return Ok(false);
    }
    let minutes = range(
        connection,
        random,
        "travel_min_duration_minutes",
        "travel_max_duration_minutes",
    )?;
    let seconds = minutes
        .checked_mul(60)
        .and_then(|v| i64::try_from(v).ok())
        .ok_or_else(AppError::storage)?;
    let planned = now
        .checked_add_signed(chrono::Duration::seconds(seconds))
        .ok_or_else(AppError::storage)?;
    let midway = now
        .checked_add_signed(chrono::Duration::seconds(seconds / 2))
        .ok_or_else(AppError::storage)?;
    connection.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(?1,'away',?2,?3,?4,?2)",params![session,time(now),time(planned),time(midway)])?;
    connection.execute(
        "UPDATE sessions SET travel_trigger_remaining_seconds=NULL WHERE id=?1",
        [session],
    )?;
    Ok(true)
}

/// 所有到期旅行先恢复状态，再允许事实/信件任务扫描；关闭期间自然到期。
/// 每次事务内复验 away，抽签仅在确实返回时执行。返回时间统一使用计划时刻。
pub(crate) fn recover_returns(
    connection: &mut Connection,
    now: DateTime<Utc>,
    random: &mut dyn RandomSource,
) -> AppResult<usize> {
    let rows = {
        let mut q = connection
            .prepare("SELECT id,session_id,planned_return_at FROM travels WHERE status='away'")?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut count = 0;
    for (id, session, planned) in rows {
        let deadline = DateTime::parse_from_rfc3339(&planned)
            .map_err(|_| AppError::storage())?
            .with_timezone(&Utc);
        if now < deadline {
            continue;
        }
        let tx = connection.transaction()?;
        if !tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM travels WHERE id=?1 AND status='away')",
            [id],
            |r| r.get::<_, bool>(0),
        )? {
            continue;
        }
        let gift = if draw(random, 100)? < setting(&tx, "travel_accessory_chance_percent")? {
            "accessory"
        } else {
            "text"
        };
        let wait = draw_wait(&tx, random)?;
        tx.execute("UPDATE travels SET status='returned',actual_return_at=planned_return_at,gift_kind=?1,midway_letter_state=CASE WHEN midway_letter_state='pending' THEN 'cancelled' ELSE midway_letter_state END WHERE id=?2",params![gift,id])?;
        tx.execute("UPDATE letters SET reply_state='cancelled' WHERE travel_id=?1 AND reply_state='pending'",[id])?;
        tx.execute(
            "UPDATE sessions SET last_return_at=?1,travel_trigger_remaining_seconds=?2 WHERE id=?3",
            params![planned, wait, session],
        )?;
        tx.commit()?;
        count += 1;
    }
    Ok(count)
}

/// 共用会话/人设切换事务。只有取消了 away 的会话才重抽；在家切换不改倒计时。
pub(crate) fn cancel_current(connection: &Connection, changed_at: &str) -> AppResult<()> {
    let session = connection
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM settings WHERE key='current_session_id'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?;
    let Some(session) = session else {
        return Ok(());
    };
    let changed=connection.execute("UPDATE travels SET status='cancelled',cancelled_at=?1,midway_letter_state=CASE WHEN midway_letter_state='pending' THEN 'cancelled' ELSE midway_letter_state END,return_letter_state='cancelled',gift_state='cancelled',journal_state='cancelled',facts_state=CASE WHEN facts_state='pending' THEN 'cancelled' ELSE facts_state END WHERE status='away' AND session_id=?2",params![changed_at,session])?;
    connection.execute(
        "UPDATE letters SET reply_state='cancelled' WHERE reply_state='pending' AND session_id=?1",
        [session],
    )?;
    if changed > 0 {
        reset_wait(connection, session)?
    }
    Ok(())
}

/// 用户信首次保存时抽取一次，重启与生成重试不得调用此入口。
pub(crate) fn draw_reply_minutes(connection:&Connection,random:&mut dyn RandomSource)->AppResult<i64> {
    i64::try_from(range(connection,random,"letter_reply_min_minutes","letter_reply_max_minutes")?).map_err(|_|AppError::storage())
}
