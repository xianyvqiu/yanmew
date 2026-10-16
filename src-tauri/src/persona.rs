use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
pub mod background;

/// 聊天开始时固定的自调基准；只用于乐观校验，不保存修改历史。
pub(crate) struct EvolutionSnapshot {
    pub id: i64,
    pub prompt: String,
    pub initial_prompt: String,
    pub last_evolved_at: Option<String>,
}

pub(crate) fn evolution_snapshot(c: &Connection, id: i64) -> AppResult<Option<EvolutionSnapshot>> {
    Ok(c.query_row(
        "SELECT prompt,initial_prompt,last_evolved_at FROM personas WHERE id=?1 AND self_evolve_enabled=1",
        [id], |row| Ok(EvolutionSnapshot {
            id, prompt: row.get(0)?, initial_prompt: row.get(1)?, last_evolved_at: row.get(2)?,
        }),
    ).optional()?)
}

/// 仅在聊天已成功提交后调用；开关、当前/初始设定或自调时间变化时丢弃旧候选。
pub(crate) fn apply_evolution(c: &mut Connection, expected: &EvolutionSnapshot,
    new_prompt: &str, evolved_at: &str) -> AppResult<bool> {
    if new_prompt.trim().is_empty() {
        return Err(AppError::validation("new_prompt", "人设设定不能为空"));
    }
    if new_prompt.trim() == expected.prompt.trim() { return Ok(false); }
    let tx = c.transaction()?;
    let changed = tx.execute(
        "UPDATE personas SET prompt=?1,last_evolved_at=?2,updated_at=?2 WHERE id=?3 AND self_evolve_enabled=1 AND prompt=?4 AND initial_prompt=?5 AND last_evolved_at IS ?6",
        params![new_prompt, evolved_at, expected.id, expected.prompt, expected.initial_prompt, expected.last_evolved_at],
    )?;
    tx.commit()?;
    Ok(changed == 1)
}

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
    pub chat_appearance: background::Appearance,
    pub is_active: bool,
    pub is_builtin: bool,
    pub expressions: Option<serde_json::Value>,
    pub created_at:Option<String>,
    pub updated_at:Option<String>,
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
    /// 用户直接编辑初始设定；未提供时保留原初始内容。
    #[serde(default)]
    pub initial_prompt: Option<String>,
}
pub fn list(c: &Connection) -> AppResult<Vec<Persona>> {
    let mut stmt=c.prepare("SELECT id,name,prompt,initial_prompt,self_evolve_enabled,last_evolved_at,color,chat_bg_file,chat_bg_opacity,chat_bg_blur,is_active,is_builtin,expressions,created_at,updated_at,chat_bubble_style,chat_user_text_color,chat_assistant_text_color FROM personas ORDER BY id")?;
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
            chat_appearance: background::Appearance { bubble_style: r.get(15)?, user_text_color: r.get(16)?, assistant_text_color: r.get(17)? },
            is_active: r.get(10)?,
            is_builtin: r.get(11)?,
            expressions,
            created_at:r.get(13)?,updated_at:r.get(14)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}
pub fn save(c: &mut Connection, id: Option<i64>, input: PersonaInput) -> AppResult<Persona> {
    if input.initial_prompt.as_ref().is_some_and(|value| value.trim().is_empty()) {
        return Err(AppError::validation("initialPrompt", "初始设定不能为空"));
    }
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
    let creating=id.is_none();
    let id = if let Some(id) = id {
        let changed=tx.execute("UPDATE personas SET name=?1,prompt=?2,initial_prompt=CASE WHEN ?3 THEN ?2 ELSE COALESCE(?9,initial_prompt) END,self_evolve_enabled=?4,color=?5,chat_bg_opacity=?6,chat_bg_blur=?7 WHERE id=?8",params![input.name.trim(),input.prompt,input.save_as_initial,input.self_evolve_enabled,input.color,input.chat_bg_opacity,input.chat_bg_blur,id,input.initial_prompt])?;
        if changed == 0 {
            return Err(AppError::not_found());
        }
        id
    } else {
        tx.execute("INSERT INTO personas(name,prompt,initial_prompt,self_evolve_enabled,color,chat_bg_opacity,chat_bg_blur) VALUES(?1,?2,?7,?3,?4,?5,?6)",params![input.name.trim(),input.prompt,input.self_evolve_enabled,input.color,input.chat_bg_opacity,input.chat_bg_blur,input.initial_prompt.unwrap_or_else(||input.prompt.clone())])?;
        tx.last_insert_rowid()
    };
    tx.execute("UPDATE personas SET created_at=CASE WHEN ?2 THEN ?3 ELSE created_at END,updated_at=?3 WHERE id=?1",params![id,creating,chrono::Utc::now().to_rfc3339()])?;
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
        let mut c = crate::db::test_database();
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
        let mut c = crate::db::test_database();
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
                initial_prompt: None,
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
