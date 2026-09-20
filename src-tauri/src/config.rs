use crate::error::{AppError, AppResult};
use crate::{
    crypto::{self, SecretUpdate},
    platform::contracts::SecretProtector,
};
use rusqlite::OptionalExtension;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherSettings {
    pub api_host: Option<String>,
    pub has_api_key: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WeatherInput {
    pub api_host: Option<String>,
    #[serde(default)]
    pub api_key: SecretUpdate,
}
pub fn get_weather(c: &Connection) -> AppResult<WeatherSettings> {
    Ok(WeatherSettings {
        api_host: c
            .query_row(
                "SELECT value FROM settings WHERE key='weather_qweather_api_host'",
                [],
                |r| r.get(0),
            )
            .optional()?,
        has_api_key: c.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key='weather_qweather_api_key_encrypted')",
            [],
            |r| r.get(0),
        )?,
    })
}
pub fn save_weather(
    c: &mut Connection,
    input: WeatherInput,
    protector: &dyn SecretProtector,
) -> AppResult<WeatherSettings> {
    let host = input.api_host.filter(|h| !h.trim().is_empty());
    if let Some(host) = &host {
        let parsed = url::Url::parse(host)
            .map_err(|_| AppError::validation("apiHost", "请输入和风天气专属 HTTPS 主机地址"))?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || parsed.path() != "/"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(AppError::validation(
                "apiHost",
                "天气地址必须是 HTTPS 主机，不能包含接口路径、账号或查询参数",
            ));
        }
    }
    let key = crypto::prepare(input.api_key, protector)?;
    let tx = c.transaction()?;
    tx.execute(
        "DELETE FROM settings WHERE key='weather_qweather_api_host'",
        [],
    )?;
    if let Some(host) = host {
        tx.execute(
            "INSERT INTO settings VALUES('weather_qweather_api_host',?1)",
            [host],
        )?;
    }
    if let Some(key) = key {
        tx.execute(
            "DELETE FROM settings WHERE key='weather_qweather_api_key_encrypted'",
            [],
        )?;
        if let Some(key) = key {
            tx.execute(
                "INSERT INTO settings VALUES('weather_qweather_api_key_encrypted',?1)",
                [key],
            )?;
        }
    }
    let result = get_weather(&tx)?;
    tx.commit()?;
    Ok(result)
}

/// 全局数值设置的唯一默认值入口；内部上下文和密钥不经过此接口。
pub const DEFAULTS: &[(&str, i64)] = &[
    ("hungry_minutes", 120),
    ("bored_minutes", 45),
    ("sleep_idle_minutes", 45),
    ("sleep_min_minutes", 60),
    ("sleep_max_minutes", 120),
    ("short_memory_min_chars", 12),
    ("short_memory_max_chars", 100),
    ("long_memory_min_chars", 150),
    ("long_memory_max_chars", 250),
    ("memory_merge_count", 20),
    ("recent_message_count", 30),
    ("recent_long_memory_count", 30),
    ("travel_journal_min_chars", 250),
    ("travel_journal_max_chars", 350),
    ("tool_call_rounds", 3),
    ("notebook_max_chars", 4000),
    ("intimacy_threshold_1", 10),
    ("intimacy_threshold_2", 50),
    ("intimacy_threshold_3", 150),
    ("intimacy_threshold_4", 300),
    ("accessory_cost_1", 20),
    ("accessory_cost_2", 50),
    ("accessory_cost_3", 120),
    ("accessory_cost_4", 300),
    ("sign_in_min_coins", 1),
    ("sign_in_max_coins", 10),
    ("travel_min_online_hours", 2),
    ("travel_max_online_hours", 4),
    ("travel_min_duration_minutes", 60),
    ("travel_max_duration_minutes", 180),
    ("travel_accessory_chance_percent", 10),
    ("letter_reply_min_minutes", 10),
    ("letter_reply_max_minutes", 30),
];
pub type Settings = BTreeMap<String, String>;

pub fn defaults() -> Settings {
    let mut values: Settings = DEFAULTS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    values.insert("theme_mode".into(), "system".into());
    values
}

