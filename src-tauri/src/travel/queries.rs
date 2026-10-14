//! 当前出行和历史只读查询；不会接受前端计时、生成状态或奖励参数。
use super::TravelStatus;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Travel {
    pub id: String,
    pub session_id: String,
    pub status: TravelStatus,
    pub started_at: String,
    pub planned_return_at: String,
    pub actual_return_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub midway_letter_at: String,
    pub destination: Option<String>,
    pub facts_state: String,
    pub midway_letter_state: String,
    pub return_letter_state: String,
    pub gift_state: String,
    pub journal_state: String,
    /// 只有实际已发放的旅行才显示历史礼物类型；取消/待生成不是奖励。
    pub gift_kind: Option<String>,
}
const COLUMNS: &str = "id,session_id,status,started_at,planned_return_at,actual_return_at,cancelled_at,midway_letter_at,destination,facts_state,midway_letter_state,return_letter_state,gift_state,journal_state,CASE WHEN status='returned' AND gift_state='done' THEN gift_kind ELSE NULL END";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Travel> {
    let status = match r.get::<_, String>(2)?.as_str() {
        "away" => TravelStatus::Away,
        "returned" => TravelStatus::Returned,
        "cancelled" => TravelStatus::Cancelled,
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    Ok(Travel {
        id: r.get::<_, i64>(0)?.to_string(),
        session_id: r.get::<_, i64>(1)?.to_string(),
        status,
        started_at: r.get(3)?,
        planned_return_at: r.get(4)?,
        actual_return_at: r.get(5)?,
        cancelled_at: r.get(6)?,
        midway_letter_at: r.get(7)?,
        destination: r.get(8)?,
        facts_state: r.get(9)?,
        midway_letter_state: r.get(10)?,
        return_letter_state: r.get(11)?,
        gift_state: r.get(12)?,
        journal_state: r.get(13)?,
        gift_kind: r.get(14)?,
    })
}
pub(crate) fn current(connection: &Connection, session: i64) -> AppResult<Option<Travel>> {
    Ok(connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM travels WHERE session_id=?1 AND status='away'"),
            [session],
            row,
        )
        .optional()?)
}
pub(crate) fn list(
    connection: &Connection,
    session: i64,
    before: Option<i64>,
    limit: i64,
) -> AppResult<Vec<Travel>> {
    if !(1..=200).contains(&limit) || before.is_some_and(|id| id <= 0) {
        return Err(AppError::validation(
            "pagination",
            "出行分页参数无效，每页须为1–200条",
        ));
    }
    let mut statement = connection.prepare(&format!("SELECT {COLUMNS} FROM travels WHERE session_id=?1 AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT ?3"))?;
    let rows = statement
        .query_map(params![session, before, limit], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
