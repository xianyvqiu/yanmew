//! 用户配置的唯一全局生图入口；密钥与聊天、记忆、Embedding完全隔离。
use crate::{
    crypto::{self, SecretUpdate},
    error::{parse_id, AppError, AppResult},
    platform::contracts::SecretProtector,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub(crate) mod openai_compatible;
#[cfg(test)]
mod tests;
const CONFIG_KEY: &str = "image_generation_config";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredConfig {
    version: u32,
    revision: u64,
    next_id: u64,
    enabled: bool,
    active_id: Option<String>,
    providers: Vec<Provider>,
}
impl Default for StoredConfig {
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
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicProvider {
    #[serde(flatten)]
    pub provider: Provider,
    pub has_api_key: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub revision: u64,
    pub next_id: String,
    pub enabled: bool,
    pub active_id: Option<String>,
    pub providers: Vec<PublicProvider>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub name: String,
    pub protocol: String,
    pub base_url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: SecretUpdate,
}
/// 不序列化、不Debug输出：仅在后端请求前短暂持有明文凭据。
pub(crate) struct Snapshot {
    pub provider: Provider,
    pub revision: u64,
    pub key: String,
}
fn invalid() -> AppError {
    AppError::new(
        "IMAGE_GENERATION_CONFIGURATION_ERROR",
        "生图配置无效，请检查保存的数据",
    )
}
fn key_name(id: &str) -> String {
    format!("image_generation_provider_{id}_api_key_encrypted")
}
fn secret(c: &Connection, id: &str) -> AppResult<Option<String>> {
    let legacy:Option<String>=c.query_row("SELECT value FROM settings WHERE key=?1",[key_name(id)],|r|r.get(0)).optional()?;
    if legacy.is_some(){return Ok(legacy);}
    Ok(c.query_row(
        "SELECT api_key_encrypted FROM provider_effective WHERE id=?1",
        [parse_id(id)?],
        |r| r.get(0),
    )
    .optional()?.flatten())
}
fn address(raw: &str) -> AppResult<String> {
    let url = url::Url::parse(raw.trim())
        .map_err(|_| AppError::validation("baseUrl", "请输入有效的HTTPS接口根地址"))?;
    if raw.len() > 2048
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::validation(
            "baseUrl",
            "生图根地址必须为HTTPS，不能含账号、查询参数或片段",
        ));
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}
fn validate(provider: &Provider) -> AppResult<()> {
    parse_id(&provider.id)?;
    if provider.name.trim().is_empty() || provider.name.chars().count() > 80 {
        return Err(AppError::validation("name", "名称须为1–80个字符"));
    }
    if provider.protocol != "openai-compatible" {
        return Err(AppError::validation(
            "protocol",
            "当前仅支持OpenAI-compatible生图协议",
        ));
    }
    if provider.model.trim().is_empty() || provider.model.chars().count() > 200 {
        return Err(AppError::validation(
            "model",
            "请输入生图模型名称，最多200个字符",
        ));
    }
    address(&provider.base_url)?;
    Ok(())
}
fn load(c: &Connection) -> AppResult<StoredConfig> {
    let value: Option<String> = c
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [CONFIG_KEY],
            |r| r.get(0),
        )
        .optional()?;
    let Some(value) = value else {
        let roles=crate::provider_roles::get(c)?;
        let image=roles.roles.iter().find(|r|r.purpose=="image");
        let mut providers=Vec::new();let mut next_id=1;
        for p in crate::providers::list(c)? {
            next_id=next_id.max(parse_id(&p.id)? as u64+1);
            let model=image.filter(|r|r.provider_id==p.id).map(|r|r.model.clone()).unwrap_or(p.model);
            providers.push(Provider{id:p.id,name:p.name,protocol:"openai-compatible".into(),base_url:p.base_url.unwrap_or_else(||"https://api.openai.com/v1".into()),model});
        }
        let active_id=image.map(|r|r.provider_id.clone());
        let has_key=active_id.as_deref().map(|id|secret(c,id)).transpose()?.flatten().is_some();
        return Ok(StoredConfig{version:1,revision:crate::provider_roles::image_revision(c)?,next_id,enabled:image.is_some_and(|r|r.enabled)&&has_key,active_id,providers});
    };
    let config: StoredConfig = serde_json::from_str(&value).map_err(|_| invalid())?;
    let mut ids = std::collections::HashSet::new();
    if config.version != 1
        || config.next_id == 0
        || config.next_id > i64::MAX as u64
        || config.providers.len() > 50
        || config.providers.iter().any(|p| {
            validate(p).is_err()
                || p.id.parse::<u64>().map_or(true, |id| id >= config.next_id)
                || !ids.insert(p.id.clone())
        })
        || config
            .active_id
            .as_ref()
            .is_some_and(|id| !ids.contains(id))
        || (config.enabled && config.active_id.is_none())
    {
        return Err(invalid());
    }
    Ok(config)
}
fn require_revision(config: &StoredConfig, expected: u64) -> AppResult<()> {
    if config.revision != expected {
        return Err(AppError::new(
            "CONFIG_CONFLICT",
            "生图配置已在其他窗口改变，请重新读取后再保存",
        ));
    }
    Ok(())
}
fn write(c: &Connection, config: &mut StoredConfig) -> AppResult<()> {
    config.revision = config
        .revision
        .checked_add(1)
        .ok_or_else(AppError::storage)?;
    c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![CONFIG_KEY,serde_json::to_string(config).map_err(|_|AppError::storage())?])?;
    Ok(())
}
pub fn get(c: &Connection) -> AppResult<Config> {
    let config = load(c)?;
    let providers = config
        .providers
        .iter()
        .map(|provider| {
            Ok(PublicProvider {
                provider: provider.clone(),
                has_api_key: secret(c, &provider.id)?.is_some(),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Config {
        version: config.version,
        revision: config.revision,
        next_id: config.next_id.to_string(),
        enabled: config.enabled,
        active_id: config.active_id,
        providers,
    })
}
pub fn save(
    c: &mut Connection,
    id: Option<String>,
    expected: u64,
    input: Input,
    protector: &dyn SecretProtector,
) -> AppResult<Config> {
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut config = load(&tx)?;
    require_revision(&config, expected)?;
    let id = match id {
        Some(id) => {
            parse_id(&id)?;
            if !config.providers.iter().any(|p| p.id == id) {
                return Err(AppError::not_found());
            }
            id
        }
        None => {
            if config.providers.len() >= 50 {
                return Err(AppError::validation("providers", "生图提供商最多50项"));
            }
            let id = config.next_id.to_string();
            config.next_id = config
                .next_id
                .checked_add(1)
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or_else(AppError::storage)?;
            id
        }
    };
    let provider = Provider {
        id: id.clone(),
        name: input.name.trim().into(),
        protocol: input.protocol,
        base_url: address(&input.base_url)?,
        model: input.model.trim().into(),
    };
    validate(&provider)?;
    if config
        .providers
        .iter()
        .any(|p| p.id == id && p.base_url != provider.base_url)
        && matches!(&input.api_key, SecretUpdate::Keep)
    {
        return Err(AppError::validation(
            "apiKey",
            "更换接口根地址时必须重新填写或明确清除Key",
        ));
    }
    let update = crypto::prepare(input.api_key, protector)?;
    if let Some(key) = update {
        tx.execute("DELETE FROM settings WHERE key=?1", [key_name(&id)])?;
        if let Some(key) = key {
            tx.execute(
                "INSERT INTO settings(key,value) VALUES(?1,?2)",
                params![key_name(&id), key],
            )?;
        }
    }
    if let Some(old) = config.providers.iter_mut().find(|p| p.id == id) {
        *old = provider
    } else {
        config.providers.push(provider)
    }
    if config.active_id.as_ref() == Some(&id) && secret(&tx, &id)?.is_none() {
        config.enabled = false;
    }
    write(&tx, &mut config)?;
    let result = get(&tx)?;
    tx.commit()?;
    Ok(result)
}
pub fn select(c: &mut Connection, id: Option<String>, expected: u64) -> AppResult<Config> {
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut config = load(&tx)?;
    require_revision(&config, expected)?;
    if let Some(id) = &id {
        parse_id(id)?;
        if !config.providers.iter().any(|p| &p.id == id) {
            return Err(AppError::not_found());
        }
    }
    let has_key = match &id {
        Some(id) => secret(&tx, id)?.is_some(),
        None => false,
    };
    if !has_key {
        config.enabled = false;
    }
    config.active_id = id;
    write(&tx, &mut config)?;
    let result = get(&tx)?;
    tx.commit()?;
    Ok(result)
}
pub fn set_enabled(c: &mut Connection, enabled: bool, expected: u64) -> AppResult<Config> {
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut config = load(&tx)?;
    require_revision(&config, expected)?;
    let has_key = match &config.active_id {
        Some(id) => secret(&tx, id)?.is_some(),
        None => false,
    };
    if enabled && !has_key {
        return Err(AppError::validation(
            "enabled",
            "先选择配置完整且已保存Key的生图提供商，才能开启",
        ));
    }
    config.enabled = enabled;
    write(&tx, &mut config)?;
    let result = get(&tx)?;
    tx.commit()?;
    Ok(result)
}
pub fn delete(c: &mut Connection, id: &str, expected: u64) -> AppResult<Config> {
    parse_id(id)?;
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut config = load(&tx)?;
    require_revision(&config, expected)?;
    if config.active_id.as_deref() == Some(id) {
        return Err(AppError::new(
            "ACTIVE_PROVIDER",
            "当前生图提供商不能删除，请先切换或清除选择",
        ));
    }
    let position = config
        .providers
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(AppError::not_found)?;
    config.providers.remove(position);
    tx.execute("DELETE FROM settings WHERE key=?1", [key_name(id)])?;
    write(&tx, &mut config)?;
    let result = get(&tx)?;
    tx.commit()?;
    Ok(result)
}
pub(crate) fn revision(c: &Connection) -> AppResult<u64> {
    Ok(load(c)?.revision)
}
pub(crate) fn snapshot(
    c: &Connection,
    protector: &dyn SecretProtector,
) -> AppResult<Option<Snapshot>> {
    let config = load(c)?;
    if !config.enabled {
        return Ok(None);
    }
    let id = config.active_id.as_deref().ok_or_else(invalid)?;
    let provider = config
        .providers
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(invalid)?;
    let key = secret(c, id)?
        .ok_or_else(|| AppError::new("IMAGE_GENERATION_NO_KEY", "当前生图提供商尚未保存Key"))?;
    let key = protector.unprotect(&key)?;
    if key.trim().is_empty() {
        return Err(invalid());
    }
    Ok(Some(Snapshot {
        provider,
        revision: config.revision,
        key,
    }))
}
