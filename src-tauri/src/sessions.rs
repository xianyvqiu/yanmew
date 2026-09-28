use crate::{
    error::{AppError, AppResult},
    models::{
        ChatRound, ChatRoundStatus, Message, MessagePage, MessageRole, RoundSnapshot, Session,
    },
};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

const DEFAULT_TITLE: &str = "新会话";
const MAX_TITLE_CHARS: usize = 100;
const MAX_MESSAGE_PAGE: i64 = 200;

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn session_from_row(row: &Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get::<_, i64>(0)?.to_string(),
        persona_id: row.get::<_, i64>(1)?.to_string(),
        user_profile_id: row.get::<_, Option<i64>>(2)?.map(|id| id.to_string()),
        title: row.get(3)?,
        memory_revision: row.get(4)?,
        intimacy: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        is_current: row.get(8)?,
    })
}

fn message_from_row(row: &Row<'_>) -> rusqlite::Result<Message> {
    let role: String = row.get(3)?;
    let role = MessageRole::parse(&role).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            "invalid message role".into(),
        )
    })?;
    Ok(Message {
        id: row.get::<_, i64>(0)?.to_string(),
        session_id: row.get::<_, i64>(1)?.to_string(),
        round_id: row.get::<_, Option<i64>>(2)?.map(|id| id.to_string()),
        role,
        content: row.get(4)?,
        expression: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn round_from_row(row: &Row<'_>) -> rusqlite::Result<ChatRound> {
    let status: String = row.get(2)?;
    let status = ChatRoundStatus::parse(&status).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            "invalid chat round status".into(),
        )
    })?;
    Ok(ChatRound {
        id: row.get::<_, i64>(0)?.to_string(),
        session_id: row.get::<_, i64>(1)?.to_string(),
        status,
        created_at: row.get(3)?,
        completed_at: row.get(4)?,
    })
}

fn require_session(connection: &Connection, persona_id: i64, session_id: i64) -> AppResult<()> {
    if connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1 AND persona_id=?2)",
        params![session_id, persona_id],
        |row| row.get::<_, bool>(0),
    )? {
        Ok(())
    } else {
        Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设或会话已经改变，请刷新后重试",
        ))
    }
}

pub fn get(connection: &Connection, persona_id: i64, session_id: i64) -> AppResult<Session> {
    connection
        .query_row(
            "SELECT s.id,s.persona_id,s.user_profile_id,s.title,s.memory_revision,s.intimacy,s.created_at,s.updated_at,
                    EXISTS(SELECT 1 FROM settings WHERE key='current_session_id' AND value=CAST(s.id AS TEXT))
             FROM sessions s WHERE s.id=?1 AND s.persona_id=?2",
            params![session_id, persona_id],
            session_from_row,
        )
        .optional()?
        .ok_or_else(AppError::not_found)
}

pub fn list(connection: &Connection, persona_id: i64) -> AppResult<Vec<Session>> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM personas WHERE id=?1)",
        [persona_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Err(AppError::not_found());
    }
    let mut statement = connection.prepare(
        "SELECT s.id,s.persona_id,s.user_profile_id,s.title,s.memory_revision,s.intimacy,s.created_at,s.updated_at,
                EXISTS(SELECT 1 FROM settings WHERE key='current_session_id' AND value=CAST(s.id AS TEXT))
         FROM sessions s WHERE s.persona_id=?1 ORDER BY s.updated_at DESC,s.id DESC",
    )?;
    let sessions = statement
        .query_map([persona_id], session_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sessions)
}

/// 切换人设或会话时，已出发旅行与待回信都取消；未出发倒计时保留在会话行。
pub(crate) fn cancel_current_travel_and_replies(
    connection: &Connection,
    changed_at: &str,
) -> AppResult<()> {
    connection.execute("UPDATE travels SET status='cancelled',cancelled_at=?1,midway_letter_state=CASE WHEN midway_letter_state='pending' THEN 'cancelled' ELSE midway_letter_state END,return_letter_state='cancelled',gift_state='cancelled',journal_state='cancelled',facts_state=CASE WHEN facts_state='pending' THEN 'cancelled' ELSE facts_state END WHERE status='away' AND session_id=(SELECT CAST(value AS INTEGER) FROM settings WHERE key='current_session_id')",[changed_at])?;
    connection.execute("UPDATE letters SET reply_state='cancelled' WHERE reply_state='pending' AND session_id=(SELECT CAST(value AS INTEGER) FROM settings WHERE key='current_session_id')",[])?;
    Ok(())
}