pub fn validate(values: &Settings) -> AppResult<()> {
    for (key, value) in values {
        if key == "theme_mode" {
            if !["system", "light", "dark"].contains(&value.as_str()) {
                return Err(AppError::validation(
                    key,
                    "主题只能是 system、light 或 dark",
                ));
            }
            continue;
        }
        if !DEFAULTS.iter().any(|(k, _)| k == key) {
            return Err(AppError::validation(key, "不允许通过此接口修改该设置"));
        }
        let number = value
            .parse::<i32>()
            .map_err(|_| AppError::validation(key, "请输入有效整数"))?;
        let zero_allowed = key.starts_with("accessory_cost_")
            || key.starts_with("sign_in_")
            || key == "travel_accessory_chance_percent";
        if number < if zero_allowed { 0 } else { 1 } || number.to_string() != *value {
            return Err(AppError::validation(key, "数值范围或格式无效"));
        }
        if key == "travel_accessory_chance_percent" && number > 100 {
            return Err(AppError::validation(key, "概率必须在 0–100 之间"));
        }
    }
    let number = |k: &str| -> AppResult<i64> {
        values
            .get(k)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| AppError::validation(k, "缺少设置值"))
    };
    for (min, max) in [
        ("sleep_min_minutes", "sleep_max_minutes"),
        ("short_memory_min_chars", "short_memory_max_chars"),
        ("long_memory_min_chars", "long_memory_max_chars"),
        ("travel_journal_min_chars", "travel_journal_max_chars"),
        ("sign_in_min_coins", "sign_in_max_coins"),
        ("travel_min_online_hours", "travel_max_online_hours"),
        ("travel_min_duration_minutes", "travel_max_duration_minutes"),
        ("letter_reply_min_minutes", "letter_reply_max_minutes"),
    ] {
        if number(min)? > number(max)? {
            return Err(AppError::validation(min, "最小值不能大于最大值"));
        }
    }
    for n in 1..4 {
        let key = format!("intimacy_threshold_{n}");
        if number(&key)? >= number(&format!("intimacy_threshold_{}", n + 1))? {
            return Err(AppError::validation(&key, "亲密度解锁档位必须逐档递增"));
        }
    }
    Ok(())
}

pub fn get(conn: &Connection) -> AppResult<Settings> {
    let mut result = defaults();
    let mut stmt = conn.prepare("SELECT key,value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (key, value) = row?;
        if result.contains_key(&key) {
            result.insert(key, value);
        }
    }
    validate(&result)?;
    Ok(result)
}

pub fn save(conn: &mut Connection, changes: Settings) -> AppResult<Settings> {
    let tx = conn.transaction()?;
    let mut merged = get(&tx)?;
    for (key, value) in &changes {
        merged.insert(key.clone(), value.clone());
    }
    validate(&merged)?;
    for (key, value) in changes {
        tx.execute("INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value])?;
    }
    tx.commit()?;
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Secret;
    impl SecretProtector for Secret {
        fn protect(&self, plain: &str) -> AppResult<String> {
            Ok(format!("cipher:{plain}"))
        }
        fn unprotect(&self, _: &str) -> AppResult<String> {
            unreachable!()
        }
    }
    #[test]
    fn weather_secrets_are_separate_and_updates_are_explicit() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        let host = Some("https://example.com".into());
        let result = save_weather(
            &mut c,
            WeatherInput {
                api_host: host.clone(),
                api_key: SecretUpdate::Set {
                    value: "private".into(),
                },
            },
            &Secret,
        )
        .unwrap();
        assert!(result.has_api_key);
        assert!(!serde_json::to_string(&result).unwrap().contains("private"));
        assert!(!serde_json::to_string(&get(&c).unwrap())
            .unwrap()
            .contains("cipher"));
        assert!(save_weather(
            &mut c,
            WeatherInput {
                api_host: Some("http://example.com".into()),
                api_key: SecretUpdate::Clear
            },
            &Secret
        )
        .is_err());
        assert!(get_weather(&c).unwrap().has_api_key);
        assert!(
            save_weather(
                &mut c,
                WeatherInput {
                    api_host: host.clone(),
                    api_key: SecretUpdate::Keep
                },
                &Secret
            )
            .unwrap()
            .has_api_key
        );
        assert!(
            !save_weather(
                &mut c,
                WeatherInput {
                    api_host: host,
                    api_key: SecretUpdate::Clear
                },
                &Secret
            )
            .unwrap()
            .has_api_key
        );
    }
}
