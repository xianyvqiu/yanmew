//! 会话信件仓储与发送规则。信件不写聊天回合、记忆、亲密度或钱包。
use crate::{
    error::{AppError, AppResult},
    models::{Letter, LetterKind, LetterReplyState, LetterSender},
    pet::RandomSource,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;

pub(crate) mod generation;
pub use generation::{GenerationState, GenerationStatus};
#[cfg(test)]
mod tests;

/// 只由命令层按当前上下文构造；不能用目标会话替代实际当前会话。
#[derive(Clone, Copy)]
pub(crate) struct Scope {
    pub persona: i64,
    pub session: i64,
    pub revision: u64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SendInput {
    /// 绑定打开写信时的出行，防止旧草稿落入下一次旅行。
    pub travel_id: String,
    pub content: String,
    pub reply_to_id: Option<String>,
}
pub(crate) fn require_current(
    connection: &Connection,
    actual_revision: u64,
    scope: Scope,
) -> AppResult<()> {
    let current = crate::persona::context(connection, actual_revision)?;
    if current.persona_id != scope.persona.to_string()
        || current.session_id != Some(scope.session.to_string())
        || actual_revision != scope.revision
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前会话已变化，请重新打开信件",
        ));
    }
    Ok(())
}
const COLUMNS:&str="id,session_id,travel_id,sender,content,reply_to_id,has_been_replied,reply_due_at,reply_state,letter_kind,is_favorite,is_read,created_at";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Letter> {
    let invalid = || rusqlite::Error::InvalidQuery;
    Ok(Letter {
        id: r.get::<_, i64>(0)?.to_string(),
        session_id: r.get::<_, i64>(1)?.to_string(),
        travel_id: r.get::<_, Option<i64>>(2)?.map(|v| v.to_string()),
        sender: LetterSender::parse(&r.get::<_, String>(3)?).ok_or_else(invalid)?,
        content: r.get(4)?,
        reply_to_id: r.get::<_, Option<i64>>(5)?.map(|v| v.to_string()),
        has_been_replied: r.get(6)?,
        reply_due_at: r.get(7)?,
        reply_state: LetterReplyState::parse(&r.get::<_, String>(8)?).ok_or_else(invalid)?,
        letter_kind: LetterKind::parse(&r.get::<_, String>(9)?).ok_or_else(invalid)?,
        is_favorite: r.get(10)?,
        is_read: r.get(11)?,
        created_at: r.get(12)?,
    })
}
pub(crate) fn get(connection: &Connection, session: i64, id: i64) -> AppResult<Letter> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM letters WHERE session_id=?1 AND id=?2"),
            params![session, id],
            row,
        )
        .optional()?
        .ok_or_else(AppError::not_found)
}
pub(crate) fn list(
    connection: &Connection,
    session: i64,
    before: Option<i64>,
    limit: i64,
) -> AppResult<Vec<Letter>> {
    if !(1..=200).contains(&limit) || before.is_some_and(|id| id <= 0) {
        return Err(AppError::validation(
            "limit",
            "信件分页参数无效，每页须为1–200条",
        ));
    }
    let mut q=connection.prepare(&format!("SELECT {COLUMNS} FROM letters WHERE session_id=?1 AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT ?3"))?;
    let result = q
        .query_map(params![session, before, limit], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(result)
}
pub(crate) fn send(
    connection: &mut Connection,
    actual_revision: u64,
    scope: Scope,
    travel: i64,
    content: &str,
    reply_to: Option<i64>,
    now: DateTime<Utc>,
    random: &mut dyn RandomSource,
) -> AppResult<Letter> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::validation("content", "信件正文不能为空"));
    }
    let tx = connection.transaction()?;
    require_current(&tx, actual_revision, scope)?;
    validate_target(&tx, scope.session, travel, reply_to, now)?;
    if let Some(id) = reply_to {
        tx.execute("UPDATE letters SET has_been_replied=1 WHERE id=?1 AND has_been_replied=0", [id])?;
    }
    let minutes = crate::travel::draw_reply_minutes(&tx, random)?;
    let deadline = now
        .checked_add_signed(chrono::Duration::minutes(minutes))
        .ok_or_else(AppError::storage)?;
    tx.execute("INSERT INTO letters(session_id,travel_id,sender,content,reply_to_id,reply_due_at,reply_state,letter_kind,is_read,created_at) VALUES(?1,?2,'user',?3,?4,?5,'pending','user',1,?6)",params![scope.session,travel,content,reply_to,crate::travel::time(deadline),crate::travel::time(now)])?;
    let saved = get(&tx, scope.session, tx.last_insert_rowid())?;
    tx.commit()?;
    Ok(saved)
}
/// 编辑器和发送事务共用资格校验；仅打开窗口不占用原信回复资格。
pub(crate) fn validate_target(connection: &Connection, session: i64, travel: i64, reply_to: Option<i64>, now: DateTime<Utc>) -> AppResult<()> {
    let planned = connection
        .query_row(
            "SELECT planned_return_at FROM travels WHERE id=?1 AND session_id=?2 AND status='away'",
            params![travel, session],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(not_away)?;
    if now >= parse_time(&planned)? {
        return Err(not_away());
    }
    if let Some(id) = reply_to {
        let target = get(connection, session, id)?;
        if target.travel_id.as_deref() != Some(travel.to_string().as_str())
            || target.sender != LetterSender::Ai
        {
            return Err(AppError::validation(
                "replyToId",
                "只能回复本次旅行的AI信件",
            ));
        }
        if target.has_been_replied {
            return Err(AppError::new(
                "LETTER_ALREADY_REPLIED",
                "这封信已经回复过，不能再次回复",
            ));
        }
    }
    Ok(())
}
pub(crate) fn flags(
    connection: &Connection,
    session: i64,
    id: i64,
    favorite: Option<bool>,
    read: Option<bool>,
) -> AppResult<Letter> {
    if connection.execute("UPDATE letters SET is_favorite=COALESCE(?1,is_favorite),is_read=COALESCE(?2,is_read) WHERE id=?3 AND session_id=?4",params![favorite,read,id,session])?!=1 {return Err(AppError::not_found())}
    get(connection, session, id)
}
pub(crate) fn delete(connection: &Connection, session: i64, id: i64) -> AppResult<()> {
    // 外键仅将现存回复的reply_to_id置空，不清除原信历史回复标记、不重置旅行done。
    if connection.execute(
        "DELETE FROM letters WHERE id=?1 AND session_id=?2",
        params![id, session],
    )? != 1
    {
        return Err(AppError::not_found());
    }
    Ok(())
}
pub(crate) fn parse_time(value: &str) -> AppResult<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .map_err(|_| AppError::storage())?
        .with_timezone(&Utc))
}
fn not_away() -> AppError {
    AppError::new("LETTER_NOT_AWAY", "本次旅行已结束或取消，不能再写信或回信")
}
