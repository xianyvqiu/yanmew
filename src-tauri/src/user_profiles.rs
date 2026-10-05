//! 全局用户设定仓储；删除由外键解除会话引用，不依赖窗口。
use crate::{
    error::{AppError, AppResult},
    models::UserProfile,
};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

fn from_row(row: &Row<'_>) -> rusqlite::Result<UserProfile> {
    Ok(UserProfile {
        id: row.get::<_, i64>(0)?.to_string(),
        name: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}
fn validate<'a>(name: &'a str, content: &'a str) -> AppResult<(&'a str, &'a str)> {
    let (name, content) = (name.trim(), content.trim());
    if name.is_empty() {
        return Err(AppError::validation("name", "用户设定名称不能为空"));
    }
    if content.is_empty() {
        return Err(AppError::validation("content", "用户设定内容不能为空"));
    }
    Ok((name, content))
}
pub fn get(conn: &Connection, id: i64) -> AppResult<UserProfile> {
    conn.query_row(
        "SELECT id,name,content,created_at,updated_at FROM user_profiles WHERE id=?1",
        [id],
        from_row,
    )
    .optional()?
    .ok_or_else(AppError::not_found)
}
pub fn list(conn: &Connection) -> AppResult<Vec<UserProfile>> {
    let mut stmt=conn.prepare("SELECT id,name,content,created_at,updated_at FROM user_profiles ORDER BY updated_at DESC,id DESC")?;
    let items = stmt
        .query_map([], from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}
pub fn create(conn: &Connection, name: &str, content: &str) -> AppResult<i64> {
    let (name, content) = validate(name, content)?;
    let stamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    conn.execute(
        "INSERT INTO user_profiles(name,content,created_at,updated_at) VALUES(?1,?2,?3,?3)",
        params![name, content, stamp],
    )?;
    Ok(conn.last_insert_rowid())
}
pub fn update(conn: &Connection, id: i64, name: &str, content: &str) -> AppResult<()> {
    let (name, content) = validate(name, content)?;
    if conn.execute(
        "UPDATE user_profiles SET name=?1,content=?2,updated_at=?3 WHERE id=?4",
        params![
            name,
            content,
            Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            id
        ],
    )? == 0
    {
        return Err(AppError::not_found());
    }
    Ok(())
}
pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM user_profiles WHERE id=?1", [id])? == 0 {
        return Err(AppError::not_found());
    }
    Ok(())
}

/// 即使资料写入失败，也通知界面补查已提交的中断回合。
pub(crate) struct Change<T> {
    pub result: AppResult<T>,
    pub sessions: Vec<i64>,
}

