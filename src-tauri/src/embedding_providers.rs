//! Embedding 配置独立于聊天模型；密钥只在后端解密，不进入界面 DTO。
use crate::{
    crypto,
    error::{AppError, AppResult},
    platform::contracts::SecretProtector,
    providers::{ConnectionTest, ProviderInput},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub model: String,
    pub has_api_key: bool,
    pub is_builtin: bool,
    pub is_active: bool,
}

/// 包括密文的固定配置快照只供后端比较；不实现 Debug/Serialize。
#[derive(Clone, PartialEq)]
pub struct Stamp {
    pub id: i64,
    pub kind: String,
    pub base_url: Option<String>,
    pub model: String,
    pub encrypted: Option<String>,
}
impl Stamp {
    pub fn connection(&self, secrets: &dyn SecretProtector) -> AppResult<ConnectionTest> {
        Ok(ConnectionTest {
            max_output_tokens: crate::providers::default_max_output_tokens(),
            context_window_override: None,
            base_url: self.base_url.clone().unwrap_or_default(),
            model: self.model.clone(),
            api_key: self
                .encrypted
                .as_ref()
                .map(|key| secrets.unprotect(key))
                .transpose()?,
        })
    }
}
pub fn stamp(c: &Connection, id: Option<i64>) -> AppResult<Stamp> {
    c.query_row("SELECT id,kind,base_url,model,api_key_encrypted FROM embedding_providers WHERE (?1 IS NULL AND is_active=1) OR id=?1", [id], |r| {
        Ok(Stamp { id: r.get(0)?, kind: r.get(1)?, base_url: r.get(2)?, model: r.get(3)?, encrypted: r.get(4)? })
    }).optional()?.ok_or_else(AppError::not_found)
}
pub fn list(c: &Connection) -> AppResult<Vec<Provider>> {
    let mut statement = c.prepare("SELECT id,name,kind,base_url,model,api_key_encrypted IS NOT NULL,is_builtin,is_active FROM embedding_providers ORDER BY is_builtin DESC,id")?;
    let rows = statement.query_map([], |r| {
        Ok(Provider {
            id: r.get::<_, i64>(0)?.to_string(),
            name: r.get(1)?,
            kind: r.get(2)?,
            base_url: r.get(3)?,
            model: r.get(4)?,
            has_api_key: r.get(5)?,
            is_builtin: r.get(6)?,
            is_active: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
pub fn save(
    c: &mut Connection,
    id: Option<i64>,
    input: ProviderInput,
    secrets: &dyn SecretProtector,
) -> AppResult<Provider> {
    if input.name.trim().is_empty() {
        return Err(AppError::validation("name", "请填写提供商名称"));
    }
    if input.model.trim().is_empty() {
        return Err(AppError::validation("model", "请填写模型名称"));
    }
    let url = input
        .base_url
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            AppError::validation(
                "baseUrl",
                "请填写 Embedding 接口地址，例如 https://api.openai.com/v1",
            )
        })?;
    crate::providers::validate_url(url.trim())?;
    let key = crypto::prepare(input.api_key, secrets)?;
    let tx = c.transaction()?;
    let id = if let Some(id) = id {
        let (builtin, old): (bool, Option<String>) = tx
            .query_row(
                "SELECT is_builtin,api_key_encrypted FROM embedding_providers WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(AppError::not_found)?;
        if builtin {
            return Err(AppError::new("VALIDATION_ERROR", "内置本地模型不能编辑"));
        }
        tx.execute("UPDATE embedding_providers SET name=?1,base_url=?2,model=?3,api_key_encrypted=?4 WHERE id=?5", params![input.name.trim(),url.trim().trim_end_matches('/'),input.model.trim(),key.unwrap_or(old),id])?;
        id
    } else {
        tx.execute("INSERT INTO embedding_providers(name,kind,base_url,model,api_key_encrypted) VALUES(?1,'online',?2,?3,?4)", params![input.name.trim(),url.trim().trim_end_matches('/'),input.model.trim(),key.flatten()])?;
        tx.last_insert_rowid()
    };
    let result = list(&tx)?
        .into_iter()
        .find(|p| p.id == id.to_string())
        .ok_or_else(AppError::not_found)?;
    tx.commit()?;
    Ok(result)
}
pub fn select(c: &mut Connection, id: i64) -> AppResult<()> {
    let tx = c.transaction()?;
    let selected = stamp(&tx, Some(id))?;
    let same = stamp(&tx, None)? == selected;
    if !same {
        tx.execute(
            "UPDATE embedding_providers SET is_active=0 WHERE is_active=1",
            [],
        )?;
        tx.execute(
            "UPDATE embedding_providers SET is_active=1 WHERE id=?1",
            [id],
        )?;
        tx.execute("DELETE FROM long_memory_embeddings", [])?;
    }
    tx.commit()?;
    Ok(())
}
pub fn delete(c: &mut Connection, id: i64) -> AppResult<()> {
    let tx = c.transaction()?;
    let (builtin, active): (bool, bool) = tx
        .query_row(
            "SELECT is_builtin,is_active FROM embedding_providers WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if builtin || active {
        return Err(AppError::new(
            "VALIDATION_ERROR",
            "内置模型和正在使用的 Embedding 提供商不能删除，请先切换",
        ));
    }
    tx.execute("DELETE FROM embedding_providers WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Secret;
    impl SecretProtector for Secret {
        fn protect(&self, value: &str) -> AppResult<String> {
            Ok(format!("encrypted:{value}"))
        }
        fn unprotect(&self, value: &str) -> AppResult<String> {
            Ok(value.trim_start_matches("encrypted:").into())
        }
    }
    fn input(key: crypto::SecretUpdate) -> ProviderInput {
        ProviderInput {
            max_output_tokens: 32768,
            context_window_override: None,
            name: "联网模型".into(),
            base_url: Some("https://example.com/v1/".into()),
            model: "model".into(),
            api_key: key,
        }
    }
    #[test]
    fn builtin_protection_secret_updates_and_transactional_selection() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        let builtin = stamp(&c, None).unwrap().id;
        assert!(save(
            &mut c,
            Some(builtin),
            input(crypto::SecretUpdate::Keep),
            &Secret
        )
        .is_err());
        assert!(delete(&mut c, builtin).is_err());
        let provider = save(
            &mut c,
            None,
            input(crypto::SecretUpdate::Set {
                value: "private-key".into(),
            }),
            &Secret,
        )
        .unwrap();
        let json = serde_json::to_string(&provider).unwrap();
        assert!(!json.contains("private-key"));
        assert!(!json.contains("encrypted"));
        let id = crate::error::parse_id(&provider.id).unwrap();
        assert!(
            save(&mut c, Some(id), input(crypto::SecretUpdate::Keep), &Secret)
                .unwrap()
                .has_api_key
        );
        assert_eq!(
            stamp(&c, Some(id))
                .unwrap()
                .connection(&Secret)
                .unwrap()
                .api_key
                .as_deref(),
            Some("private-key")
        );
        select(&mut c, id).unwrap();
        assert!(delete(&mut c, id).is_err());
        assert!(select(&mut c, 999).is_err());
        assert_eq!(stamp(&c, None).unwrap().id, id);
        assert!(
            !save(
                &mut c,
                Some(id),
                input(crypto::SecretUpdate::Clear),
                &Secret
            )
            .unwrap()
            .has_api_key
        );
        select(&mut c, builtin).unwrap();
        delete(&mut c, id).unwrap();
        let mut bad = input(crypto::SecretUpdate::Keep);
        bad.base_url = None;
        assert!(save(&mut c, None, bad, &Secret).is_err());
    }
}
