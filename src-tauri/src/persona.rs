use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub initial_prompt: String,
    pub self_evolve_enabled: bool,
    pub last_evolved_at: Option<String>,
    pub color: String,
    pub chat_bg_file: Option<String>,
    pub chat_bg_opacity: i64,
    pub chat_bg_blur: i64,
    pub is_active: bool,
    pub is_builtin: bool,
    pub expressions: Option<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonaInput {
    pub name: String,
    pub prompt: String,
    pub self_evolve_enabled: bool,
    pub color: String,
    pub chat_bg_opacity: i64,
    pub chat_bg_blur: i64,
    #[serde(default)]
    pub save_as_initial: bool,
}
pub fn list(c: &Connection) -> AppResult<Vec<Persona>> {
    let mut stmt=c.prepare("SELECT id,name,prompt,initial_prompt,self_evolve_enabled,last_evolved_at,color,chat_bg_file,chat_bg_opacity,chat_bg_blur,is_active,is_builtin,expressions FROM personas ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        let raw: Option<String> = r.get(12)?;
        let expressions = raw
            .map(|v| serde_json::from_str(&v))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        Ok(Persona {
            id: r.get::<_, i64>(0)?.to_string(),
            name: r.get(1)?,
            prompt: r.get(2)?,
            initial_prompt: r.get(3)?,
            self_evolve_enabled: r.get(4)?,
            last_evolved_at: r.get(5)?,
            color: r.get(6)?,
            chat_bg_file: r.get(7)?,
            chat_bg_opacity: r.get(8)?,
            chat_bg_blur: r.get(9)?,
            is_active: r.get(10)?,
            is_builtin: r.get(11)?,
            expressions,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}
pub fn save(c: &mut Connection, id: Option<i64>, input: PersonaInput) -> AppResult<Persona> {
    if input.name.trim().is_empty() || input.prompt.trim().is_empty() {
        return Err(AppError::validation("prompt", "人设名称和设定不能为空"));
    }
    if input.color.len() != 7
        || !input.color.starts_with('#')
        || !input.color[1..].bytes().all(|v| v.is_ascii_hexdigit())
    {
        return Err(AppError::validation("color", "颜色必须为 #RRGGBB 格式"));
    }
    if !(0..=100).contains(&input.chat_bg_opacity) || !(0..=40).contains(&input.chat_bg_blur) {
        return Err(AppError::validation(
            "chatBgOpacity",
            "背景透明度须为 0–100，模糊须为 0–40",
        ));
    }
    let tx = c.transaction()?;
    let id = if let Some(id) = id {
        let changed=tx.execute("UPDATE personas SET name=?1,prompt=?2,initial_prompt=CASE WHEN ?3 THEN ?2 ELSE initial_prompt END,self_evolve_enabled=?4,color=?5,chat_bg_opacity=?6,chat_bg_blur=?7 WHERE id=?8",params![input.name.trim(),input.prompt,input.save_as_initial,input.self_evolve_enabled,input.color,input.chat_bg_opacity,input.chat_bg_blur,id])?;
        if changed == 0 {
            return Err(AppError::not_found());
        }
        id
    } else {
        tx.execute("INSERT INTO personas(name,prompt,initial_prompt,self_evolve_enabled,color,chat_bg_opacity,chat_bg_blur) VALUES(?1,?2,?2,?3,?4,?5,?6)",params![input.name.trim(),input.prompt,input.self_evolve_enabled,input.color,input.chat_bg_opacity,input.chat_bg_blur])?;
        tx.last_insert_rowid()
    };
    let result = list(&tx)?
        .into_iter()
        .find(|p| p.id == id.to_string())
        .ok_or_else(AppError::not_found)?;
    tx.commit()?;
    Ok(result)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub persona_id: String,
    pub session_id: Option<String>,
    pub context_revision: u64,
}
pub fn context(c: &Connection, revision: u64) -> AppResult<Context> {
    Ok(Context {
        persona_id: c
            .query_row("SELECT id FROM personas WHERE is_active=1", [], |r| {
                r.get::<_, i64>(0)
            })?
            .to_string(),
        session_id: c
            .query_row(
                "SELECT value FROM settings WHERE key='current_session_id'",
                [],
                |r| r.get(0),
            )
            .optional()?,
        context_revision: revision,
    })
}
fn select_in_transaction(c: &Connection, id: i64) -> AppResult<()> {
    let active = c
        .query_row("SELECT is_active FROM personas WHERE id=?1", [id], |r| {
            r.get::<_, bool>(0)
        })
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if active {
        return Ok(());
    }
    // 保留已生成的信件和事实，仅取消离开会话中尚未完成的旅行任务。
    crate::sessions::cancel_current_travel_and_replies(
        c,
        &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )?;
    c.execute("UPDATE personas SET is_active=0 WHERE is_active=1", [])?;
    c.execute("UPDATE personas SET is_active=1 WHERE id=?1", [id])?;
    c.execute("DELETE FROM settings WHERE key='current_session_id'", [])?;
    c.execute("INSERT INTO settings(key,value) SELECT 'current_session_id',CAST(id AS TEXT) FROM sessions WHERE persona_id=?1 ORDER BY updated_at DESC,id DESC LIMIT 1",[id])?;
    Ok(())
}
pub fn select(c: &mut Connection, id: i64) -> AppResult<()> {
    let tx = c.transaction()?;
    select_in_transaction(&tx, id)?;
    tx.commit()?;
    Ok(())
}
pub fn delete(c: &mut Connection, id: i64) -> AppResult<()> {
    let tx = c.transaction()?;
    let builtin = tx
        .query_row("SELECT is_builtin FROM personas WHERE id=?1", [id], |r| {
            r.get::<_, bool>(0)
        })
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if builtin {
        return Err(AppError::new("VALIDATION_ERROR", "系统默认人设不能删除"));
    }
    let active = tx.query_row("SELECT is_active FROM personas WHERE id=?1", [id], |r| {
        r.get::<_, bool>(0)
    })?;
    if active {
        let fallback = tx.query_row("SELECT id FROM personas WHERE is_builtin=1", [], |r| {
            r.get(0)
        })?;
        select_in_transaction(&tx, fallback)?;
    }
    // 先解除来源 RESTRICT，再由外键级联清理人设及其会话。
    tx.execute("DELETE FROM long_memories WHERE session_id IN (SELECT id FROM sessions WHERE persona_id=?1)",[id])?;
    tx.execute("DELETE FROM personas WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleting_persona_cleans_memory_sources_and_preserves_wallet() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        c.execute_batch("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(2,'测试','设定','设定');
          INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,2,'会话','a','a');
          INSERT INTO chat_rounds(id,session_id,status,created_at) VALUES(1,2,'completed','a'),(2,2,'completed','b');
          INSERT INTO short_memories(id,session_id,round_id,content,created_at,updated_at) VALUES(1,2,1,'事实','a','a');
          INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(1,2,'标题','事实',1,2,'b','b');
          INSERT INTO long_memory_sources(long_memory_id,short_memory_id,source_order) VALUES(1,1,1);
          UPDATE user_state SET coins=99;").unwrap();
        delete(&mut c, 2).unwrap();
        for table in [
            "sessions",
            "chat_rounds",
            "short_memories",
            "long_memories",
            "long_memory_sources",
        ] {
            assert_eq!(
                c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        assert_eq!(
            c.query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            99
        );
        assert_eq!(context(&c, 0).unwrap().persona_id, "1");
    }
    #[test]
    fn switch_cancel_and_delete_current_persona() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        assert!(delete(&mut c, 1).is_err());
        let p = save(
            &mut c,
            None,
            PersonaInput {
                name: "另一只猫".into(),
                prompt: "你好".into(),
                self_evolve_enabled: true,
                color: "#aabbcc".into(),
                chat_bg_opacity: 85,
                chat_bg_blur: 0,
                save_as_initial: false,
            },
        )
        .unwrap();
        c.execute(
            "INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(1,'会话','a','a')",
            [],
        )
        .unwrap();
        c.execute("INSERT INTO settings VALUES('current_session_id','1')", [])
            .unwrap();
        c.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(1,'away','a','b','c','a')",[]).unwrap();
        assert!(select(&mut c, 999).is_err());
        assert_eq!(context(&c, 0).unwrap().persona_id, "1");
        select(&mut c, p.id.parse().unwrap()).unwrap();
        assert_eq!(
            c.query_row("SELECT status FROM travels", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "cancelled"
        );
        assert_eq!(context(&c, 0).unwrap().session_id, None);
        delete(&mut c, p.id.parse().unwrap()).unwrap();
        assert_eq!(context(&c, 0).unwrap().persona_id, "1");
        assert_eq!(context(&c, 0).unwrap().session_id, Some("1".into()));
    }
}
