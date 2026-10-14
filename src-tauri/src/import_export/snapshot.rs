//! 独立只读事务与图片句柄快照；记录由游标逐条读取，不收集聊天全集。
use super::*;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::{
    fs::{self, File},
    path::PathBuf,
};

pub(super) type AssetKey = (bool, String);
pub(super) type PinnedAssets = BTreeMap<AssetKey, Option<File>>;
pub(super) type AssetReferences = BTreeMap<AssetKey, Option<String>>;
const SELECTED: &str = "SELECT id FROM temp.backup_selection";

/// 一个来源人设可包含大量会话/记忆，使用子查询而不是将它们展开为SQL参数。
pub(super) struct Scope {
    pub request: ExportRequest,
}
impl Scope {
    pub fn includes(&self, table: &str) -> bool {
        match self.request.kind {
            PackageKind::Full => true,
            PackageKind::PluginData => [
                "game_plugins",
                "game_permissions",
                "game_plugin_settings",
                "game_plugin_data",
            ]
            .contains(&table),
            PackageKind::PersonaProfile => table == "personas",
            PackageKind::UserProfile if !self.request.include_sessions => table == "user_profiles",
            _ => ![
                "providers",
                "embedding_providers",
                "settings",
                "reminders",
                "reminder_occurrences",
                "user_state",
                "currency_ledger",
                "game_runs",
                "game_plugins",
                "game_permissions",
                "game_plugin_settings",
                "game_plugin_data",
            ]
            .contains(&table),
        }
    }
    fn sessions(&self) -> String {
        let owner = match self.request.kind {
            PackageKind::Session => "id",
            PackageKind::PersonaFull => "persona_id",
            PackageKind::UserProfile => "user_profile_id",
            _ => "id",
        };
        format!("SELECT id FROM sessions WHERE {owner} IN ({SELECTED})")
    }
    pub fn predicate(&self, table: &str) -> String {
        if self.request.kind == PackageKind::Full {
            return if table == "embedding_providers" {
                "is_builtin=0".into()
            } else {
                "1".into()
            };
        }
        if self.request.kind == PackageKind::PluginData {
            return format!("plugin_id IN ({SELECTED})");
        }
        let sessions = self.sessions();
        match table {
            "personas"
                if [PackageKind::PersonaFull, PackageKind::PersonaProfile]
                    .contains(&self.request.kind) =>
            {
                format!("id IN ({SELECTED})")
            }
            "personas" => {
                format!("id IN (SELECT persona_id FROM sessions WHERE id IN ({sessions}))")
            }
            "user_profiles" if self.request.kind == PackageKind::UserProfile => {
                format!("id IN ({SELECTED})")
            }
            "user_profiles" => {
                format!("id IN (SELECT user_profile_id FROM sessions WHERE id IN ({sessions}))")
            }
            "sessions" => format!("id IN ({sessions})"),
            "long_memory_sources" => format!(
                "long_memory_id IN (SELECT id FROM long_memories WHERE session_id IN ({sessions}))"
            ),
            _ => format!("session_id IN ({sessions})"),
        }
    }
    pub fn query(&self, table: &str, columns: &str) -> String {
        format!(
            "SELECT {columns} FROM {table} WHERE {} ORDER BY 1",
            self.predicate(table)
        )
    }
}

/// 临时选择表只存用户勾选的对象ID；每次只绑定一个参数，不受单条SQL参数总数影响。
pub(super) fn initialize(c: &Connection, request: &ExportRequest) -> AppResult<Scope> {
    request.validate()?;
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != SCHEMA_VERSION {
        return Err(AppError::new(
            "BACKUP_VERSION_UNSUPPORTED",
            "当前数据库版本不支持此备份协议",
        ));
    }
    c.execute_batch("CREATE TEMP TABLE IF NOT EXISTS backup_selection(id TEXT PRIMARY KEY); DELETE FROM temp.backup_selection;")?;
    {
        let mut insert = c.prepare("INSERT INTO temp.backup_selection VALUES(?1)")?;
        for id in &request.ids {
            insert.execute([id])?;
        }
    }
    if request.kind != PackageKind::Full {
        let (table, key) = match request.kind {
            PackageKind::Session => ("sessions", "id"),
            PackageKind::UserProfile => ("user_profiles", "id"),
            PackageKind::PluginData => ("game_plugins", "plugin_id"),
            _ => ("personas", "id"),
        };
        let found: i64 = c.query_row(
            &format!("SELECT count(*) FROM {table} WHERE {key} IN ({SELECTED})"),
            [],
            |r| r.get(0),
        )?;
        if found as usize != request.ids.len() {
            return Err(AppError::not_found());
        }
    }
    Ok(Scope {
        request: request.clone(),
    })
}

