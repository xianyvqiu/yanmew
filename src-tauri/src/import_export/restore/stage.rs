//! 备份逐条解码至临时SQLite；此模块不会修改正式数据库。
use super::*;
use rusqlite::{types::Value as SqlValue, Connection};
use serde::de::{DeserializeSeed, Error as DeError, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

pub struct Directory {
    root: PathBuf,
    pub path: PathBuf,
}
impl Directory {
    pub fn new(runtime: &Runtime) -> AppResult<Self> {
        let root = runtime
            .paths
            .as_ref()
            .map_err(Clone::clone)?
            .data_root
            .canonicalize()
            .map_err(|_| AppError::storage())?;
        let name = format!(
            "backup-import-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| AppError::storage())?
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        let path = root.join(name);
        fs::create_dir(&path).map_err(|_| AppError::storage())?;
        Ok(Self { root, path })
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        // 仅清理本次在受管根内独占创建的目录，不跟随越界链接。
        if self.path.parent() == Some(self.root.as_path())
            && self
                .path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("backup-import-"))
            && self
                .path
                .canonicalize()
                .is_ok_and(|p| p.starts_with(&self.root) && p != self.root)
        {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
pub struct Staged {
    pub connection: Connection,
    pub manifest: Manifest,
    pub directory: Directory,
}
fn fail() -> AppError {
    AppError::new(
        "BACKUP_INVALID",
        "备份文件损坏、字段不支持或数据关联不完整，原数据未修改",
    )
}
pub(super) fn sql_values(c: &Connection, table: &str, row: &Record) -> AppResult<Vec<SqlValue>> {
    let mut info = c.prepare(&format!("PRAGMA table_info({table})"))?;
    let types = info
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    row.iter()
        .map(|(key, value)| {
            if value.is_null() {
                if key == "id" {
                    return Err(fail());
                }
                return Ok(SqlValue::Null);
            }
            match types.get(key).map(String::as_str) {
                Some("INTEGER") if key == "id" || key.ends_with("_id") => Ok(SqlValue::Integer(
                    crate::error::parse_id(value.as_str().ok_or_else(fail)?).map_err(|_| fail())?,
                )),
                Some("INTEGER") => Ok(SqlValue::Integer(value.as_i64().ok_or_else(fail)?)),
                Some("TEXT") => Ok(SqlValue::Text(value.as_str().ok_or_else(fail)?.to_owned())),
                _ => Err(fail()),
            }
        })
        .collect()
}
fn insert(c: &Connection, table: &str, row: &Record) -> AppResult<()> {
    let columns = row.keys().map(String::as_str).collect::<Vec<_>>();
    let values = sql_values(c, table, row)?;
    let sql = format!(
        "INSERT INTO {table}({}) VALUES({})",
        columns.join(","),
        vec!["?"; columns.len()].join(",")
    );
    c.prepare_cached(&sql)?
        .execute(rusqlite::params_from_iter(values))
        .map_err(|_| fail())?;
    Ok(())
}
fn normalize(c: &Connection, table: &str, mut row: Record) -> AppResult<Record> {
    let columns = super::super::protocol::COLUMNS
        .iter()
        .find(|(name, _)| *name == table)
        .ok_or_else(fail)?
        .1;
    let mut expected = columns
        .split(',')
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if table == "session_festivals" {
        expected.remove("festival_id");
        expected.insert("festival_code".into());
    }
    if table == "accessories" && !row.contains_key("image_file") {
        row.insert("image_file".into(), Value::Null);
    }
    if expected != row.keys().cloned().collect() {
        return Err(fail());
    }
    for (key, val) in &row {
        if !val.is_null() && (key.ends_with("_at") || key == "sleep_until") {
            chrono::DateTime::parse_from_rfc3339(val.as_str().ok_or_else(fail)?)
                .map_err(|_| fail())?;
        }
    }
    if table == "settings" {
        let key = value(&row, "key")?;
        if !super::super::protocol::setting_allowed(key) {
            return Err(fail());
        }
        if ["image_generation_config", "web_search_config"].contains(&key) {
            let safe = super::super::protocol::public_config(
                value(&row, "value")?,
                key == "image_generation_config",
            )?;
            row.insert("value".into(), Value::String(safe));
        }
    }
    if table == "game_plugins" {
        if !row.get("package_path").is_some_and(Value::is_null) {
            return Err(fail());
        }
        for key in ["installed_version", "api_version"] {
            semver::Version::parse(value(&row, key)?).map_err(|_| fail())?;
        }
        let origins: Vec<String> =
            serde_json::from_str(value(&row, "network_origins_json")?).map_err(|_| fail())?;
        let mut distinct = BTreeSet::new();
        for origin in origins {
            if crate::game_plugins::manifest::origin(&origin)? != origin || !distinct.insert(origin)
            {
                return Err(fail());
            }
        }
        c.execute(
            "INSERT INTO import_plugin_flags(plugin_id,is_enabled) VALUES(?1,?2)",
            rusqlite::params![
                value(&row, "plugin_id")?,
                row.get("is_enabled")
                    .and_then(Value::as_i64)
                    .ok_or_else(fail)?
            ],
        )?;
        row.insert(
            "package_status".into(),
            Value::String("missing_package".into()),
        );
        row.insert("is_enabled".into(), Value::from(0));
    }
    if ["game_plugin_settings", "game_plugin_data"].contains(&table) {
        let key = value(
            &row,
            if table == "game_plugin_data" {
                "data_key"
            } else {
                "setting_key"
            },
        )?;
        crate::game_plugins::manifest::key(key)?;
        let raw = value(&row, "value_json")?;
        if raw.len() > crate::game_plugins::manifest::MAX_VALUE {
            return Err(fail());
        }
        serde_json::from_str::<Value>(raw).map_err(|_| fail())?;
    }
    if table == "game_permissions" {
        match value(&row, "permission")? {
            "wallet.modify" if value(&row, "permission_scope")?.is_empty() => {}
            "network.fetch" => {
                let origin = value(&row, "permission_scope")?;
                if crate::game_plugins::manifest::origin(origin)? != origin {
                    return Err(fail());
                }
            }
            _ => return Err(fail()),
        }
    }
    if table == "embedding_providers" && row.get("is_builtin") != Some(&Value::from(0)) {
        return Err(fail());
    }
    if table == "session_festivals" {
        let code = row.remove("festival_code").ok_or_else(fail)?;
        let id = if let Some(code) = code.as_str() {
            if code.trim().is_empty() {
                return Err(fail());
            }
            c.execute("INSERT OR IGNORE INTO festival_catalog(code,name,year,calendar_type,solar_month,solar_day,blessing) VALUES(?1,'备份校验',2026,'solar',1,1,'校验')",[code])?;
            Some(c.query_row(
                "SELECT id FROM festival_catalog WHERE code=?1",
                [code],
                |r| r.get::<_, i64>(0),
            )?)
        } else if code.is_null() {
            None
        } else {
            return Err(fail());
        };
        row.insert(
            "festival_id".into(),
            id.map(|n| Value::String(n.to_string()))
                .unwrap_or(Value::Null),
        );
    }
    Ok(row)
}
struct HashReader<R> {
    inner: R,
    hash: Sha256,
    size: u64,
}
impl<R: Read> Read for HashReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buffer)?;
        self.hash.update(&buffer[..n]);
        self.size += n as u64;
        Ok(n)
    }
}
struct Data<'a> {
    c: &'a Connection,
    scope: &'a super::super::snapshot::Scope,
}
impl<'de> DeserializeSeed<'de> for Data<'_> {
    type Value = BTreeMap<String, u64>;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for Data<'_> {
    type Value = BTreeMap<String, u64>;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("按表分组的备份")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut counts = BTreeMap::new();
        while let Some(table) = map.next_key::<String>()? {
            if counts.contains_key(&table)
                || !self.scope.includes(&table)
                || !super::super::protocol::COLUMNS
                    .iter()
                    .any(|(n, _)| *n == table)
            {
                return Err(A::Error::custom("不支持或重复的数据表"));
            }
            let count = map.next_value_seed(Rows {
                c: self.c,
                table: &table,
            })?;
            counts.insert(table, count);
        }
        Ok(counts)
    }
}
struct Rows<'a> {
    c: &'a Connection,
    table: &'a str,
}
impl<'de> DeserializeSeed<'de> for Rows<'_> {
    type Value = u64;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<u64, D::Error> {
        d.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Rows<'_> {
    type Value = u64;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("记录数组")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<u64, A::Error> {
        let mut count = 0;
        while let Some(row) = seq.next_element_seed(RecordSeed)? {
            let row = normalize(self.c, self.table, row).map_err(A::Error::custom)?;
            insert(self.c, self.table, &row).map_err(A::Error::custom)?;
            count += 1;
        }
        Ok(count)
    }
}
struct RecordSeed;
impl<'de> DeserializeSeed<'de> for RecordSeed {
    type Value = Record;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<Record, D::Error> {
        d.deserialize_map(self)
    }
}
impl<'de> Visitor<'de> for RecordSeed {
    type Value = Record;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("单条记录")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Record, A::Error> {
        let mut row = Record::new();
        while let Some(key) = map.next_key::<String>()? {
            if row.contains_key(&key) {
                return Err(A::Error::custom("重复字段"));
            }
            row.insert(key, map.next_value::<Value>()?);
        }
        Ok(row)
    }
}
fn valid_file(entry: &FileEntry) -> bool {
    if entry.sha256.len() != 64
        || !entry
            .sha256
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return false;
    }
    ["png", "jpg", "webp"]
        .iter()
        .any(|ext| entry.path == format!("files/{}.{}", entry.sha256, ext))
}
pub fn read(runtime: &Runtime, path: &Path) -> AppResult<Staged> {
    let directory = Directory::new(runtime)?;
    let mut zip =
        ZipArchive::new(File::open(path).map_err(|_| AppError::storage())?).map_err(|_| fail())?;
    let mut names = BTreeSet::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i).map_err(|_| fail())?;
        let name = file.name();
        if !names.insert(name.to_owned())
            || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            || (file.is_dir() && name != "files/")
        {
            return Err(fail());
        }
    }
    let manifest: Manifest =
        serde_json::from_reader(zip.by_name("manifest.json").map_err(|_| fail())?)
            .map_err(|_| fail())?;
    if manifest.format_version != FORMAT_VERSION || manifest.schema_version != SCHEMA_VERSION {
        return Err(AppError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            "备份格式或数据库版本不兼容，原数据未修改",
        ));
    }
    manifest.selection.validate()?;
    chrono::DateTime::parse_from_rfc3339(&manifest.exported_at).map_err(|_| fail())?;
    if manifest.selection.kind != manifest.package_type {
        return Err(fail());
    }
    let mut allowed = BTreeSet::from(["manifest.json".to_owned(), "data.json".to_owned()]);
    if names.contains("files/") {
        allowed.insert("files/".into());
    }
    for entry in &manifest.files {
        if !valid_file(entry) || !allowed.insert(entry.path.clone()) {
            return Err(fail());
        }
    }
    if names != allowed {
        return Err(fail());
    }
    let c = Connection::open(directory.path.join("source.db"))?;
    c.execute_batch(include_str!("../../../../db/schema.sql"))?;
    let triggers = c
        .prepare("SELECT name FROM sqlite_master WHERE type='trigger'")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for name in triggers {
        c.execute_batch(&format!("DROP TRIGGER \"{}\"", name.replace('"', "\"\"")))?;
    }
    c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA user_version=6; CREATE TABLE import_plugin_flags(plugin_id TEXT PRIMARY KEY,is_enabled INTEGER); CREATE TABLE import_files(path TEXT PRIMARY KEY,disk_name TEXT NOT NULL); BEGIN;")?;
    let scope = super::super::snapshot::Scope {
        request: manifest.selection.clone(),
    };
    let counts = {
        let data = zip.by_name("data.json").map_err(|_| fail())?;
        if data.size() != manifest.data_size {
            return Err(fail());
        }
        let mut reader = HashReader {
            inner: data,
            hash: Sha256::new(),
            size: 0,
        };
        let mut deserializer = serde_json::Deserializer::from_reader(&mut reader);
        let counts = Data {
            c: &c,
            scope: &scope,
        }
        .deserialize(&mut deserializer)
        .map_err(|_| fail())?;
        deserializer.end().map_err(|_| fail())?;
        if reader.size != manifest.data_size
            || format!("{:x}", reader.hash.finalize()) != manifest.data_sha256
        {
            return Err(fail());
        }
        counts
    };
    let expected = super::super::protocol::COLUMNS
        .iter()
        .filter(|(n, _)| scope.includes(n))
        .map(|(n, _)| n.to_string())
        .collect::<BTreeSet<_>>();
    if counts != manifest.record_counts
        || counts.keys().cloned().collect::<BTreeSet<_>>() != expected
    {
        return Err(fail());
    }
    c.execute_batch("COMMIT; PRAGMA foreign_keys=ON;")?;
    let scope = super::super::snapshot::initialize(&c, &manifest.selection)?;
    for (table, _) in super::super::protocol::COLUMNS {
        if scope.includes(table) {
            let count: i64 = c.query_row(
                &format!(
                    "SELECT count(*) FROM {table} WHERE NOT ({})",
                    scope.predicate(table)
                ),
                [],
                |r| r.get(0),
            )?;
            if count != 0 {
                return Err(fail());
            }
        }
    }
    super::super::snapshot::validate(&c, &scope)?;
    validate_business(&c, manifest.package_type)?;
    if manifest.package_type == PackageKind::Full
        && c.query_row("SELECT count(*) FROM user_state WHERE id=1", [], |r| {
            r.get::<_, i64>(0)
        })? != 1
    {
        return Err(fail());
    }
    crate::config::validate(&crate::config::get(&c)?)?;
    let mut used = BTreeSet::new();
    for (table, column) in [("personas", "chat_bg_file"), ("accessories", "image_file")] {
        let mut stmt = c.prepare(&format!(
            "SELECT DISTINCT {column} FROM {table} WHERE {column} IS NOT NULL"
        ))?;
        for name in stmt.query_map([], |r| r.get::<_, String>(0))? {
            let name = name?;
            if !manifest.files.iter().any(|e| e.path == name) {
                return Err(fail());
            }
            used.insert(name);
        }
    }
    if used != manifest.files.iter().map(|e| e.path.clone()).collect() {
        return Err(fail());
    }
    for (index, entry) in manifest.files.iter().enumerate() {
        let background: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM personas WHERE chat_bg_file=?1)",
            [&entry.path],
            |r| r.get(0),
        )?;
        let accessory: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM accessories WHERE image_file=?1)",
            [&entry.path],
            |r| r.get(0),
        )?;
        let max = if background {
            crate::persona::background::MAX_BYTES
        } else {
            crate::accessories::images::MAX_BYTES
        };
        let mut file = zip.by_name(&entry.path).map_err(|_| fail())?;
        if file.size() != entry.size {
            return Err(fail());
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(max as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| fail())?;
        if bytes.len() as u64 != entry.size || super::super::archive::hash(&bytes) != entry.sha256 {
            return Err(fail());
        }
        let filename = entry.path.strip_prefix("files/").ok_or_else(fail)?;
        if background {
            crate::persona::background::validate(filename, &bytes)?;
        }
        if accessory {
            crate::accessories::images::validate_png(&bytes)?;
        }
        let disk_name = format!("image-{index}");
        let output = directory.path.join(&disk_name);
        fs::write(output, &bytes).map_err(|_| AppError::storage())?;
        c.execute(
            "INSERT INTO import_files VALUES(?1,?2)",
            rusqlite::params![entry.path, disk_name],
        )?;
    }
    Ok(Staged {
        connection: c,
        manifest,
        directory,
    })
}

