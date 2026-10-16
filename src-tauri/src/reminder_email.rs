//! 邮件提醒与聊天提供商独立；网络发送不持有数据库锁。
mod outbox;
#[cfg(test)]
mod tests;
mod transport;

use crate::{
    crypto::SecretUpdate,
    error::{AppError, AppResult},
    platform::contracts::SecretProtector,
    runtime::Runtime,
};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::{atomic::AtomicBool, Mutex},
    time::{Duration, Instant},
};

pub(crate) use outbox::retry as outbox_retry;
pub(crate) use outbox::{enqueue, tick};
const CONFIG_KEY: &str = "reminder_email_config";
const SECRET_KEY: &str = "reminder_email_secret";

#[derive(Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectionConfig {
    pub email: String,
    pub host: String,
    pub port: u16,
    pub encryption: String,
    pub username: String,
    pub from: String,
    pub to: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Config {
    pub version: u32,
    pub connection: ConnectionConfig,
    pub revision: u64,
    pub enabled: bool,
    pub verified: bool,
    pub enabled_since: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            connection: ConnectionConfig {
                port: 465,
                encryption: "tls".into(),
                ..Default::default()
            },
            revision: 0,
            enabled: false,
            verified: false,
            enabled_since: None,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub connection: ConnectionConfig,
    #[serde(default)]
    pub secret: SecretUpdate,
    pub revision: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub connection: ConnectionConfig,
    pub revision: u64,
    pub enabled: bool,
    pub verified: bool,
    pub has_secret: bool,
    pub deliveries: Vec<outbox::DeliveryView>,
}
struct Challenge {
    fingerprint: Vec<u8>,
    digest: Vec<u8>,
    expires: Instant,
    attempts: u8,
    delivered: bool,
}
#[derive(Default)]
pub(crate) struct Service {
    challenge: Mutex<Option<Challenge>>,
    last_code: Mutex<Option<Instant>>,
    verification_gate: tokio::sync::Mutex<()>,
    pub(super) running: AtomicBool,
    pub(super) recovered: AtomicBool,
}
pub(crate) fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Millis, true)
}
pub(crate) fn load(c: &Connection) -> AppResult<Config> {
    let raw: Option<String> = c
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [CONFIG_KEY],
            |r| r.get(0),
        )
        .optional()?;
    let config: Config = raw
        .map(|raw| {
            serde_json::from_str(&raw)
                .map_err(|_| AppError::new("EMAIL_CONFIG_INVALID", "邮件配置无法读取，请重新配置"))
        })
        .transpose()?
        .unwrap_or_default();
    if config.version != 1 {
        return Err(AppError::new("EMAIL_CONFIG_INVALID", "邮件配置版本不兼容"));
    }
    Ok(config)
}
fn persist(c: &Connection, config: &Config) -> AppResult<()> {
    let raw = serde_json::to_string(config).map_err(|_| AppError::storage())?;
    c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![CONFIG_KEY,raw])?;
    Ok(())
}
fn encrypted_secret(c: &Connection) -> AppResult<Option<String>> {
    Ok(c.query_row(
        "SELECT value FROM settings WHERE key=?1",
        [SECRET_KEY],
        |r| r.get(0),
    )
    .optional()?)
}
fn fingerprint(c: &Connection, config: &Config) -> AppResult<Vec<u8>> {
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&config.connection).map_err(|_| AppError::storage())?);
    hash.update(config.revision.to_le_bytes());
    hash.update(
        encrypted_secret(c)?.ok_or_else(|| AppError::validation("secret", "请填写邮箱授权码"))?,
    );
    Ok(hash.finalize().to_vec())
}
fn validate(connection: &mut ConnectionConfig) -> AppResult<()> {
    for value in [
        &mut connection.email,
        &mut connection.host,
        &mut connection.username,
        &mut connection.from,
        &mut connection.to,
    ] {
        *value = value.trim().to_owned();
    }
    for (field, value) in [
        ("email", &connection.email),
        ("from", &connection.from),
        ("to", &connection.to),
    ] {
        value
            .parse::<lettre::Address>()
            .map_err(|_| AppError::validation(field, "请输入完整有效的邮箱地址"))?;
    }
    if connection.host.is_empty()
        || connection.host.contains(['/', ':', '@', '?', '#'])
        || connection.host.chars().any(char::is_whitespace)
        || url::Host::parse(&connection.host).is_err()
    {
        return Err(AppError::validation(
            "host",
            "请填写SMTP服务器主机名，例如 smtp.qq.com，不带协议或路径",
        ));
    }
    if connection.port == 0 || !["tls", "starttls"].contains(&connection.encryption.as_str()) {
        return Err(AppError::validation(
            "encryption",
            "SMTP仅支持TLS或STARTTLS加密连接，端口须有效",
        ));
    }
    if connection.username.is_empty() || connection.username.contains(['\r', '\n']) {
        return Err(AppError::validation("username", "请填写SMTP登录账号"));
    }
    Ok(())
}
pub fn view(c: &Connection) -> AppResult<View> {
    let config = load(c)?;
    Ok(View {
        connection: config.connection,
        revision: config.revision,
        enabled: config.enabled,
        verified: config.verified,
        has_secret: encrypted_secret(c)?.is_some(),
        deliveries: outbox::list(c)?,
    })
}
/// 保存配置和使旧任务失效为同一事务；留空的Keep不会覆盖原授权码。
pub fn save(
    c: &mut Connection,
    secrets: &dyn SecretProtector,
    mut input: Input,
) -> AppResult<View> {
    validate(&mut input.connection)?;
    let tx = c.transaction()?;
    let mut config = load(&tx)?;
    if config.revision != input.revision {
        return Err(AppError::new(
            "EMAIL_CONFIG_CHANGED",
            "邮件配置已被修改，请重新读取后再操作",
        ));
    }
    let secret = crate::crypto::prepare(input.secret, secrets)?;
    if config.connection != input.connection || secret.is_some() {
        config.connection = input.connection;
        config.revision = config
            .revision
            .checked_add(1)
            .ok_or_else(AppError::storage)?;
        config.enabled = false;
        config.verified = false;
        config.enabled_since = None;
        if let Some(value) = secret {
            tx.execute("DELETE FROM settings WHERE key=?1", [SECRET_KEY])?;
            if let Some(value) = value {
                tx.execute(
                    "INSERT INTO settings VALUES(?1,?2)",
                    params![SECRET_KEY, value],
                )?;
            }
        }
        outbox::cancel(&tx)?;
        persist(&tx, &config)?;
    }
    let result = view(&tx)?;
    tx.commit()?;
    Ok(result)
}
pub fn enable(
    c: &mut Connection,
    enabled: bool,
    revision: u64,
    now: DateTime<Utc>,
) -> AppResult<View> {
    let tx = c.transaction()?;
    let mut config = load(&tx)?;
    if revision != config.revision {
        return Err(AppError::new(
            "EMAIL_CONFIG_CHANGED",
            "邮件配置已更新，请重新读取",
        ));
    }
    if enabled && (!config.verified || encrypted_secret(&tx)?.is_none()) {
        return Err(AppError::new(
            "EMAIL_NOT_VERIFIED",
            "请先发送验证码并验证邮箱",
        ));
    }
    if config.enabled != enabled {
        config.enabled = enabled;
        config.enabled_since = enabled.then(|| stamp(now));
        // 开关也推进修订号，关闭再打开不能复活旧任务和旧验证码。
        config.revision = config
            .revision
            .checked_add(1)
            .ok_or_else(AppError::storage)?;
        outbox::cancel(&tx)?;
        persist(&tx, &config)?;
    }
    let result = view(&tx)?;
    tx.commit()?;
    Ok(result)
}
fn code_digest(code: &str, fingerprint: &[u8]) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(fingerprint);
    hash.update(code);
    hash.finalize().to_vec()
}
pub async fn send_code(runtime: &Runtime, input: Input) -> AppResult<View> {
    let _gate = runtime.reminder_email.verification_gate.lock().await;
    {
        let mut last = runtime
            .reminder_email
            .last_code
            .lock()
            .map_err(|_| AppError::storage())?;
        if last.is_some_and(|at| at.elapsed() < Duration::from_secs(60)) {
            return Err(AppError::new(
                "EMAIL_CODE_COOLDOWN",
                "请等待60秒后重新发送验证码",
            ));
        }
        *last = Some(Instant::now());
    }
    let code = format!(
        "{:06}",
        runtime
            .random
            .lock()
            .map_err(|_| AppError::storage())?
            .below(1_000_000)?
    );
    let (config, secret, expected) = runtime.read(|store, secrets| {
        save(&mut store.connection, secrets, input)?;
        let config = load(&store.connection)?;
        let expected = fingerprint(&store.connection, &config)?;
        let secret = secrets.unprotect(
            &encrypted_secret(&store.connection)?
                .ok_or_else(|| AppError::validation("secret", "请填写邮箱授权码"))?,
        )?;
        *runtime
            .reminder_email
            .challenge
            .lock()
            .map_err(|_| AppError::storage())? = Some(Challenge {
            fingerprint: expected.clone(),
            digest: code_digest(&code, &expected),
            expires: Instant::now() + Duration::from_secs(600),
            attempts: 0,
            delivered: false,
        });
        Ok((config, secret, expected))
    })?;
    let body=format!("你的颜喵邮箱验证码是：{code}\n\n10分钟内有效，用于开启邮件提醒。此验证码不是邮箱授权码，请勿向他人提供。\n如果不是你发起的操作，请忽略本邮件。");
    let result = transport::send(&config.connection, &secret, || {
        runtime
            .read(|store, _| {
                if fingerprint(&store.connection, &load(&store.connection)?)? != expected {
                    return Err(AppError::new(
                        "EMAIL_CONFIG_CHANGED",
                        "配置已更改，请重新发送验证码",
                    ));
                }
                Ok(("[颜喵] 邮箱验证码".to_owned(), body))
            })
            .map_err(|e| transport::Failure::permanent(&e.message))
    })
    .await;
    runtime.read(|store, _| {
        let current = load(&store.connection)?;
        if fingerprint(&store.connection, &current)? != expected {
            return Err(AppError::new(
                "EMAIL_CONFIG_CHANGED",
                "配置已更改，请向新配置重新发送验证码",
            ));
        }
        let mut challenge = runtime
            .reminder_email
            .challenge
            .lock()
            .map_err(|_| AppError::storage())?;
        if let Some(value) = challenge.as_mut() {
            value.delivered = result.is_ok();
            if result.is_err() {
                *challenge = None;
            }
        }
        result.map_err(|e| AppError::new("EMAIL_SEND_FAILED", e.message))?;
        view(&store.connection)
    })
}
pub fn verify(runtime: &Runtime, code: &str, revision: u64) -> AppResult<View> {
    if code.len() != 6 || !code.bytes().all(|v| v.is_ascii_digit()) {
        return Err(AppError::validation("code", "请输入邮件中的6位数字验证码"));
    }
    runtime.read(|store, _| {
        let tx = store.connection.transaction()?;
        let mut config = load(&tx)?;
        let mut guard = runtime
            .reminder_email
            .challenge
            .lock()
            .map_err(|_| AppError::storage())?;
        let challenge = guard.as_mut().ok_or_else(|| {
            AppError::new(
                "EMAIL_CODE_REQUIRED",
                "请先发送验证码；重启应用后需要重新发送",
            )
        })?;
        if revision != config.revision
            || challenge.fingerprint != fingerprint(&tx, &config)?
            || Instant::now() >= challenge.expires
            || !challenge.delivered
        {
            *guard = None;
            return Err(AppError::new(
                "EMAIL_CODE_EXPIRED",
                "验证码已失效，请重新发送",
            ));
        }
        if challenge.digest != code_digest(code, &challenge.fingerprint) {
            challenge.attempts += 1;
            if challenge.attempts >= 5 {
                *guard = None;
                return Err(AppError::new(
                    "EMAIL_CODE_EXPIRED",
                    "验证码连续输错5次，请重新发送",
                ));
            }
            return Err(AppError::new(
                "EMAIL_CODE_WRONG",
                format!("验证码不正确，还可尝试{}次", 5 - challenge.attempts),
            ));
        }
        config.verified = true;
        config.enabled = true;
        config.enabled_since = Some(stamp(runtime.clock.sample()?.utc));
        outbox::cancel(&tx)?;
        persist(&tx, &config)?;
        let result = view(&tx)?;
        tx.commit()?;
        *guard = None;
        Ok(result)
    })
}
/// 只导出公开连接字段；不携带授权码、绑定、开关、修订号或发送起点。
pub(crate) fn public_config(raw: &str) -> AppResult<String> {
    let mut value: Config = serde_json::from_str(raw)
        .map_err(|_| AppError::new("EMAIL_CONFIG_INVALID", "备份中的邮件配置无效"))?;
    if value.version != 1 {
        return Err(AppError::new("EMAIL_CONFIG_INVALID", "邮件配置版本不兼容"));
    }
    validate(&mut value.connection)?;
    let safe = Config {
        connection: value.connection,
        ..Default::default()
    };
    serde_json::to_string(&safe).map_err(|_| AppError::storage())
}