fn set_current_in_transaction(
    transaction: &Transaction<'_>,
    persona_id: i64,
    session_id: i64,
    changed_at: &str,
) -> AppResult<bool> {
    require_session(transaction, persona_id, session_id)?;
    let current: Option<String> = transaction
        .query_row(
            "SELECT value FROM settings WHERE key='current_session_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if current.as_deref() == Some(&session_id.to_string()) {
        return Ok(false);
    }
    cancel_current_travel_and_replies(transaction, changed_at)?;
    transaction.execute(
        "INSERT INTO settings(key,value) VALUES('current_session_id',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [session_id.to_string()],
    )?;
    Ok(true)
}

pub fn create(connection: &mut Connection, persona_id: i64) -> AppResult<Session> {
    let changed_at = now();
    let transaction = connection.transaction()?;
    let active = transaction
        .query_row(
            "SELECT is_active FROM personas WHERE id=?1",
            [persona_id],
            |row| row.get::<_, bool>(0),
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if !active {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设已经改变，请刷新后重试",
        ));
    }
    transaction.execute(
        "INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(?1,?2,?3,?3)",
        params![persona_id, DEFAULT_TITLE, changed_at],
    )?;
    let session_id = transaction.last_insert_rowid();
    set_current_in_transaction(&transaction, persona_id, session_id, &changed_at)?;
    let result = get(&transaction, persona_id, session_id)?;
    transaction.commit()?;
    Ok(result)
}

pub fn select(connection: &mut Connection, persona_id: i64, session_id: i64) -> AppResult<bool> {
    let changed_at = now();
    let transaction = connection.transaction()?;
    let changed = set_current_in_transaction(&transaction, persona_id, session_id, &changed_at)?;
    transaction.commit()?;
    Ok(changed)
}

pub fn rename(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    title: &str,
) -> AppResult<Session> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(AppError::validation("title", "会话标题须为 1–100 个字符"));
    }
    let transaction = connection.transaction()?;
    require_session(&transaction, persona_id, session_id)?;
    transaction.execute(
        "UPDATE sessions SET title=?1,updated_at=?2 WHERE id=?3 AND persona_id=?4",
        params![title, now(), session_id, persona_id],
    )?;
    let result = get(&transaction, persona_id, session_id)?;
    transaction.commit()?;
    Ok(result)
}

pub fn messages(
    connection: &Connection,
    persona_id: i64,
    session_id: i64,
    after_id: Option<i64>,
    limit: i64,
) -> AppResult<MessagePage> {
    require_session(connection, persona_id, session_id)?;
    if !(1..=MAX_MESSAGE_PAGE).contains(&limit) {
        return Err(AppError::validation("limit", "每页消息数须为 1–200"));
    }
    let after_id = after_id.unwrap_or(0);
    if after_id < 0 {
        return Err(AppError::validation("afterId", "消息游标不能为负数"));
    }
    let mut statement = connection.prepare(
        "SELECT id,session_id,round_id,role,content,expression,created_at
         FROM messages WHERE session_id=?1 AND id>?2 ORDER BY id LIMIT ?3",
    )?;
    let mut items = statement
        .query_map(params![session_id, after_id, limit + 1], message_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = items.len() > limit as usize;
    if has_more {
        items.pop();
    }
    let next_after_id = has_more.then(|| items.last().expect("non-empty page").id.clone());
    Ok(MessagePage {
        items,
        next_after_id,
    })
}

pub fn start_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    content: &str,
) -> AppResult<RoundSnapshot> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::validation("content", "消息不能为空"));
    }
    let created_at = now();
    let transaction = connection.transaction()?;
    require_session(&transaction, persona_id, session_id)?;
    let pending = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM chat_rounds WHERE session_id=?1 AND status='pending')",
        [session_id],
        |row| row.get::<_, bool>(0),
    )?;
    if pending {
        return Err(AppError::new(
            "REQUEST_BUSY",
            "当前会话已有回复正在生成，请稍后重试",
        ));
    }
    transaction.execute(
        "INSERT INTO chat_rounds(session_id,status,created_at) VALUES(?1,'pending',?2)",
        params![session_id, created_at],
    )?;
    let round_id = transaction.last_insert_rowid();
    transaction.execute(
        "INSERT INTO messages(session_id,round_id,role,content,created_at) VALUES(?1,?2,'user',?3,?4)",
        params![session_id, round_id, content, created_at],
    )?;
    let message_id = transaction.last_insert_rowid();
    transaction.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
        params![created_at, session_id],
    )?;
    let snapshot = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    debug_assert_eq!(snapshot.user_message.id, message_id.to_string());
    transaction.commit()?;
    Ok(snapshot)
}

