//! 普通聊天联网搜索：配置与密钥复用 settings，外部资料只读且不参与后台任务。
use crate::{
    crypto::{self, SecretUpdate},
    error::{AppError, AppResult},
    platform::contracts::SecretProtector,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

mod network;
pub(crate) use network::public_ip;
mod tools;
pub use tools::{definitions, SearchSession, RULES};
#[cfg(test)]
mod tests;

const CONFIG_KEY: &str = "web_search_config";
pub fn source_url(raw: &str) -> AppResult<String> {
    Ok(network::page_url(raw)?.to_string())
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Tavily,
    Bocha,
    Custom,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Entry {
    id: String,
    name: String,
    kind: Kind,
    base_url: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    version: u32,
    revision: u64,
    next_id: u64,
    enabled: bool,
    active_id: Option<String>,
    providers: Vec<Entry>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            next_id: 1,
            enabled: false,
            active_id: None,
            providers: vec![],
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub base_url: String,
    pub has_api_key: bool,
    pub is_active: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub enabled: bool,
    pub providers: Vec<Provider>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub name: String,
    pub kind: Kind,
    pub base_url: String,
    #[serde(default)]
    pub api_key: SecretUpdate,
}
/// 不实现 Serialize/Debug：解密密钥仅用于后端 HTTP 请求。
pub struct Snapshot {
    entry: Entry,
    revision: u64,
    key: String,
}
fn key_name(id: &str) -> String {
    format!("web_search_provider_{id}_api_key_encrypted")
}
fn secret(c: &Connection, id: &str) -> AppResult<Option<String>> {
    Ok(c.query_row(
        "SELECT value FROM settings WHERE key=?1",
        [key_name(id)],
        |r| r.get(0),
    )
    .optional()?)
}
fn load(c: &Connection) -> AppResult<Config> {
    let value: Option<String> = c
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [CONFIG_KEY],
            |r| r.get(0),
        )
        .optional()?;
    let Some(value) = value else {
        return Ok(Config::default());
    };
    let config: Config = serde_json::from_str(&value).map_err(|_| {
        AppError::new(
            "WEB_SEARCH_CONFIGURATION_ERROR",
            "搜索配置格式无效，请检查数据",
        )
    })?;
    let mut ids = std::collections::HashSet::new();
    if config.version != 1
        || config.next_id == 0
        || (config.enabled && config.active_id.is_none())
        || config.providers.len() > 50
        || config.providers.iter().any(|p| {
            p.id.parse::<u64>()
                .map_or(true, |id| id == 0 || id >= config.next_id)
                || !ids.insert(p.id.clone())
                || p.name.trim().is_empty()
                || crate::providers::validate_url(&p.base_url).is_err()
        })
        || config
            .active_id
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
    {
        return Err(AppError::new(
            "WEB_SEARCH_CONFIGURATION_ERROR",
            "搜索配置不完整，请检查数据",
        ));
    }
    Ok(config)
}
fn write(c: &Connection, config: &mut Config) -> AppResult<()> {
    config.revision = config
        .revision
        .checked_add(1)
        .ok_or_else(AppError::storage)?;
    let value = serde_json::to_string(config).map_err(|_| AppError::storage())?;
    c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![CONFIG_KEY,value])?;
    Ok(())
}
pub fn view(c: &Connection) -> AppResult<View> {
    let config = load(c)?;
    let providers = config
        .providers
        .iter()
        .map(|p| {
            Ok(Provider {
                id: p.id.clone(),
                name: p.name.clone(),
                kind: p.kind,
                base_url: p.base_url.clone(),
                has_api_key: secret(c, &p.id)?.is_some(),
                is_active: config.active_id.as_ref() == Some(&p.id),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(View {
        enabled: config.enabled,
        providers,
    })
}
pub fn save(
    c: &mut Connection,
    id: Option<String>,
    input: Input,
    secrets: &dyn SecretProtector,
) -> AppResult<()> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::validation("name", "名称须为 1–80 个字符"));
    }
    crate::providers::validate_url(input.base_url.trim())?;
    let base_url = input.base_url.trim().trim_end_matches('/').to_string();
    let prepared = crypto::prepare(input.api_key, secrets)?;
    let tx = c.transaction()?;
    let mut config = load(&tx)?;
    if let Some(id) = id {
        let entry = config
            .providers
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(AppError::not_found)?;
        if (entry.kind != input.kind || entry.base_url != base_url)
            && prepared.is_none()
            && secret(&tx, &id)?.is_some()
        {
            return Err(AppError::validation(
                "apiKey",
                "更换服务类型或地址时，请重新填写或清除 Key",
            ));
        }
        entry.name = name.into();
        entry.kind = input.kind;
        entry.base_url = base_url;
        if let Some(value) = prepared {
            save_secret(&tx, &id, value)?;
        }
        if config.active_id.as_ref() == Some(&id) && secret(&tx, &id)?.is_none() {
            config.enabled = false;
        }
    } else {
        if config.providers.len() >= 50 {
            return Err(AppError::validation("name", "最多保存 50 项搜索配置"));
        }
        let id = config.next_id.to_string();
        config.next_id = config
            .next_id
            .checked_add(1)
            .ok_or_else(AppError::storage)?;
        save_secret(&tx, &id, prepared.flatten())?;
        if config.active_id.is_none() {
            config.active_id = Some(id.clone());
        }
        config.providers.push(Entry {
            id,
            name: name.into(),
            kind: input.kind,
            base_url,
        });
    }
    write(&tx, &mut config)?;
    tx.commit()?;
    Ok(())
}
fn save_secret(c: &Connection, id: &str, value: Option<String>) -> AppResult<()> {
    if let Some(value) = value {
        c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key_name(id),value])?;
    } else {
        c.execute("DELETE FROM settings WHERE key=?1", [key_name(id)])?;
    }
    Ok(())
}
pub fn select(c: &mut Connection, id: &str) -> AppResult<()> {
    let tx = c.transaction()?;
    let mut config = load(&tx)?;
    if !config.providers.iter().any(|p| p.id == id) {
        return Err(AppError::not_found());
    }
    config.active_id = Some(id.into());
    if secret(&tx, id)?.is_none() {
        config.enabled = false;
    }
    write(&tx, &mut config)?;
    tx.commit()?;
    Ok(())
}
pub fn delete(c: &mut Connection, id: &str) -> AppResult<()> {
    let tx = c.transaction()?;
    let mut config = load(&tx)?;
    if config.active_id.as_deref() == Some(id) {
        return Err(AppError::new(
            "VALIDATION_ERROR",
            "当前搜索提供商不能删除，请先切换当前项",
        ));
    }
    let before = config.providers.len();
    config.providers.retain(|p| p.id != id);
    if before == config.providers.len() {
        return Err(AppError::not_found());
    }
    save_secret(&tx, id, None)?;
    write(&tx, &mut config)?;
    tx.commit()?;
    Ok(())
}
pub fn enable(c: &mut Connection, enabled: bool) -> AppResult<()> {
    let tx = c.transaction()?;
    let mut config = load(&tx)?;
    if enabled
        && config
            .active_id
            .as_ref()
            .map(|id| secret(&tx, id))
            .transpose()?
            .flatten()
            .is_none()
    {
        return Err(AppError::validation(
            "enabled",
            "请先新增并选择保存了 Key 的搜索服务",
        ));
    }
    config.enabled = enabled;
    write(&tx, &mut config)?;
    tx.commit()?;
    Ok(())
}
pub fn snapshot(
    c: &Connection,
    secrets: &dyn SecretProtector,
    id: Option<&str>,
) -> AppResult<Option<Snapshot>> {
    let config = load(c)?;
    if id.is_none() && !config.enabled {
        return Ok(None);
    }
    let id = id
        .or(config.active_id.as_deref())
        .ok_or_else(AppError::not_found)?;
    let entry = config
        .providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(AppError::not_found)?;
    let encrypted = secret(c, id)?
        .ok_or_else(|| AppError::new("WEB_SEARCH_KEY_MISSING", "搜索提供商未填写 Key"))?;
    let key = secrets.unprotect(&encrypted)?;
    Ok(Some(Snapshot {
        entry,
        revision: config.revision,
        key,
    }))
}
impl Snapshot {
    pub fn check(&self, c: &Connection) -> AppResult<()> {
        if load(c)?.revision != self.revision {
            Err(AppError::new(
                "WEB_SEARCH_CONFIGURATION_CHANGED",
                "搜索配置已经改变，请按当前配置重新搜索",
            ))
        } else {
            Ok(())
        }
    }
    pub async fn test(
        &self,
        client: &crate::ai::client::AiClient,
        cancel: &crate::state::Cancellation,
    ) -> AppResult<usize> {
        Ok(
            network::search(client, self, &tools::SearchArgs::test(), cancel)
                .await?
                .len(),
        )
    }
}