pub(super) fn validate(c: &Connection, scope: &Scope) -> AppResult<()> {
    // 真正的外键异常应明确失败；历史已删除饰品的账单引用另按协议置空。
    if c.prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some()
    {
        return Err(invalid());
    }
    for (child, foreign, parent) in [
        ("messages", "round_id", "chat_rounds"),
        ("short_memories", "round_id", "chat_rounds"),
        ("long_memories", "activated_by_round_id", "chat_rounds"),
        ("travel_journals", "travel_id", "travels"),
        ("accessories", "travel_id", "travels"),
        ("letters", "travel_id", "travels"),
        ("souvenirs", "travel_id", "travels"),
        ("letters", "reply_to_id", "letters"),
    ] {
        if !scope.includes(child) {
            continue;
        }
        let filter = scope.predicate(child);
        let mismatch=c.query_row(&format!("SELECT 1 FROM {child} child JOIN {parent} parent ON parent.id=child.{foreign} WHERE child.rowid IN (SELECT rowid FROM {child} WHERE {filter}) AND child.session_id<>parent.session_id LIMIT 1"),[],|_|Ok(())).optional()?;
        if mismatch.is_some() {
            return Err(invalid());
        }
    }
    if scope.includes("long_memories") {
        let filter = scope.predicate("long_memories");
        let bad=c.query_row(&format!("SELECT 1 FROM long_memories l LEFT JOIN long_memory_sources s ON s.long_memory_id=l.id LEFT JOIN short_memories m ON m.id=s.short_memory_id WHERE l.id IN (SELECT id FROM long_memories WHERE {filter}) GROUP BY l.id HAVING count(s.short_memory_id)<>l.source_count OR min(s.source_order)<>1 OR max(s.source_order)<>l.source_count OR sum(CASE WHEN m.session_id<>l.session_id OR m.integration_state<>'integrated' THEN 1 ELSE 0 END)>0 LIMIT 1"),[],|_|Ok(())).optional()?;
        if bad.is_some() {
            return Err(invalid());
        }
    }
    if scope.request.kind == PackageKind::Full {
        let current: Option<String> = c
            .query_row(
                "SELECT value FROM settings WHERE key='current_session_id'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = current {
            crate::error::parse_id(&id).map_err(|_| invalid())?;
            let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sessions s JOIN personas p ON p.id=s.persona_id WHERE s.id=?1 AND p.is_active=1)",[id],|r|r.get(0))?;
            if !valid {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// 在存储锁中取得句柄，随后改名/清理旧图片不会使已打开的快照失效。
pub(super) fn pin(runtime: &Runtime, c: &Connection, scope: &Scope) -> AppResult<PinnedAssets> {
    let mut files = PinnedAssets::new();
    for (table, column, background) in [
        ("personas", "chat_bg_file", true),
        ("accessories", "image_file", false),
    ] {
        if !scope.includes(table) {
            continue;
        }
        let mut stmt = c.prepare(&scope.query(table, column))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let Some(name) = row.get::<_, Option<String>>(0)? else {
                continue;
            };
            let key = (background, name.clone());
            if files.contains_key(&key) {
                continue;
            }
            if name.contains([':', '\\', '\0'])
                || (background && (name.contains('/') || name == ".."))
                || (!background
                    && !name.strip_prefix("assets/accessories/").is_some_and(|n| {
                        n.ends_with(".png") && !n.contains('/') && n.len() <= 180 && n.trim() == n
                    }))
            {
                return Err(AppError::new(
                    "BACKUP_ASSET_INVALID",
                    "备份图片引用不是受管路径",
                ));
            }
            let paths = runtime.paths.as_ref().map_err(Clone::clone)?;
            let folder = paths.data_root.join(if background {
                "chat_background"
            } else {
                "assets/accessories"
            });
            let path = if !folder.try_exists().map_err(|_| AppError::storage())? {
                None
            } else {
                let resolved = if background {
                    crate::persona::background::managed_file(runtime, &name)
                } else {
                    crate::accessories::images::managed_file(paths, &name)
                };
                match resolved {
                    Ok(path) => Some(path),
                    Err(e)
                        if ["BACKGROUND_MISSING", "ACCESSORY_IMAGE_UNAVAILABLE"]
                            .contains(&e.code) =>
                    {
                        None
                    }
                    Err(e) => return Err(e),
                }
            };
            let file = match path {
                Some(path) => match File::open(path) {
                    Ok(file) => Some(file),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(_) => return Err(AppError::storage()),
                },
                None => None,
            };
            files.insert(key, file);
        }
    }
    Ok(files)
}

struct SnapshotFile(PathBuf);
impl Drop for SnapshotFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
// 字段按声明顺序释放：先关闭SQLite连接，最后删除本次临时快照。
pub(super) struct Source {
    pub connection: Connection,
    pub scope: Scope,
    pub pinned: PinnedAssets,
    _file: SnapshotFile,
}
/// 主库未启用WAL，不能长时间持有其读事务；SQLite自身生成磁盘一致性副本。
/// 临时副本位于主库目录，不进入ZIP，所有凭据仍由导出白名单排除。
pub(super) fn independent(
    runtime: &Runtime,
    c: &Connection,
    request: &ExportRequest,
    unique_name: &std::ffi::OsStr,
) -> AppResult<Option<Source>> {
    let Some(path) = c.path().filter(|p| !p.is_empty()) else {
        #[cfg(test)]
        {
            return Ok(None);
        }
        #[cfg(not(test))]
        {
            return Err(AppError::storage());
        }
    };
    let root = std::path::Path::new(path)
        .parent()
        .ok_or_else(AppError::storage)?
        .canonicalize()
        .map_err(|_| AppError::storage())?;
    let mut name = unique_name.to_owned();
    name.push(".db");
    let path = root.join(name);
    // 先独占创建空文件再建立清理守卫，不能因名称冲突删除别人的文件。
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| AppError::storage())?,
    );
    let file = SnapshotFile(path);
    let target = file.0.to_str().ok_or_else(AppError::storage)?;
    c.execute("VACUUM main INTO ?1", [target])?;
    let source = Connection::open_with_flags(&file.0, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let scope = initialize(&source, request)?;
    let pinned = pin(runtime, &source, &scope)?;
    Ok(Some(Source {
        connection: source,
        scope,
        pinned,
        _file: file,
    }))
}

pub(super) fn warn(warnings: &mut Vec<Warning>, code: &str, table: &str, id: &str, message: &str) {
    warnings.push(Warning {
        code: code.into(),
        table: table.into(),
        record_id: id.into(),
        message: message.into(),
    });
}

/// 只投影一行；旧引用、密钥与插件授权处理沿用V1协议，原库不修改。
pub(super) fn project(
    c: &Connection,
    table: &str,
    mut row: Record,
    scope: &Scope,
    assets: &AssetReferences,
    warnings: &mut Vec<Warning>,
) -> AppResult<Option<Record>> {
    match table {
        "settings" => {
            let key = value(&row, "key")?;
            if !super::protocol::setting_allowed(key) {
                return Ok(None);
            }
            if ["image_generation_config", "web_search_config"].contains(&key) {
                let safe = super::protocol::public_config(
                    value(&row, "value")?,
                    key == "image_generation_config",
                )?;
                row.insert("value".into(), Value::String(safe));
            }
        }
        "game_plugins" => {
            row.insert("package_path".into(), Value::Null);
        }
        "game_permissions" if scope.request.kind == PackageKind::PluginData => {
            row.insert("decision".into(), Value::String("pending".into()));
            row.insert("decided_at".into(), Value::Null);
        }
        "currency_ledger" => {
            if ["accessory_craft", "accessory_sell"].contains(&value(&row, "reason")?) {
                if let Some(id) = row.get("ref_id").and_then(Value::as_str) {
                    let exists: bool = c.query_row(
                        "SELECT EXISTS(SELECT 1 FROM accessories WHERE id=?1)",
                        [id],
                        |r| r.get(0),
                    )?;
                    if !exists {
                        warn(
                            warnings,
                            "MISSING_ACCESSORY_REFERENCE",
                            table,
                            value(&row, "id")?,
                            "历史账单保留，已不存在的饰品引用在备份副本中置空。",
                        );
                        row.insert("ref_id".into(), Value::Null);
                    }
                }
            }
        }
        "session_festivals" => {
            let code = match row.get("festival_id").and_then(Value::as_str) {
                Some(id) => Some(
                    c.query_row("SELECT code FROM festival_catalog WHERE id=?1", [id], |r| {
                        r.get::<_, String>(0)
                    })
                    .optional()?
                    .ok_or_else(invalid)?,
                ),
                None => None,
            };
            row.remove("festival_id");
            row.insert(
                "festival_code".into(),
                code.map(Value::String).unwrap_or(Value::Null),
            );
        }
        _ => {}
    }
    if let Some((column, background)) = match table {
        "personas" => Some(("chat_bg_file", true)),
        "accessories" => Some(("image_file", false)),
        _ => None,
    } {
        if let Some(name) = row.get(column).and_then(Value::as_str) {
            let reference = assets
                .get(&(background, name.to_owned()))
                .ok_or_else(invalid)?;
            if reference.is_none() {
                warn(
                    warnings,
                    "MISSING_IMAGE",
                    table,
                    value(&row, "id")?,
                    if background {
                        "背景图片缺失，备份副本恢复默认背景，原库不修改。"
                    } else {
                        "饰品图片缺失，备份副本使用图标、Emoji、颜文字或符号组合兜底，原库不修改。"
                    },
                );
            }
            row.insert(
                column.into(),
                reference.clone().map(Value::String).unwrap_or(Value::Null),
            );
        }
    }
    Ok(Some(row))
}
