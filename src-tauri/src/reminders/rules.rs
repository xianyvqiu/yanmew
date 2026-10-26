use super::{schedule::Schedule, *};
use crate::models::Reminder;
use chrono::TimeZone;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReminderInput {
    pub name: String,
    pub content: String,
    pub schedule: Option<Schedule>,
    pub fire_at: Option<String>,
}

fn stamp(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Millis, true)
}
fn parse_time(at: &str) -> AppResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(at)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|_| AppError::validation("fireAt", "提醒时间须包含时区，且为有效日期"))
}
fn rule_from_row(row: &Row<'_>) -> rusqlite::Result<Reminder> {
    let schedule: Option<String> = row.get(3)?;
    let status: String = row.get(5)?;
    let invalid = || {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            "invalid reminder rule".into(),
        )
    };
    Ok(Reminder {
        id: row.get::<_, i64>(0)?.to_string(),
        name: row.get(1)?,
        kind: row.get(2)?,
        schedule: schedule
            .map(|s| serde_json::from_str(&s).map_err(|_| invalid()))
            .transpose()?,
        content: row.get(4)?,
        status: match status.as_str() {
            "pending" => ReminderStatus::Pending,
            "done" => ReminderStatus::Done,
            "expired" => ReminderStatus::Expired,
            _ => return Err(invalid()),
        },
        fire_at: row.get(6)?,
        source: row.get(7)?,
        created_at: row.get(8)?,
    })
}
const SELECT_RULE: &str =
    "SELECT id,name,kind,schedule,content,status,fire_at,source,created_at FROM reminders";

pub fn get_rule(connection: &Connection, id: i64) -> AppResult<Reminder> {
    connection
        .query_row(&format!("{SELECT_RULE} WHERE id=?1"), [id], rule_from_row)
        .optional()?
        .ok_or_else(AppError::not_found)
}

pub fn list_rules(
    connection: &Connection,
    after_id: Option<i64>,
    limit: u32,
) -> AppResult<Vec<Reminder>> {
    if !(1..=200).contains(&limit) {
        return Err(AppError::validation("limit", "每页数量须为 1–200"));
    }
    Ok(connection
        .prepare(&format!("{SELECT_RULE} WHERE id>?1 ORDER BY id LIMIT ?2"))?
        .query_map(params![after_id.unwrap_or(0), limit], rule_from_row)?
        .collect::<Result<Vec<_>, _>>()?)
}

/// 编辑未改变时间规则时保留原游标和状态；只修改未来安排，不删除历史 occurrence。
pub fn save_rule<T: TimeZone>(
    connection: &mut Connection,
    id: Option<i64>,
    mut input: ReminderInput,
    source: &str,
    now: DateTime<Utc>,
    zone: &T,
) -> AppResult<Reminder> {
    input.name = input.name.trim().to_owned();
    if input.name.is_empty() {
        return Err(AppError::validation("name", "请输入提醒名称"));
    }
    if !["manual", "chat"].contains(&source) {
        return Err(AppError::validation("source", "提醒来源无效"));
    }
    let tx = connection.transaction()?;
    let old = id.map(|id| get_rule(&tx, id)).transpose()?;
    let kind = input
        .schedule
        .as_ref()
        .map(|s| s.repeat.as_str())
        .unwrap_or("once");
    let (fire_at, status) = if let Some(schedule) = &mut input.schedule {
        schedule.validate()?;
        schedule.next_local_at = None;
        if input.fire_at.is_some() {
            return Err(AppError::validation(
                "fireAt",
                "循环提醒的下次时间由系统计算",
            ));
        }
        let unchanged = old
            .as_ref()
            .and_then(|o| o.schedule.as_ref())
            .is_some_and(|previous| {
                let mut previous = previous.clone();
                previous.next_local_at = None;
                serde_json::to_value(previous).ok() == serde_json::to_value(&*schedule).ok()
            });
        if unchanged {
            let previous = old.as_ref().unwrap();
            schedule.next_local_at = previous
                .schedule
                .as_ref()
                .and_then(|s| s.next_local_at.clone());
            (previous.fire_at.clone(), ReminderStatus::Pending)
        } else {
            let next = schedule.next(zone, now)?;
            schedule.set_cursor(zone, next);
            (Some(stamp(next)), ReminderStatus::Pending)
        }
    } else {
        let at = input.fire_at.as_deref().map(parse_time).transpose()?;
        let same = old.as_ref().is_some_and(|old| {
            old.kind == "once"
                && old.fire_at.as_deref().map(parse_time).transpose().ok() == Some(at)
        });
        if same {
            (
                old.as_ref().unwrap().fire_at.clone(),
                old.as_ref().unwrap().status,
            )
        } else {
            let at = at
                .filter(|at| *at > now)
                .ok_or_else(|| AppError::validation("fireAt", "请选择未来的提醒时间"))?;
            (Some(stamp(at)), ReminderStatus::Pending)
        }
    };
    let schedule = input
        .schedule
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| AppError::storage())?;
    let result_id = if let Some(id) = id {
        tx.execute("UPDATE reminders SET name=?1,content=?2,kind=?3,schedule=?4,fire_at=?5,status=?6 WHERE id=?7",
            params![input.name,input.content,kind,schedule,fire_at,status.as_str(),id])?;
        id
    } else {
        tx.execute("INSERT INTO reminders(name,content,kind,schedule,fire_at,status,source,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![input.name,input.content,kind,schedule,fire_at,status.as_str(),source,stamp(now)])?;
        tx.last_insert_rowid()
    };
    let result = get_rule(&tx, result_id)?;
    tx.commit()?;
    Ok(result)
}

