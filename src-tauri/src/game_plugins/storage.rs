use super::*;
use std::collections::BTreeMap;
pub fn snapshot(c: &Connection, id: &str) -> AppResult<PrivateSnapshot> {
    let p = get(c, id)?;
    let wrong: i64 = c.query_row(
        "SELECT COUNT(*) FROM game_plugin_data WHERE plugin_id=?1 AND storage_version<>?2",
        params![id, p.storage_version],
        |r| r.get(0),
    )?;
    if wrong != 0 {
        return Err(AppError::new(
            "PLUGIN_STORAGE_VERSION",
            "插件存档版本与登记不一致，请保留数据并检查导入来源",
        ));
    }
    fn read(c: &Connection, sql: &str, id: &str) -> AppResult<BTreeMap<String, Value>> {
        let mut s = c.prepare(sql)?;
        let rows = s.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = BTreeMap::new();
        for row in rows {
            let (k, v) = row?;
            out.insert(
                k,
                serde_json::from_str(&v).map_err(|_| AppError::storage())?,
            );
        }
        Ok(out)
    }
    Ok(PrivateSnapshot {
        storage_version: p.storage_version,
        settings: read(
            c,
            "SELECT setting_key,value_json FROM game_plugin_settings WHERE plugin_id=?1",
            id,
        )?,
        data: read(
            c,
            "SELECT data_key,value_json FROM game_plugin_data WHERE plugin_id=?1",
            id,
        )?,
    })
}
pub fn validate_snapshot(m: &Manifest, s: &PrivateSnapshot) -> AppResult<()> {
    if s.storage_version != m.storage_version || s.data.len() > 1024 || s.settings.len() > 64 {
        return Err(AppError::validation("snapshot", "存档版本或键数量无效"));
    }
    let mut total = 0usize;
    for (k, v) in &s.data {
        manifest::key(k)?;
        let json = encode(v)?;
        total += k.len() + json.len();
    }
    if total > manifest::MAX_STORAGE {
        return Err(AppError::new("PLUGIN_STORAGE_LIMIT", "插件存档超过 10 MiB"));
    }
    for (k, v) in &s.settings {
        setting(m, k)?.validate_value(v)?;
    }
    Ok(())
}
fn setting<'a>(m: &'a Manifest, k: &str) -> AppResult<&'a manifest::Setting> {
    m.settings_schema
        .iter()
        .find(|s| s.key == k)
        .ok_or_else(|| AppError::validation("key", "插件未声明此设置键"))
}
fn encode(v: &Value) -> AppResult<String> {
    let text = serde_json::to_string(v)
        .map_err(|_| AppError::validation("value", "存档必须是合法 JSON"))?;
    if text.len() > manifest::MAX_VALUE {
        return Err(AppError::new(
            "PLUGIN_STORAGE_LIMIT",
            "单个存档值超过 1 MiB",
        ));
    }
    Ok(text)
}
pub fn merged_settings(c: &Connection, m: &Manifest) -> AppResult<BTreeMap<String, Value>> {
    let s = snapshot(c, &m.id)?;
    validate_snapshot(m, &s)?;
    let mut merged: BTreeMap<_, _> = m
        .settings_schema
        .iter()
        .map(|s| (s.key.clone(), s.default.clone()))
        .collect();
    merged.extend(s.settings);
    Ok(merged)
}
pub fn save_setting(
    c: &Connection,
    m: &Manifest,
    k: &str,
    value: &Value,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let p = get(c, &m.id)?;
    if p.installed_version != m.version {
        return Err(denied());
    }
    setting(m, k)?.validate_value(value)?;
    c.execute("INSERT INTO game_plugin_settings VALUES(?1,?2,?3,?4) ON CONFLICT(plugin_id,setting_key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",params![m.id,k,encode(value)?,stamp(now)])?;
    Ok(())
}
pub fn data_get(c: &Connection, id: &str, version: &str, k: &str) -> AppResult<Option<Value>> {
    authorize(c, id, version)?;
    manifest::key(k)?;
    let raw: Option<String> = c
        .query_row(
            "SELECT value_json FROM game_plugin_data WHERE plugin_id=?1 AND data_key=?2",
            params![id, k],
            |r| r.get(0),
        )
        .optional()?;
    raw.map(|v| serde_json::from_str(&v).map_err(|_| AppError::storage()))
        .transpose()
}
pub fn data_set(
    c: &mut Connection,
    id: &str,
    version: &str,
    k: &str,
    v: &Value,
    now: DateTime<Utc>,
) -> AppResult<()> {
    manifest::key(k)?;
    let json = encode(v)?;
    let tx = c.transaction()?;
    let p = authorize(&tx, id, version)?;
    let(size,count):(i64,i64)=tx.query_row("SELECT COALESCE(SUM(length(CAST(data_key AS BLOB))+length(CAST(value_json AS BLOB))),0),COUNT(*) FROM game_plugin_data WHERE plugin_id=?1 AND data_key<>?2",params![id,k],|r|Ok((r.get(0)?,r.get(1)?)))?;
    if size as usize + k.len() + json.len() > manifest::MAX_STORAGE || count >= 1024 {
        return Err(AppError::new(
            "PLUGIN_STORAGE_LIMIT",
            "插件存档超过容量或键数量限制",
        ));
    }
    tx.execute("INSERT INTO game_plugin_data VALUES(?1,?2,?3,?4,?5) ON CONFLICT(plugin_id,data_key) DO UPDATE SET value_json=excluded.value_json,storage_version=excluded.storage_version,updated_at=excluded.updated_at",params![id,k,json,p.storage_version,stamp(now)])?;
    tx.commit()?;
    Ok(())
}
pub fn data_delete(c: &Connection, id: &str, version: &str, k: &str) -> AppResult<()> {
    authorize(c, id, version)?;
    manifest::key(k)?;
    c.execute(
        "DELETE FROM game_plugin_data WHERE plugin_id=?1 AND data_key=?2",
        params![id, k],
    )?;
    Ok(())
}
pub(crate) fn replace_snapshot(
    c: &Connection,
    m: &Manifest,
    s: &PrivateSnapshot,
    now: DateTime<Utc>,
) -> AppResult<()> {
    validate_snapshot(m, s)?;
    c.execute(
        "DELETE FROM game_plugin_settings WHERE plugin_id=?1",
        [&m.id],
    )?;
    c.execute("DELETE FROM game_plugin_data WHERE plugin_id=?1", [&m.id])?;
    for (k, v) in &s.settings {
        c.execute(
            "INSERT INTO game_plugin_settings VALUES(?1,?2,?3,?4)",
            params![m.id, k, encode(v)?, stamp(now)],
        )?;
    }
    for (k, v) in &s.data {
        c.execute(
            "INSERT INTO game_plugin_data VALUES(?1,?2,?3,?4,?5)",
            params![m.id, k, encode(v)?, m.storage_version, stamp(now)],
        )?;
    }
    Ok(())
}
