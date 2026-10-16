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
    ("standby_minutes", 10),
    ("idle_event_minutes", 45),
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

/// 旧版分别配置无聊/入睡；统一后优先保留无聊值，只有缺失时使用入睡值。
/// 初始化在业务事务内调用，导入仅在隔离暂存库调用，不留下两套可独立修改的阈值。
pub(crate) fn normalize_legacy_idle_settings(conn: &Connection) -> AppResult<()> {
    conn.execute_batch("INSERT OR IGNORE INTO settings(key,value)
        SELECT 'idle_event_minutes', COALESCE(
            (SELECT value FROM settings WHERE key='bored_minutes'),
            (SELECT value FROM settings WHERE key='sleep_idle_minutes'))
        WHERE EXISTS(SELECT 1 FROM settings WHERE key IN ('bored_minutes','sleep_idle_minutes'));
        DELETE FROM settings WHERE key IN ('bored_minutes','sleep_idle_minutes');")?;
    Ok(())
}

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
        if key.starts_with("intimacy_threshold_") && i64::from(number) > crate::pet::MAX_INTIMACY {
            return Err(AppError::validation(key, "亲密度档位不能超过上限 999"));
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

/// 内部窗口物理坐标不经过全局设置接口；残缺或无效坐标保留原值并回退默认定位。
pub fn get_window_position(c: &Connection) -> AppResult<Option<(i32, i32)>> {
    let (x, y): (Option<String>, Option<String>) = c.query_row(
        "SELECT (SELECT value FROM settings WHERE key='window_pos_x'),
                (SELECT value FROM settings WHERE key='window_pos_y')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let coordinate = |value: Option<String>| {
        let value = value?;
        let number = value.parse::<i32>().ok()?;
        (number.to_string() == value).then_some(number)
    };
    Ok(coordinate(x).zip(coordinate(y)))
}

pub fn save_window_position(c: &mut Connection, position: (i32, i32)) -> AppResult<()> {
    let tx = c.transaction()?;
    for (key, value) in [("window_pos_x", position.0), ("window_pos_y", position.1)] {
        tx.execute(
            "INSERT INTO settings(key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value.to_string()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_position_missing_partial_and_invalid_values_fall_back_without_deletion() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        assert_eq!(get_window_position(&c).unwrap(), None);
        for key in ["window_pos_x", "window_pos_y"] {
            c.execute("DELETE FROM settings WHERE key IN ('window_pos_x','window_pos_y')", []).unwrap();
            c.execute("INSERT INTO settings(key,value) VALUES (?1,'-20')", [key]).unwrap();
            assert_eq!(get_window_position(&c).unwrap(), None);
        }
        for key in ["window_pos_x", "window_pos_y"] {
            for invalid in ["", " ", "01", "-01", "-0", "+1", " 1", "1 ", "1.5", "abc", "2147483648", "-2147483649"] {
                save_window_position(&mut c, (-20, 30)).unwrap();
                c.execute("UPDATE settings SET value=?1 WHERE key=?2", params![invalid, key]).unwrap();
                assert_eq!(get_window_position(&c).unwrap(), None, "{key}={invalid:?}");
                let retained: String = c.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get(0)).unwrap();
                assert_eq!(retained, invalid);
                get(&c).unwrap();
            }
        }
    }

    #[test]
    fn window_position_signed_boundaries_persist_after_disk_reopen() {
        let temp_root = std::env::temp_dir().canonicalize().unwrap();
        let file_name = format!(
            "yanmew-window-position-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        );
        let path = temp_root.join(&file_name);
        std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
        {
            let mut c = Connection::open(&path).unwrap();
            crate::db::initialize(&mut c).unwrap();
            for position in [(0, 0), (-1920, -1080), (i32::MIN, i32::MAX), (i32::MAX, i32::MIN)] {
                save_window_position(&mut c, position).unwrap();
                assert_eq!(get_window_position(&c).unwrap(), Some(position));
            }
        }
        {
            let c = Connection::open(&path).unwrap();
            assert_eq!(get_window_position(&c).unwrap(), Some((i32::MAX, i32::MIN)));
        }
        let verified_path = path.canonicalize().unwrap();
        assert_eq!(verified_path.parent(), Some(temp_root.as_path()));
        assert_eq!(verified_path.file_name(), Some(std::ffi::OsStr::new(&file_name)));
        std::fs::remove_file(verified_path).unwrap();
    }

    #[test]
    fn window_position_second_write_failure_rolls_back_both_coordinates() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        c.execute_batch("CREATE TRIGGER reject_window_y_insert BEFORE INSERT ON settings WHEN NEW.key='window_pos_y' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert_eq!(save_window_position(&mut c, (-20, 30)).unwrap_err().code, "STORAGE_ERROR");
        let count: i64 = c.query_row("SELECT COUNT(*) FROM settings WHERE key IN ('window_pos_x','window_pos_y')", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0);
        c.execute_batch("DROP TRIGGER reject_window_y_insert;").unwrap();
        save_window_position(&mut c, (-20, 30)).unwrap();
        c.execute_batch("CREATE TRIGGER reject_window_y_update BEFORE UPDATE ON settings WHEN NEW.key='window_pos_y' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert_eq!(save_window_position(&mut c, (10, -40)).unwrap_err().code, "STORAGE_ERROR");
        assert_eq!(get_window_position(&c).unwrap(), Some((-20, 30)));
    }

    #[test]
    fn window_position_is_not_exposed_or_writable_through_global_settings() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        save_window_position(&mut c, (-20, 30)).unwrap();
        let original = get(&c).unwrap();
        for key in ["window_pos_x", "window_pos_y"] {
            assert!(!defaults().contains_key(key));
            assert!(!original.contains_key(key));
            let changes = [(key.into(), "100".into()), ("theme_mode".into(), "dark".into())].into_iter().collect();
            assert_eq!(save(&mut c, changes).unwrap_err().code, "VALIDATION_ERROR");
            assert_eq!(get(&c).unwrap(), original);
            assert_eq!(get_window_position(&c).unwrap(), Some((-20, 30)));
        }
    }

    #[test]
    fn window_position_database_read_failure_remains_storage_error() {
        let c = Connection::open_in_memory().unwrap();
        assert_eq!(get_window_position(&c).unwrap_err().code, "STORAGE_ERROR");
    }

    #[test]
    fn memory_settings_roundtrip_and_invalid_ranges_are_atomic() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut connection).unwrap();
        let original = get(&connection).unwrap();
        for (key, expected) in [
            ("short_memory_min_chars", "12"), ("short_memory_max_chars", "100"),
            ("long_memory_min_chars", "150"), ("long_memory_max_chars", "250"),
            ("travel_journal_min_chars", "250"), ("travel_journal_max_chars", "350"),
            ("memory_merge_count", "20"), ("recent_message_count", "30"),
            ("recent_long_memory_count", "30"), ("tool_call_rounds", "3"),
            ("notebook_max_chars", "4000"),
        ] { assert_eq!(original[key], expected); }
        let changed = [
            ("short_memory_min_chars", "20"), ("short_memory_max_chars", "120"),
            ("long_memory_min_chars", "160"), ("long_memory_max_chars", "300"),
            ("travel_journal_min_chars", "280"), ("travel_journal_max_chars", "400"),
            ("memory_merge_count", "10"), ("recent_message_count", "40"),
            ("recent_long_memory_count", "15"), ("tool_call_rounds", "4"),
            ("notebook_max_chars", "5000"),
        ].into_iter().map(|(key, value)| (key.into(), value.into())).collect();
        let saved = save(&mut connection, changed).unwrap();
        assert_eq!(get(&connection).unwrap(), saved);
        assert_eq!(saved["hungry_minutes"], original["hungry_minutes"]);
        for (key, value) in [
            ("short_memory_min_chars", "121"), ("long_memory_min_chars", "301"),
            ("travel_journal_min_chars", "401"), ("memory_merge_count", "0"),
            ("recent_message_count", "01"), ("recent_long_memory_count", "1.5"),
            ("notebook_max_chars", "2147483648"), ("unknown_key", "1"),
        ] {
            let invalid = [(key.into(), value.into()), ("tool_call_rounds".into(), "5".into())].into_iter().collect();
            assert!(save(&mut connection, invalid).is_err(), "{key}");
            assert_eq!(get(&connection).unwrap(), saved);
        }
    }
    #[test]
    fn pet_timing_settings_roundtrip_and_invalid_groups_are_atomic() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut connection).unwrap();
        validate(&defaults()).unwrap();
        let changes = [
            ("hungry_minutes", "90"), ("standby_minutes", "5"),
            ("idle_event_minutes", "40"), ("sleep_min_minutes", "20"),
            ("sleep_max_minutes", "50"), ("travel_min_online_hours", "3"),
            ("travel_max_online_hours", "5"), ("intimacy_threshold_1", "15"),
            ("intimacy_threshold_2", "60"), ("intimacy_threshold_3", "200"),
            ("intimacy_threshold_4", "999"), ("accessory_cost_1", "0"),
            ("accessory_cost_2", "25"), ("accessory_cost_3", "80"),
            ("accessory_cost_4", "150"),
        ].into_iter().map(|(key, value)| (key.into(), value.into())).collect();
        let saved = save(&mut connection, changes).unwrap();
        assert_eq!(get(&connection).unwrap(), saved);
        assert_eq!(saved["theme_mode"], "system");
        assert_eq!(saved["memory_merge_count"], "20");
        for (key, value) in [
            ("sleep_min_minutes", "51"), ("travel_min_online_hours", "6"),
            ("intimacy_threshold_2", "15"), ("intimacy_threshold_4", "1000"),
            ("accessory_cost_1", "-1"), ("hungry_minutes", "0"),
            ("standby_minutes", "1.5"), ("idle_event_minutes", "2147483648"),
            ("bored_minutes", "10"), ("sleep_idle_minutes", "10"),
        ] {
            let invalid = [(key.into(), value.into()), ("accessory_cost_2".into(), "100".into())].into_iter().collect();
            assert!(save(&mut connection, invalid).is_err(), "{key}");
            assert_eq!(get(&connection).unwrap(), saved);
        }
    }
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