fn round_snapshot(
    connection: &Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
) -> AppResult<RoundSnapshot> {
    require_session(connection, persona_id, session_id)?;
    let round = connection
        .query_row(
            "SELECT id,session_id,status,created_at,completed_at FROM chat_rounds WHERE id=?1 AND session_id=?2",
            params![round_id, session_id],
            round_from_row,
        )
        .optional()?
        .ok_or_else(|| {
            AppError::new(
                "CONTEXT_CHANGED",
                "聊天回合不属于当前会话，请刷新后重试",
            )
        })?;
    let mut statement = connection.prepare(
        "SELECT id,session_id,round_id,role,content,expression,created_at
         FROM messages WHERE round_id=?1 AND session_id=?2 ORDER BY id",
    )?;
    let messages = statement
        .query_map(params![round_id, session_id], message_from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let user_message = messages
        .iter()
        .find(|message| message.role == MessageRole::User)
        .cloned()
        .ok_or_else(AppError::storage)?;
    let assistant_message = messages
        .into_iter()
        .find(|message| message.role == MessageRole::Assistant);
    Ok(RoundSnapshot {
        round,
        user_message,
        assistant_message,
    })
}

fn finish_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    content: Option<&str>,
    expression: Option<&str>,
    completed: bool,
) -> AppResult<RoundSnapshot> {
    let normalized = content.map(str::trim).filter(|value| !value.is_empty());
    if completed && normalized.is_none() {
        return Err(AppError::validation("content", "完整回复正文不能为空"));
    }
    let completed_at = now();
    let transaction = connection.transaction()?;
    let current = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    if current.round.status != ChatRoundStatus::Pending {
        transaction.commit()?;
        return Ok(current);
    }
    if let Some(content) = normalized {
        transaction.execute(
            "INSERT INTO messages(session_id,round_id,role,content,expression,created_at)
             VALUES(?1,?2,'assistant',?3,?4,?5)",
            params![session_id, round_id, content, expression, completed_at],
        )?;
    }
    let status = if completed {
        "completed"
    } else {
        "interrupted"
    };
    transaction.execute(
        "UPDATE chat_rounds SET status=?1,completed_at=?2 WHERE id=?3 AND session_id=?4 AND status='pending'",
        params![status, completed_at, round_id, session_id],
    )?;
    transaction.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
        params![completed_at, session_id],
    )?;
    let result = round_snapshot(&transaction, persona_id, session_id, round_id)?;
    transaction.commit()?;
    Ok(result)
}

pub fn complete_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    content: &str,
    expression: Option<&str>,
) -> AppResult<RoundSnapshot> {
    finish_round(
        connection,
        persona_id,
        session_id,
        round_id,
        Some(content),
        expression,
        true,
    )
}

pub fn interrupt_round(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    round_id: i64,
    partial_content: Option<&str>,
    expression: Option<&str>,
) -> AppResult<RoundSnapshot> {
    finish_round(
        connection,
        persona_id,
        session_id,
        round_id,
        partial_content,
        expression,
        false,
    )
}