fn bound_sessions(conn: &Connection, id: i64) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT id FROM sessions WHERE user_profile_id=?1 ORDER BY id")?;
    let ids = stmt
        .query_map([id], |r| r.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// 调用者持有唯一 store 锁，只取消指定会话，不等待网络任务退出。
fn interrupt(
    runtime: &crate::runtime::Runtime,
    store: &mut crate::runtime::Store,
    sessions: &[i64],
) -> AppResult<()> {
    for session in sessions {
        if let Some(active) = store.chat_requests.get(session).cloned() {
            let persona = crate::error::parse_id(&active.persona_id)?;
            let round = crate::error::parse_id(&active.round_id)?;
            // 校验回合存在后才取消；重说预览不进入原消息或记忆。
            crate::sessions::round_snapshot(&store.connection, persona, *session, round)?;
            runtime.requests.cancel_key(&format!("chat:{session}"));
            if !active.regenerating {
                let saved = crate::sessions::interrupt_round_with_reason(
                    &mut store.connection,
                    persona,
                    *session,
                    round,
                    Some(&active.partial_content),
                    active.expression.as_deref(),
                    "user_cancelled",
                )?;
                if let Some(current) = store.chat_requests.get_mut(session) {
                    current.round = saved;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn update_in_runtime(
    runtime: &crate::runtime::Runtime,
    id: i64,
    name: &str,
    content: &str,
) -> AppResult<Change<UserProfile>> {
    let (name, content) = validate(name, content)?;
    runtime.read(|store, _| {
        runtime.jobs_cancel.check()?;
        let old = get(&store.connection, id)?;
        if old.name == name && old.content == content {
            return Ok(Change {
                result: Ok(old),
                sessions: vec![],
            });
        }
        let sessions = bound_sessions(&store.connection, id)?;
        interrupt(runtime, store, &sessions)?;
        let result =
            update(&store.connection, id, name, content).and_then(|_| get(&store.connection, id));
        Ok(Change { result, sessions })
    })
}

pub(crate) fn delete_in_runtime(
    runtime: &crate::runtime::Runtime,
    id: i64,
) -> AppResult<Change<()>> {
    runtime.read(|store, _| {
        runtime.jobs_cancel.check()?;
        get(&store.connection, id)?;
        let sessions = bound_sessions(&store.connection, id)?;
        interrupt(runtime, store, &sessions)?;
        let result = delete(&store.connection, id);
        Ok(Change { result, sessions })
    })
}

pub(crate) fn bind_in_runtime(
    runtime: &crate::runtime::Runtime,
    persona: i64,
    session: i64,
    revision: u64,
    profile: Option<i64>,
) -> AppResult<Change<crate::models::Session>> {
    runtime.read(|store, _| {
        runtime.jobs_cancel.check()?;
        let context = crate::persona::context(&store.connection, store.context_revision)?;
        if context.persona_id != persona.to_string()
            || context.context_revision != revision
            || context.session_id != Some(session.to_string())
        {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前人设或会话已经改变，请刷新后重试",
            ));
        }
        let old = crate::sessions::get(&store.connection, persona, session)?;
        if let Some(id) = profile {
            get(&store.connection, id)?;
        }
        if old.user_profile_id == profile.map(|id| id.to_string()) {
            return Ok(Change {
                result: Ok(old),
                sessions: vec![],
            });
        }
        let sessions = vec![session];
        interrupt(runtime, store, &sessions)?;
        let result =
            crate::sessions::set_user_profile(&mut store.connection, persona, session, profile);
        Ok(Change { result, sessions })
    })
}

#[cfg(test)]
#[path = "user_profiles/runtime_tests.rs"]
mod runtime_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn database() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        c
    }
    #[test]
    fn crud_validates_trims_and_preserves_creation_and_string_ids() {
        let c = database();
        assert!(list(&c).unwrap().is_empty());
        assert_eq!(
            create(&c, " ", "内容").unwrap_err().code,
            "VALIDATION_ERROR"
        );
        assert_eq!(
            create(&c, "名称", "\n").unwrap_err().code,
            "VALIDATION_ERROR"
        );
        let id = create(&c, " 背景 ", " 第一行\n第二行 ").unwrap();
        let saved = get(&c, id).unwrap();
        assert_eq!(saved.name, "背景");
        assert_eq!(saved.content, "第一行\n第二行");
        assert!(chrono::DateTime::parse_from_rfc3339(&saved.created_at).is_ok());
        assert_eq!(serde_json::to_value(&saved).unwrap()["id"], id.to_string());
        update(&c, id, "新名称", &"字".repeat(20_000)).unwrap();
        let updated = get(&c, id).unwrap();
        assert_eq!(updated.created_at, saved.created_at);
        assert_eq!(updated.content.chars().count(), 20_000);
        assert_eq!(list(&c).unwrap(), vec![updated]);
        assert_eq!(
            update(&c, id, "", "内容").unwrap_err().code,
            "VALIDATION_ERROR"
        );
        assert_eq!(
            update(&c, id + 1, "名称", "内容").unwrap_err().code,
            "NOT_FOUND"
        );
        assert_eq!(delete(&c, id + 1).unwrap_err().code, "NOT_FOUND");
    }
    #[test]
    fn deleting_shared_profile_unbinds_without_removing_sessions_or_history() {
        let c = database();
        let id = create(&c, "共享资料", "背景").unwrap();
        c.execute("INSERT INTO sessions(id,persona_id,user_profile_id,title,created_at,updated_at) VALUES(1,1,?1,'甲','a','a'),(2,1,?1,'乙','a','a')",[id]).unwrap();
        c.execute("INSERT INTO messages(session_id,role,content,created_at) VALUES(1,'assistant','已有消息','a')",[]).unwrap();
        delete(&c, id).unwrap();
        assert_eq!(
            c.query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_profile_id IS NULL",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
        assert_eq!(
            c.query_row("SELECT content FROM messages", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "已有消息"
        );
        assert!(list(&c).unwrap().is_empty());
    }
    #[test]
    fn session_creation_and_binding_are_atomic_and_do_not_rewrite_history() {
        let mut c = database();
        let id = create(&c, "背景", "内容").unwrap();
        let first = crate::sessions::create_with_profile(&mut c, 1, Some(id)).unwrap();
        assert_eq!(first.user_profile_id, Some(id.to_string()));
        let sid = first.id.parse::<i64>().unwrap();
        c.execute("INSERT INTO messages(session_id,role,content,created_at) VALUES(?1,'assistant','历史','a')",[sid]).unwrap();
        assert_eq!(
            crate::sessions::create_with_profile(&mut c, 1, Some(id + 1))
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
        assert!(crate::sessions::get(&c, 1, sid).unwrap().is_current);
        assert_eq!(crate::sessions::list(&c, 1).unwrap().len(), 1);
        assert_eq!(
            crate::sessions::set_user_profile(&mut c, 2, sid, None)
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
        assert_eq!(
            crate::sessions::set_user_profile(&mut c, 1, sid, Some(id + 1))
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
        assert_eq!(
            crate::sessions::get(&c, 1, sid).unwrap().user_profile_id,
            Some(id.to_string())
        );
        let unbound = crate::sessions::set_user_profile(&mut c, 1, sid, None).unwrap();
        assert!(unbound.user_profile_id.is_none());
        assert_eq!(
            (unbound.memory_revision, unbound.intimacy),
            (first.memory_revision, first.intimacy)
        );
        assert_eq!(
            c.query_row("SELECT content FROM messages", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "历史"
        );
        let empty = crate::sessions::create(&mut c, 1).unwrap();
        assert!(empty.user_profile_id.is_none());
    }
}
