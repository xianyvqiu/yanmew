//! 游记是独立展示记录；正常整合、编辑、删除不改变出行事实或聊天记忆。
use crate::{
    error::{AppError, AppResult},
    models::{TravelJournal, TravelJournalDeleted, TravelJournalExpected, TravelJournalSummary},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
pub(crate) mod generation;
#[cfg(test)]
mod tests;
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TravelJournal> {
    Ok(TravelJournal {
        id: r.get::<_, i64>(0)?.to_string(),
        session_id: r.get::<_, i64>(1)?.to_string(),
        travel_id: r.get::<_, i64>(2)?.to_string(),
        title: r.get(3)?,
        content: r.get(4)?,
        created_at: r.get(5)?,
        updated_at: r.get(6)?,
    })
}
pub(crate) fn get(c: &Connection, session: i64, id: i64) -> AppResult<TravelJournal> {
    c.query_row("SELECT id,session_id,travel_id,title,content,created_at,updated_at FROM travel_journals WHERE session_id=?1 AND id=?2",params![session,id],row).optional()?.ok_or_else(AppError::not_found)
}
pub(crate) fn list(
    c: &Connection,
    session: i64,
    before: Option<i64>,
    limit: i64,
) -> AppResult<Vec<TravelJournalSummary>> {
    if !(1..=200).contains(&limit) || before.is_some_and(|id| id <= 0) {
        return Err(AppError::validation(
            "pagination",
            "游记分页参数无效，每页须为1–200条",
        ));
    }
    let mut q=c.prepare("SELECT id,session_id,travel_id,title,created_at,updated_at FROM travel_journals WHERE session_id=?1 AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT ?3")?;
    let rows = q
        .query_map(params![session, before, limit], |r| {
            Ok(TravelJournalSummary {
                id: r.get::<_, i64>(0)?.to_string(),
                session_id: r.get::<_, i64>(1)?.to_string(),
                travel_id: r.get::<_, i64>(2)?.to_string(),
                title: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
fn require_travel(
    c: &Connection,
    session: i64,
    travel: i64,
    journal: &TravelJournal,
) -> AppResult<()> {
    if journal.travel_id != travel.to_string() {
        return Err(AppError::not_found());
    }
    let valid=c.query_row("SELECT EXISTS(SELECT 1 FROM travels WHERE id=?1 AND session_id=?2 AND status='returned' AND journal_state='done')",params![travel,session],|r|r.get::<_,bool>(0))?;
    if !valid {
        return Err(AppError::new(
            "JOURNAL_INVALID_STATE",
            "游记与出行状态不一致，未修改数据",
        ));
    }
    Ok(())
}
pub(crate) fn update(
    c: &mut Connection,
    session: i64,
    travel: i64,
    id: i64,
    expected: TravelJournalExpected,
    title: &str,
    content: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> AppResult<TravelJournal> {
    if title.trim().is_empty() || content.trim().is_empty() {
        return Err(AppError::validation("content", "游记标题和正文不能为空"));
    }
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let original = get(&tx, session, id)?;
    require_travel(&tx, session, travel, &original)?;
    if original.title != expected.title
        || original.content != expected.content
        || original.updated_at != expected.updated_at
    {
        return Err(AppError::new(
            "JOURNAL_CONFLICT",
            "游记已被修改，请保留草稿并重新读取后再保存",
        ));
    }
    tx.execute("UPDATE travel_journals SET title=?1,content=?2,updated_at=?3 WHERE id=?4 AND session_id=?5 AND travel_id=?6",params![title,content,crate::travel::time(now),id,session,travel])?;
    let result = get(&tx, session, id)?;
    tx.commit()?;
    Ok(result)
}
pub(crate) fn delete(
    c: &mut Connection,
    session: i64,
    travel: i64,
    id: i64,
) -> AppResult<TravelJournalDeleted> {
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let original = get(&tx, session, id)?;
    require_travel(&tx, session, travel, &original)?;
    tx.execute(
        "DELETE FROM travel_journals WHERE id=?1 AND session_id=?2 AND travel_id=?3",
        params![id, session, travel],
    )?;
    tx.commit()?;
    Ok(TravelJournalDeleted {
        deleted_id: id.to_string(),
        session_id: session.to_string(),
        travel_id: travel.to_string(),
    })
}