pub fn append_proactive_message(
    connection: &mut Connection,
    persona_id: i64,
    session_id: i64,
    content: &str,
    expression: Option<&str>,
) -> AppResult<Message> {
    let content = content.trim();
    if content.is_empty() {
        return Err(AppError::validation("content", "主动消息不能为空"));
    }
    let created_at = now();
    let transaction = connection.transaction()?;
    require_session(&transaction, persona_id, session_id)?;
    transaction.execute(
        "INSERT INTO messages(session_id,round_id,role,content,expression,created_at)
         VALUES(?1,NULL,'assistant',?2,?3,?4)",
        params![session_id, content, expression, created_at],
    )?;
    let id = transaction.last_insert_rowid();
    transaction.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1 WHERE id=?2",
        params![created_at, session_id],
    )?;
    let result = transaction.query_row(
        "SELECT id,session_id,round_id,role,content,expression,created_at FROM messages WHERE id=?1",
        [id],
        message_from_row,
    )?;
    transaction.commit()?;
    Ok(result)
}

/// 启动时没有运行请求，所有遗留 pending 回合都收束为 interrupted。
pub(crate) fn recover_pending_in_transaction(
    connection: &Connection,
    recovered_at: &str,
) -> AppResult<usize> {
    connection.execute(
        "UPDATE sessions SET memory_revision=memory_revision+1,updated_at=?1
         WHERE id IN (SELECT DISTINCT session_id FROM chat_rounds WHERE status='pending')",
        [recovered_at],
    )?;
    Ok(connection.execute(
        "UPDATE chat_rounds SET status='interrupted',completed_at=?1 WHERE status='pending'",
        [recovered_at],
    )?)
}

