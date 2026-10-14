//! 普通旅行伴手礼管理；展示项删除不修改不可变出行事实和发放标记。
use crate::{
    error::{AppError, AppResult},
    models::Souvenir,
};
use rusqlite::{params, Connection, OptionalExtension};
pub(crate) mod generation;
#[cfg(test)]
mod tests;

const COLUMNS: &str = "id,session_id,travel_id,content,note,is_favorite,created_at";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Souvenir> {
    Ok(Souvenir {
        id: r.get::<_, i64>(0)?.to_string(),
        session_id: r.get::<_, i64>(1)?.to_string(),
        travel_id: r.get::<_, Option<i64>>(2)?.map(|id| id.to_string()),
        content: r.get(3)?,
        note: r.get(4)?,
        is_favorite: r.get(5)?,
        created_at: r.get(6)?,
    })
}
pub(crate) fn list(
    c: &Connection,
    session: i64,
    before: Option<i64>,
    limit: i64,
) -> AppResult<Vec<Souvenir>> {
    if !(1..=200).contains(&limit) || before.is_some_and(|id| id <= 0) {
        return Err(AppError::validation(
            "pagination",
            "伴手礼分页参数无效，每页须为1–200条",
        ));
    }
    let mut query=c.prepare(&format!("SELECT {COLUMNS} FROM souvenirs WHERE session_id=?1 AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT ?3"))?;
    let rows = query
        .query_map(params![session, before, limit], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub(crate) fn get(c: &Connection, session: i64, id: i64) -> AppResult<Souvenir> {
    c.query_row(
        &format!("SELECT {COLUMNS} FROM souvenirs WHERE session_id=?1 AND id=?2"),
        params![session, id],
        row,
    )
    .optional()?
    .ok_or_else(AppError::not_found)
}
pub(crate) fn favorite(c: &Connection, session: i64, id: i64, value: bool) -> AppResult<Souvenir> {
    if c.execute(
        "UPDATE souvenirs SET is_favorite=?1 WHERE session_id=?2 AND id=?3",
        params![value, session, id],
    )? != 1
    {
        return Err(AppError::not_found());
    }
    get(c, session, id)
}
pub(crate) fn delete(c: &Connection, session: i64, id: i64) -> AppResult<()> {
    if c.execute(
        "DELETE FROM souvenirs WHERE session_id=?1 AND id=?2",
        params![session, id],
    )? != 1
    {
        return Err(AppError::not_found());
    }
    Ok(())
}