pub fn delete_rule(connection: &Connection, id: i64) -> AppResult<()> {
    if connection.execute("DELETE FROM reminders WHERE id=?1", [id])? == 0 {
        return Err(AppError::not_found());
    }
    Ok(())
}

/// 同一调度入口先 offline 再 online；游标与 occurrence 始终原子提交。
pub fn dispatch<T: TimeZone>(
    connection: &mut Connection,
    now: DateTime<Utc>,
    zone: &T,
    offline: bool,
) -> AppResult<usize> {
    let tx = connection.transaction()?;
    let rules = tx
        .prepare(&format!(
            "{SELECT_RULE} WHERE status='pending' AND fire_at IS NOT NULL"
        ))?
        .query_map([], rule_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut inserted = 0;
    let mut email_occurrences = Vec::new();
    for mut rule in rules {
        let original = parse_time(rule.fire_at.as_deref().ok_or_else(AppError::storage)?)?;
        let due = match &rule.schedule {
            Some(s) => s.cursor(zone, original)?,
            None => original,
        };
        if due > now {
            if due != original {
                if let Some(s) = &mut rule.schedule {
                    s.set_cursor(zone, due);
                }
                tx.execute(
                    "UPDATE reminders SET fire_at=?1,schedule=?2 WHERE id=?3",
                    params![
                        stamp(due),
                        rule.schedule
                            .as_ref()
                            .map(serde_json::to_string)
                            .transpose()
                            .map_err(|_| AppError::storage())?,
                        rule.id
                    ],
                )?;
            }
            continue;
        }
        let scheduled = if offline {
            match &rule.schedule {
                Some(s) => s.latest(zone, now, due)?.unwrap_or(due),
                None => due,
            }
        } else {
            due
        };
        let delivery = if offline { "missed" } else { "queued" };
        let status = if offline { "expired" } else { "done" };
        let added = tx.execute("INSERT INTO reminder_occurrences(reminder_id,scheduled_at,status,delivery_state,completed_at,created_at)
            VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(reminder_id,scheduled_at) DO NOTHING",
            params![rule.id,stamp(scheduled),status,delivery,Some(stamp(now)),stamp(now)])?;
        inserted += added;
        if added > 0 {
            let occurrence=tx.last_insert_rowid();
            email_occurrences.push(occurrence);
            if !offline {super::presentation::schedule(&tx,occurrence,&rule.name,&rule.content)?;}
        }
        let next = if let Some(schedule) = &mut rule.schedule {
            let next = schedule.next(zone, now)?;
            schedule.set_cursor(zone, next);
            Some(stamp(next))
        } else {
            None
        };
        let rule_status = if rule.kind == "once" {
            status
        } else {
            "pending"
        };
        tx.execute(
            "UPDATE reminders SET fire_at=?1,schedule=?2,status=?3 WHERE id=?4",
            params![
                next,
                rule.schedule
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(|_| AppError::storage())?,
                rule_status,
                rule.id
            ],
        )?;
    }
    crate::reminder_email::enqueue(&tx, &email_occurrences, offline, now)?;
    tx.commit()?;
    Ok(inserted)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissedReminderSummary {
    pub occurrences: Vec<ReminderOccurrence>,
    pub next_after_id: Option<String>,
    pub text: String,
}
pub fn missed_summary(
    connection: &Connection,
    after_id: Option<i64>,
) -> AppResult<MissedReminderSummary> {
    let page = list_occurrences(
        connection,
        None,
        Some(ReminderDeliveryState::Missed),
        after_id,
        200,
    )?;
    let mut seen = std::collections::HashSet::new();
    let names = page
        .items
        .iter()
        .filter(|r| seen.insert(&r.reminder_id))
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
        .join("、");
    let text = if names.is_empty() {
        String::new()
    } else {
        format!("你离开这段时间，{names}过期了！")
    };
    Ok(MissedReminderSummary {
        occurrences: page.items,
        next_after_id: page.next_after_id,
        text,
    })
}