pub fn recover_pending(connection: &mut Connection, recovered_at: &str) -> AppResult<usize> {
    let transaction = connection.transaction()?;
    let recovered = recover_pending_in_transaction(&transaction, recovered_at)?;
    transaction.commit()?;
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut connection).unwrap();
        connection
    }

    #[test]
    fn create_list_select_and_rename_keep_global_data() {
        let mut connection = database();
        connection
            .execute("UPDATE user_state SET coins=77", [])
            .unwrap();
        let first = create(&mut connection, 1).unwrap();
        let first_id = first.id.parse::<i64>().unwrap();
        connection.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(?1,'away','a','b','c','a')",[first_id]).unwrap();
        connection.execute("INSERT INTO letters(session_id,sender,content,reply_due_at,reply_state,letter_kind,created_at) VALUES(?1,'user','信','b','pending','user','a')",[first_id]).unwrap();
        let second = create(&mut connection, 1).unwrap();
        assert!(second.is_current);
        assert_eq!(list(&connection, 1).unwrap().len(), 2);
        assert_eq!(
            connection
                .query_row("SELECT status FROM travels", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "cancelled"
        );
        assert_eq!(
            connection
                .query_row("SELECT reply_state FROM letters", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "cancelled"
        );
        let renamed = rename(&mut connection, 1, first_id, " 旧朋友 ").unwrap();
        assert_eq!(renamed.title, "旧朋友");
        assert!(select(&mut connection, 1, first_id).unwrap());
        assert!(!select(&mut connection, 1, first_id).unwrap());
        assert!(get(&connection, 1, first_id).unwrap().is_current);
        assert_eq!(
            connection
                .query_row("SELECT coins FROM user_state", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            77
        );
        assert!(rename(&mut connection, 999, first_id, "越权").is_err());
    }

    #[test]
    fn rounds_are_atomic_idempotent_and_reject_cross_session_writes() {
        let mut connection = database();
        let first = create(&mut connection, 1).unwrap().id.parse().unwrap();
        let started = start_round(&mut connection, 1, first, " 你好 ").unwrap();
        let round_id = started.round.id.parse().unwrap();
        assert_eq!(started.round.status, ChatRoundStatus::Pending);
        assert_eq!(started.user_message.content, "你好");
        assert!(start_round(&mut connection, 1, first, "重复").is_err());
        let interrupted =
            interrupt_round(&mut connection, 1, first, round_id, Some("  "), None).unwrap();
        assert_eq!(interrupted.round.status, ChatRoundStatus::Interrupted);
        assert!(interrupted.assistant_message.is_none());
        let revision: i64 = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [first],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(revision, 2);

        let completed = start_round(&mut connection, 1, first, "第二轮").unwrap();
        let completed_id = completed.round.id.parse().unwrap();
        let result = complete_round(
            &mut connection,
            1,
            first,
            completed_id,
            "回答",
            Some("(^_^)"),
        )
        .unwrap();
        assert_eq!(result.round.status, ChatRoundStatus::Completed);
        assert_eq!(result.assistant_message.as_ref().unwrap().content, "回答");
        let revision: i64 = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [first],
                |row| row.get(0),
            )
            .unwrap();
        let repeated =
            complete_round(&mut connection, 1, first, completed_id, "不能覆盖", None).unwrap();
        assert_eq!(repeated.assistant_message.unwrap().content, "回答");
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [first],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            revision
        );

        let second = create(&mut connection, 1).unwrap().id.parse().unwrap();
        assert!(complete_round(&mut connection, 1, second, completed_id, "串会话", None).is_err());
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM messages WHERE round_id=?1",
                    [completed_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            2
        );
    }

    #[test]
    fn proactive_messages_page_in_id_order_and_recovery_is_one_time() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        for index in 0..3 {
            let message = append_proactive_message(
                &mut connection,
                1,
                session_id,
                &format!("主动 {index}"),
                None,
            )
            .unwrap();
            assert!(message.round_id.is_none());
            assert_eq!(message.role, MessageRole::Assistant);
        }
        let first = messages(&connection, 1, session_id, None, 2).unwrap();
        assert_eq!(first.items.len(), 2);
        let next = first.next_after_id.as_ref().unwrap().parse().unwrap();
        let second = messages(&connection, 1, session_id, Some(next), 2).unwrap();
        assert_eq!(second.items.len(), 1);
        assert!(second.next_after_id.is_none());
        assert!(messages(&connection, 1, session_id, None, 0).is_err());

        let pending = start_round(&mut connection, 1, session_id, "未完成").unwrap();
        let before: i64 = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            recover_pending(&mut connection, "2026-09-20T00:00:00.000Z").unwrap(),
            1
        );
        assert_eq!(
            recover_pending(&mut connection, "2026-09-20T00:01:00.000Z").unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before + 1
        );
        let recovered = round_snapshot(
            &connection,
            1,
            session_id,
            pending.round.id.parse().unwrap(),
        )
        .unwrap();
        assert_eq!(recovered.round.status, ChatRoundStatus::Interrupted);
        assert!(recovered.assistant_message.is_none());
    }

    #[test]
    fn failed_round_commit_rolls_back_messages_and_revision() {
        let mut connection = database();
        let session_id = create(&mut connection, 1).unwrap().id.parse().unwrap();
        connection.execute_batch("CREATE TRIGGER reject_message AFTER INSERT ON messages BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(start_round(&mut connection, 1, session_id, "不会半写").is_err());
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM chat_rounds", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        connection
            .execute_batch("DROP TRIGGER reject_message")
            .unwrap();
        let pending = start_round(&mut connection, 1, session_id, "用户消息").unwrap();
        let round_id = pending.round.id.parse::<i64>().unwrap();
        connection.execute_batch("CREATE TRIGGER reject_round BEFORE UPDATE ON chat_rounds BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        assert!(complete_round(
            &mut connection,
            1,
            session_id,
            round_id,
            "不能半写的回复",
            None
        )
        .is_err());
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM messages WHERE round_id=?1",
                    [round_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT status FROM chat_rounds WHERE id=?1",
                    [round_id],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "pending"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn database_initialization_recovers_orphan_pending_round_once() {
        let mut connection = database();
        let session_id = create(&mut connection, 1)
            .unwrap()
            .id
            .parse::<i64>()
            .unwrap();
        start_round(&mut connection, 1, session_id, "崩溃前已保存").unwrap();
        let before = connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=?1",
                [session_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        crate::db::initialize(&mut connection).unwrap();
        assert_eq!(
            connection
                .query_row("SELECT status FROM chat_rounds", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "interrupted"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before + 1
        );
        crate::db::initialize(&mut connection).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT memory_revision FROM sessions WHERE id=?1",
                    [session_id],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            before + 1
        );
    }
}
