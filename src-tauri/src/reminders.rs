//! 全局提醒业务；展示确认与用户完成状态独立保存。
mod rules;
pub mod schedule;
use crate::{
    error::{AppError, AppResult},
    models::{ReminderDeliveryState, ReminderOccurrence, ReminderOccurrencePage, ReminderStatus},
};
use chrono::{DateTime, SecondsFormat, Utc};
pub use rules::*;
use rusqlite::{params, Connection, OptionalExtension, Row};

const SELECT_OCCURRENCE: &str = "SELECT o.id,o.reminder_id,r.name,r.content,o.scheduled_at,
    o.status,o.delivery_state,o.completed_at,o.created_at
    FROM reminder_occurrences o JOIN reminders r ON r.id=o.reminder_id";

fn from_row(row: &Row<'_>) -> rusqlite::Result<ReminderOccurrence> {
    let status: String = row.get(5)?;
    let delivery: String = row.get(6)?;
    let invalid = |column| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            "invalid reminder state".into(),
        )
    };
    Ok(ReminderOccurrence {
        id: row.get::<_, i64>(0)?.to_string(),
        reminder_id: row.get::<_, i64>(1)?.to_string(),
        name: row.get(2)?,
        content: row.get(3)?,
        scheduled_at: row.get(4)?,
        status: match status.as_str() {
            "pending" => ReminderStatus::Pending,
            "done" => ReminderStatus::Done,
            "expired" => ReminderStatus::Expired,
            _ => return Err(invalid(5)),
        },
        delivery_state: match delivery.as_str() {
            "queued" => ReminderDeliveryState::Queued,
            "missed" => ReminderDeliveryState::Missed,
            "shown" => ReminderDeliveryState::Shown,
            _ => return Err(invalid(6)),
        },
        completed_at: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub fn get_occurrence(connection: &Connection, id: i64) -> AppResult<ReminderOccurrence> {
    connection
        .query_row(
            &format!("{SELECT_OCCURRENCE} WHERE o.id=?1"),
            [id],
            from_row,
        )
        .optional()?
        .ok_or_else(AppError::not_found)
}

/// ID 游标避免新触发记录导致翻页跳项；额外读取一条判断是否还有下一页。
pub fn list_occurrences(
    connection: &Connection,
    reminder_id: Option<i64>,
    delivery: Option<ReminderDeliveryState>,
    after_id: Option<i64>,
    limit: u32,
) -> AppResult<ReminderOccurrencePage> {
    if !(1..=200).contains(&limit) {
        return Err(AppError::validation("limit", "每页数量须为 1–200"));
    }
    let mut statement = connection.prepare(&format!(
        "{SELECT_OCCURRENCE}
        WHERE (?1 IS NULL OR o.reminder_id=?1) AND (?2 IS NULL OR o.delivery_state=?2)
        AND o.id>?3 ORDER BY o.id LIMIT ?4"
    ))?;
    let mut items = statement
        .query_map(
            params![
                reminder_id,
                delivery.map(|v| v.as_str()),
                after_id.unwrap_or(0),
                i64::from(limit) + 1
            ],
            from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    let next_after_id = if items.len() > limit as usize {
        items.pop();
        items.last().map(|item| item.id.clone())
    } else {
        None
    };
    Ok(ReminderOccurrencePage {
        items,
        next_after_id,
    })
}

/// 只确认客户端实际展示的 ID；不能把领取以后新到的提醒一起标为已展示。
pub fn acknowledge(
    connection: &mut Connection,
    ids: &[i64],
    expected: ReminderDeliveryState,
) -> AppResult<()> {
    if ids.len() > 200 || ids.iter().any(|id| *id <= 0) || expected == ReminderDeliveryState::Shown
    {
        return Err(AppError::validation("ids", "展示确认参数无效"));
    }
    let transaction = connection.transaction()?;
    for id in ids {
        transaction.execute(
            "UPDATE reminder_occurrences SET delivery_state='shown'
            WHERE id=?1 AND delivery_state=?2",
            params![id, expected.as_str()],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

pub fn update_occurrence_status(
    connection: &mut Connection,
    id: i64,
    status: ReminderStatus,
    now: DateTime<Utc>,
) -> AppResult<ReminderOccurrence> {
    let transaction = connection.transaction()?;
    let current = get_occurrence(&transaction, id)?;
    if current.status != status {
        let completed_at = (status != ReminderStatus::Pending)
            .then(|| now.to_rfc3339_opts(SecondsFormat::Millis, true));
        transaction.execute(
            "UPDATE reminder_occurrences SET status=?1,completed_at=?2 WHERE id=?3",
            params![status.as_str(), completed_at, id],
        )?;
        // 一次性恢复待处理也不重置 fire_at，不重新发送同一次提醒。
        transaction.execute(
            "UPDATE reminders SET status=?1 WHERE id=?2 AND kind='once' AND fire_at IS NULL
             AND NOT EXISTS(SELECT 1 FROM reminder_occurrences WHERE reminder_id=?2 AND id>?3)",
            params![status.as_str(), current.reminder_id, id],
        )?;
    }
    let result = get_occurrence(&transaction, id)?;
    transaction.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests;
