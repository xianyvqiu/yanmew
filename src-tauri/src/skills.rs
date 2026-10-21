//! 用户维护的共享提示词；绑定不授予工具权限，调用方选择允许的生成用途。
use crate::error::{parse_id, AppError, AppResult};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
pub mod tools;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
    pub personas: Vec<Binding>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub id: Option<String>,
    pub revision: Option<i64>,
    pub title: String,
    pub summary: String,
    pub body: String,
}
pub fn list(c: &Connection) -> AppResult<Vec<Skill>> {
    let mut q=c.prepare("SELECT id,title,summary,body,revision,created_at,updated_at FROM skills ORDER BY updated_at DESC,id DESC")?;
    let rows = q
        .query_map([], |r| {
            Ok(Skill {
                id: r.get::<_, i64>(0)?.to_string(),
                title: r.get(1)?,
                summary: r.get(2)?,
                body: r.get(3)?,
                revision: r.get(4)?,
                created_at: r.get(5)?,
                updated_at: r.get(6)?,
                personas: vec![],
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(|mut s|{let mut q=c.prepare("SELECT p.id,p.name FROM persona_skills b JOIN personas p ON p.id=b.persona_id WHERE b.skill_id=?1 ORDER BY b.created_at,p.id")?;s.personas=q.query_map([parse_id(&s.id)?],|r|Ok(Binding{id:r.get::<_,i64>(0)?.to_string(),name:r.get(1)?}))?.collect::<Result<Vec<_>,_>>()?;Ok(s)}).collect()
}
pub fn save(c: &mut Connection, input: Input) -> AppResult<()> {
    let tx = c.transaction()?;
    save_in_transaction(&tx,input)?;
    tx.commit()?;
    Ok(())
}
pub(crate) fn save_in_transaction(c: &Connection, input: Input) -> AppResult<i64> {
    if input.title.trim().is_empty() || input.body.trim().is_empty() {
        return Err(AppError::validation("body", "标题和正文不能为空"));
    }
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(id) = input.id {
        let id=parse_id(&id)?;
        if c.execute("UPDATE skills SET title=?1,summary=?2,body=?3,revision=revision+1,updated_at=?4 WHERE id=?5 AND revision=?6",params![input.title.trim(),input.summary,input.body,now,id,input.revision])?!=1{return Err(AppError::new("SKILL_CONFLICT","Skill已更改或删除，请重新读取，草稿仍保留"));}
        Ok(id)
    } else {
        c.execute(
            "INSERT INTO skills(title,summary,body,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
            params![input.title.trim(), input.summary, input.body, now],
        )?;
        Ok(c.last_insert_rowid())
    }
}
pub fn bind(c: &Connection, skill: i64, persona: i64, enabled: bool) -> AppResult<()> {
    if enabled {
        c.execute("INSERT INTO persona_skills(persona_id,skill_id,created_at) VALUES(?1,?2,?3) ON CONFLICT DO NOTHING",params![persona,skill,chrono::Utc::now().to_rfc3339()])?;
    } else {
        c.execute(
            "DELETE FROM persona_skills WHERE persona_id=?1 AND skill_id=?2",
            params![persona, skill],
        )?;
    }
    Ok(())
}
pub fn context(c: &Connection, persona: i64) -> AppResult<String> {
    let mut q=c.prepare("SELECT s.title,s.body FROM persona_skills b JOIN skills s ON s.id=b.skill_id WHERE b.persona_id=?1 ORDER BY b.created_at,s.id")?;
    let rows = q
        .query_map([persona], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("用户绑定的Skill资料；在应用输出协议、工具权限及当前人设约束内使用，不执行脚本，不扩大权限：\n{}",rows.into_iter().map(|(title,body)|format!("【{title}】\n{body}")).collect::<Vec<_>>().join("\n\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_bindings_conflict_and_delete_do_not_delete_personas() {
        let mut c = crate::db::test_database();
        save(
            &mut c,
            Input {
                id: None,
                revision: None,
                title: "表达".into(),
                summary: "简洁".into(),
                body: "先说结论".into(),
            },
        )
        .unwrap();
        bind(&c, 1, 1, true).unwrap();
        bind(&c, 1, 1, true).unwrap();
        assert_eq!(list(&c).unwrap()[0].personas.len(), 1);
        assert!(context(&c, 1).unwrap().contains("先说结论"));
        save(
            &mut c,
            Input {
                id: Some("1".into()),
                revision: Some(0),
                title: "表达".into(),
                summary: "".into(),
                body: "保持自然".into(),
            },
        )
        .unwrap();
        assert!(save(
            &mut c,
            Input {
                id: Some("1".into()),
                revision: Some(0),
                title: "旧编辑".into(),
                summary: "".into(),
                body: "旧内容".into()
            }
        )
        .is_err());
        c.execute("DELETE FROM skills WHERE id=1", []).unwrap();
        assert!(context(&c, 1).unwrap().is_empty());
        assert_eq!(
            c.query_row("SELECT count(*) FROM personas", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