/// 复用数据库约束之外的业务校验；字数范围不限制手动保存的数据。
fn validate_business(c: &Connection, kind: PackageKind) -> AppResult<()> {
    for sql in [
        "SELECT 1 FROM sessions WHERE intimacy>999 LIMIT 1",
        "SELECT 1 FROM chat_rounds WHERE (status='interrupted' AND stop_reason IS NULL) OR (status<>'interrupted' AND stop_reason IS NOT NULL) OR (status='pending' AND completed_at IS NOT NULL) OR (status='completed' AND completed_at IS NULL) LIMIT 1",
        "SELECT 1 FROM chat_rounds r WHERE (SELECT count(*) FROM messages m WHERE m.round_id=r.id AND role='user')<>1 OR (r.status='completed' AND (SELECT count(*) FROM messages m WHERE m.round_id=r.id AND role='assistant')<>1) LIMIT 1",
        "SELECT 1 FROM short_memories m JOIN chat_rounds r ON r.id=m.round_id WHERE r.status<>'completed' OR length(trim(m.content))=0 LIMIT 1",
        "SELECT 1 FROM long_memories WHERE length(trim(title))=0 OR length(trim(content))=0 LIMIT 1",
        "SELECT 1 FROM short_memories m WHERE m.integration_state='integrated' AND NOT EXISTS(SELECT 1 FROM long_memory_sources s WHERE s.short_memory_id=m.id) LIMIT 1",
        "SELECT 1 FROM long_memory_sources s JOIN short_memories m ON m.id=s.short_memory_id JOIN long_memories l ON l.id=s.long_memory_id WHERE m.round_id>=l.activated_by_round_id LIMIT 1",
        "SELECT 1 FROM travel_journals j JOIN travels t ON t.id=j.travel_id WHERE t.journal_state<>'done' OR length(trim(j.title))=0 OR length(trim(j.content))=0 LIMIT 1",
        "SELECT 1 FROM letters child JOIN letters parent ON parent.id=child.reply_to_id WHERE child.travel_id IS NOT parent.travel_id LIMIT 1",
        "SELECT 1 FROM game_plugin_data d JOIN game_plugins p ON p.plugin_id=d.plugin_id WHERE d.storage_version<>p.storage_version LIMIT 1",
        "SELECT 1 FROM game_permissions a JOIN game_plugins p ON p.plugin_id=a.plugin_id WHERE a.plugin_version<>p.installed_version LIMIT 1",
    ] {
        if c.query_row(sql,[],|_|Ok(())).optional()?.is_some(){return Err(fail());}
    }
    if kind == PackageKind::Full
        && c.query_row("SELECT count(*) FROM personas WHERE is_active=1", [], |r| {
            r.get::<_, i64>(0)
        })? != 1
    {
        return Err(fail());
    }
    let mut stmt = c.prepare("SELECT plugin_id FROM game_plugins")?;
    for plugin in stmt.query_map([], |r| r.get::<_, String>(0))? {
        let snapshot = crate::game_plugins::snapshot(c, &plugin?)?;
        if snapshot.data.len() > 1024
            || snapshot.settings.len() > 64
            || snapshot
                .data
                .iter()
                .map(|(k, v)| k.len() + v.to_string().len())
                .sum::<usize>()
                > crate::game_plugins::manifest::MAX_STORAGE
        {
            return Err(fail());
        }
    }
    let mut stmt=c.prepare("SELECT destination,activities_json,started_at,planned_return_at,facts_state,gift_kind,gift_snapshot_json FROM travels")?;
    let mut cursor = stmt.query([])?;
    while let Some(r) = cursor.next()? {
        let start =
            chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>(2)?).map_err(|_| fail())?;
        let end =
            chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>(3)?).map_err(|_| fail())?;
        if end <= start {
            return Err(fail());
        }
        if r.get::<_, String>(4)? == "ready" {
            let facts = crate::travel::Facts {
                destination: r.get(0)?,
                activities: serde_json::from_str(&r.get::<_, String>(1)?).map_err(|_| fail())?,
            };
            facts.validate(end.signed_duration_since(start).num_minutes())?;
        }
        if let Some(snapshot) = r.get::<_, Option<String>>(6)? {
            crate::souvenirs::generation::validated_snapshot(&snapshot, &r.get::<_, String>(5)?)?;
        }
    }
    let mut stmt=c.prepare("SELECT base_url,model FROM providers UNION ALL SELECT base_url,model FROM embedding_providers WHERE kind='online'")?;
    for r in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (url, model) = r?;
        crate::providers::validate_url(&url)?;
        if model.trim().is_empty() {
            return Err(fail());
        }
    }
    crate::image_generation::get(c)?;
    crate::web_search::view(c)?;
    let mut reminders = c.prepare("SELECT id FROM reminders")?;
    for id in reminders.query_map([], |r| r.get::<_, i64>(0))? {
        let rule = crate::reminders::get_rule(c, id?)?;
        if let Some(schedule) = rule.schedule {
            schedule.validate()?;
        }
    }
    Ok(())
}
