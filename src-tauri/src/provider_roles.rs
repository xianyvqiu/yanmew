//! 通用连接的用途绑定。Key仅从providers读取，用途不复制密钥。
use crate::{
    error::{parse_id, AppError, AppResult},
    platform::contracts::SecretProtector,
    providers::{self, ConnectionTest},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Role {
    pub purpose: String,
    pub provider_id: String,
    pub model: String,
    pub enabled: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub revision: u64,
    pub roles: Vec<Role>,
    pub models: Vec<ModelConfig>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub purpose: String,
    pub provider_id: String,
    pub model: String,
    pub max_output_tokens: u32,
    pub context_window_override: Option<u32>,
}
pub fn revision(c: &Connection) -> AppResult<u64> {
    let value: Option<String> = c
        .query_row(
            "SELECT value FROM settings WHERE key='provider_roles_revision'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    value
        .map(|v| v.parse().map_err(|_| AppError::storage()))
        .unwrap_or(Ok(0))
}
pub fn changed(c: &Connection) -> AppResult<()> {
    let next = revision(c)?.checked_add(1).ok_or_else(AppError::storage)?;
    c.execute("INSERT INTO settings(key,value) VALUES('provider_roles_revision',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[next.to_string()])?;
    Ok(())
}
pub fn image_revision(c: &Connection) -> AppResult<u64> {
    let value: Option<String> = c
        .query_row(
            "SELECT value FROM settings WHERE key='image_generation_shared_revision'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    value
        .map(|v| v.parse().map_err(|_| AppError::storage()))
        .unwrap_or(Ok(0))
}
pub fn image_changed(c: &Connection) -> AppResult<()> {
    let next = image_revision(c)?
        .checked_add(1)
        .ok_or_else(AppError::storage)?;
    c.execute("INSERT INTO settings(key,value) VALUES('image_generation_shared_revision',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[next.to_string()])?;
    Ok(())
}
pub fn get(c: &Connection) -> AppResult<Config> {
    let mut q=c.prepare("SELECT r.purpose,r.provider_id,m.model,r.enabled FROM provider_roles r JOIN provider_models m ON m.provider_id=r.provider_id AND m.purpose=r.purpose ORDER BY r.purpose")?;
    let roles = q
        .query_map([], |r| {
            Ok(Role {
                purpose: r.get(0)?,
                provider_id: r.get::<_, i64>(1)?.to_string(),
                model: r.get(2)?,
                enabled: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut q=c.prepare("SELECT purpose,provider_id,model,max_output_tokens,context_window_override FROM provider_models ORDER BY provider_id,purpose")?;
    let models = q
        .query_map([], |r| {
            Ok(ModelConfig {
                purpose: r.get(0)?,
                provider_id: r.get::<_, i64>(1)?.to_string(),
                model: r.get(2)?,
                max_output_tokens: r.get(3)?,
                context_window_override: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Config {
        revision: revision(c)?,
        roles,
        models,
    })
}
pub fn role(c: &Connection, purpose: &str) -> AppResult<Option<Role>> {
    Ok(get(c)?.roles.into_iter().find(|r| r.purpose == purpose))
}
pub fn connection(
    c: &Connection,
    id: i64,
    purpose: &str,
    p: &dyn SecretProtector,
) -> AppResult<ConnectionTest> {
    let mut value = providers::connection_test_config(c, id, p)?;
    if let Some(role) = role(c, purpose)?.filter(|r| r.provider_id == id.to_string()) {
        value.model = role.model;
    }
    Ok(value)
}
pub fn save(
    c: &mut Connection,
    purpose: &str,
    id: Option<&str>,
    model: &str,
    enabled: bool,
    expected: u64,
    max_output: Option<u32>,
    capacity: Option<u32>,
) -> AppResult<Config> {
    if !["chat", "memory", "image", "vision"].contains(&purpose) {
        return Err(AppError::validation("purpose", "未知模型用途"));
    }
    if purpose == "chat" && id.is_none() {
        return Err(AppError::validation("providerId", "请选择聊天提供商"));
    }
    if max_output == Some(0) || capacity == Some(0) {
        return Err(AppError::validation(
            "maxOutputTokens",
            "高级容量必须为正整数",
        ));
    }
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if revision(&tx)? != expected {
        return Err(AppError::new(
            "CONFIG_CONFLICT",
            "提供商用途已变化，请重新读取后保存",
        ));
    }
    if let Some(id) = id {
        let id = parse_id(id)?;
        let has_key: Option<bool> = tx
            .query_row(
                "SELECT api_key_encrypted IS NOT NULL FROM providers WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let has_key = has_key.ok_or_else(AppError::not_found)?;
        if model.trim().is_empty() {
            return Err(AppError::validation("model", "请填写该用途的模型名"));
        }
        if purpose == "image" && enabled && !has_key {
            return Err(AppError::validation(
                "enabled",
                "生图启用前请先保存该连接的Key",
            ));
        }
        tx.execute("INSERT INTO provider_models(provider_id,purpose,model,max_output_tokens,context_window_override) VALUES(?1,?2,?3,COALESCE(?4,32768),?5) ON CONFLICT(provider_id,purpose) DO UPDATE SET model=excluded.model,max_output_tokens=CASE WHEN ?2='chat' THEN COALESCE(?4,provider_models.max_output_tokens) ELSE provider_models.max_output_tokens END,context_window_override=CASE WHEN ?2='chat' THEN ?5 ELSE provider_models.context_window_override END",params![id,purpose,model.trim(),max_output,capacity])?;
        tx.execute("INSERT INTO provider_roles(purpose,provider_id,enabled) VALUES(?1,?2,?3) ON CONFLICT(purpose) DO UPDATE SET provider_id=excluded.provider_id,enabled=excluded.enabled",params![purpose,id,if purpose=="image"{enabled}else{true}])?;
    } else {
        tx.execute("DELETE FROM provider_roles WHERE purpose=?1", [purpose])?;
    }
    if purpose == "image" {
        image_changed(&tx)?;
    }
    changed(&tx)?;
    let config = get(&tx)?;
    tx.commit()?;
    Ok(config)
}

/// 既有独立生图配置合入通用连接；保留密文，不按服务名自动合并不同凭据。
pub fn migrate_legacy(c: &Connection) -> AppResult<()> {
    let raw: Option<String> = c
        .query_row(
            "SELECT value FROM settings WHERE key='image_generation_config'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let Some(raw) = raw else { return Ok(()) };
    let config: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|_| AppError::new("STORAGE_ERROR", "旧生图配置无法读取，升级已停止"))?;
    let providers = config
        .get("providers")
        .and_then(|v| v.as_array())
        .ok_or_else(AppError::storage)?;
    for provider in providers {
        let old = provider
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(AppError::storage)?;
        let name = provider
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(AppError::storage)?;
        let base = provider
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(AppError::storage)?;
        let model = provider
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(AppError::storage)?;
        let key: Option<String> = c
            .query_row(
                "SELECT value FROM settings WHERE key=?1",
                [format!("image_generation_provider_{old}_api_key_encrypted")],
                |r| r.get(0),
            )
            .optional()?;
        c.execute(
            "INSERT INTO providers(name,base_url,api_key_encrypted) VALUES(?1,?2,?3)",
            params![name, base, key],
        )?;
        let id = c.last_insert_rowid();
        c.execute(
            "INSERT INTO provider_models(provider_id,purpose,model) VALUES(?1,'image',?2)",
            params![id, model],
        )?;
        if config.get("activeId").and_then(|v| v.as_str()) == Some(old) {
            let enabled = config
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                && key.is_some();
            c.execute("INSERT INTO provider_roles(purpose,provider_id,enabled) VALUES('image',?1,?2) ON CONFLICT(purpose) DO UPDATE SET provider_id=excluded.provider_id,enabled=excluded.enabled",params![id,enabled])?;
        }
    }
    c.execute("DELETE FROM settings WHERE key='image_generation_config' OR (key LIKE 'image_generation_provider_%' AND key LIKE '%_api_key_encrypted')",[])?;
    image_changed(c)?;
    changed(c)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_connection_has_independent_models_and_delete_protection() {
        let mut c = crate::db::test_database();
        c.execute("INSERT INTO providers(name,base_url,api_key_encrypted) VALUES('共享','https://example.invalid/v1','密文')",[]).unwrap();
        let id = c.last_insert_rowid();
        save(
            &mut c,
            "chat",
            Some(&id.to_string()),
            "chat-model",
            true,
            0,
            Some(32768),
            None,
        )
        .unwrap();
        save(
            &mut c,
            "memory",
            Some(&id.to_string()),
            "memory-model",
            true,
            1,
            None,
            None,
        )
        .unwrap();
        save(
            &mut c,
            "image",
            Some(&id.to_string()),
            "image-model",
            false,
            2,
            None,
            None,
        )
        .unwrap();
        assert_eq!(role(&c, "chat").unwrap().unwrap().model, "chat-model");
        assert_eq!(role(&c, "memory").unwrap().unwrap().model, "memory-model");
        assert_eq!(
            crate::image_generation::get(&c).unwrap().providers[0]
                .provider
                .model,
            "image-model"
        );
        assert!(providers::delete(&mut c, id).is_err());
        assert!(save(&mut c, "image", None, "", false, 0, None, None).is_err());
        assert_eq!(revision(&c).unwrap(), 3);
    }
    #[test]
    fn legacy_image_config_moves_key_without_exposing_or_losing_it() {
        let c = crate::db::test_database();
        c.execute("INSERT INTO settings VALUES('image_generation_config',?1)",[serde_json::json!({"version":1,"revision":3,"nextId":2,"enabled":true,"activeId":"1","providers":[{"id":"1","name":"图片连接","baseUrl":"https://images.invalid/v1","model":"img","protocol":"openai-compatible"}]}).to_string()]).unwrap();
        c.execute(
            "INSERT INTO settings VALUES('image_generation_provider_1_api_key_encrypted','原密文')",
            [],
        )
        .unwrap();
        migrate_legacy(&c).unwrap();
        let selected = role(&c, "image").unwrap().unwrap();
        assert!(selected.enabled);
        assert_eq!(
            c.query_row(
                "SELECT api_key_encrypted FROM providers WHERE id=?1",
                [parse_id(&selected.provider_id).unwrap()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "原密文"
        );
        assert_eq!(c.query_row("SELECT count(*) FROM settings WHERE key='image_generation_config' OR key LIKE 'image_generation_provider_%_api_key_encrypted'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
}
