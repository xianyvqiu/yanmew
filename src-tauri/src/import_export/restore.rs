//! 追加导入协调：单份磁盘预览、明确人设关联、提交后消耗预览。
use super::*;
use rusqlite::{Connection, OptionalExtension};
use std::sync::{atomic::AtomicU64, Mutex};
mod apply;
mod stage;
#[cfg(test)]
mod tests;
static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Default)]
pub struct State {
    gate: Mutex<()>,
    pending: Mutex<Option<Pending>>,
}
struct Pending {
    stage: stage::Staged,
    preview: Preview,
    plugins: BTreeMap<String, crate::game_plugins::Plugin>,
    scopes: BTreeMap<String, Vec<(String, String)>>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    pub name: String,
    pub stamp: String,
    pub different: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub source_id: String,
    pub name: String,
    pub is_builtin: bool,
    pub candidates: Vec<Candidate>,
    pub suggested_target_id: Option<String>,
    pub requires_choice: bool,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub preview_id: String,
    pub kind: PackageKind,
    pub exported_at: String,
    pub counts: BTreeMap<String, u64>,
    pub warnings: Vec<String>,
    pub personas: Vec<Node>,
    pub profiles: Vec<Node>,
    pub plugins: Vec<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub source_id: String,
    pub target_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Confirm {
    pub preview_id: String,
    pub personas: Vec<Binding>,
    pub profiles: Vec<Binding>,
    pub confirm_global_restore: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub sessions: u64,
    pub personas_created: u64,
    pub personas_reused: u64,
    pub warnings: Vec<String>,
}
fn invalid_import() -> AppError {
    AppError::new(
        "BACKUP_INVALID",
        "备份数据不完整或不符合当前导入协议，原数据未修改",
    )
}
fn stale() -> AppError {
    AppError::new(
        "BACKUP_PREVIEW_CHANGED",
        "导入预览或关联目标已变化，请重新读取预览后确认",
    )
}
fn row(c: &Connection, table: &str, id: i64) -> AppResult<Record> {
    let columns = super::protocol::COLUMNS
        .iter()
        .find(|(n, _)| *n == table)
        .ok_or_else(invalid_import)?
        .1;
    let names = columns.split(',').collect::<Vec<_>>();
    let mut stmt = c.prepare(&format!("SELECT {columns} FROM {table} WHERE id=?1"))?;
    let mut rows = stmt.query([id])?;
    let r = rows.next()?.ok_or_else(stale)?;
    super::protocol::record(r, &names)
}
fn stamp(mut row: Record) -> AppResult<String> {
    row.remove("is_active");
    Ok(super::archive::hash(
        &serde_json::to_vec(&row).map_err(|_| AppError::storage())?,
    ))
}
fn nodes(
    source: &Connection,
    target: &Connection,
    table: &str,
    updates: bool,
) -> AppResult<Vec<Node>> {
    let builtin = if table == "personas" {
        "is_builtin"
    } else {
        "0"
    };
    let mut stmt = source.prepare(&format!(
        "SELECT id,name,{builtin} FROM {table} ORDER BY id"
    ))?;
    let mut rows = stmt.query([])?;
    let mut result = Vec::new();
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        let name: String = r.get(1)?;
        let is_builtin: bool = r.get(2)?;
        let ids = if !updates {
            vec![]
        } else if is_builtin {
            let mut q = target.prepare("SELECT id FROM personas WHERE is_builtin=1")?;
            let result = q
                .query_map([], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            result
        } else {
            let mut q =
                target.prepare(&format!("SELECT id FROM {table} WHERE name=?1 ORDER BY id"))?;
            let result = q
                .query_map([&name], |r| r.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            result
        };
        if is_builtin && ids.len() != 1 {
            return Err(AppError::new(
                "BACKUP_DEFAULT_PERSONA_MISSING",
                "默认人设不完整，请重新启动应用后再导入",
            ));
        }
        let source_row = row(source, table, id)?;
        let mut candidates = Vec::new();
        for target_id in ids {
            let target_row = row(target, table, target_id)?;
            let different = source_row
                .iter()
                .filter(|(k, _)| {
                    !["id", "is_active", "is_builtin", "created_at", "updated_at"]
                        .contains(&k.as_str())
                })
                .any(|(k, v)| target_row.get(k) != Some(v));
            candidates.push(Candidate {
                id: target_id.to_string(),
                name: value(&target_row, "name")?.to_owned(),
                stamp: stamp(target_row)?,
                different,
            });
        }
        let suggested_target_id = if table == "personas" && candidates.len() == 1 {
            Some(candidates[0].id.clone())
        } else {
            None
        };
        let requires_choice = table == "personas" && candidates.len() > 1;
        result.push(Node {
            source_id: id.to_string(),
            name,
            is_builtin,
            candidates,
            suggested_target_id,
            requires_choice,
        });
    }
    Ok(result)
}
fn preview(runtime: &Runtime, source: &stage::Staged, id: String) -> AppResult<Preview> {
    runtime.read(|store,_|{
        let c=&source.connection;
        let mut warnings=source.manifest.warnings.iter().map(|w|w.message.clone()).collect::<Vec<_>>();
        if source.manifest.package_type==PackageKind::Full {warnings.push("全局喵币、账单、设置、提醒、提供商和插件使用数据将按备份恢复；主程序密钥需要重新填写，生图开关关闭。原有会话保留。".into());}
        if source.manifest.package_type==PackageKind::PluginData {warnings.push("仅恢复指定插件设置和存档；不改变钱包、账单或运行记录，钱包与联网权限改为待确认。".into());}
        let mut q=c.prepare("SELECT plugin_id FROM game_plugins ORDER BY plugin_id")?;
        let plugins=q.query_map([],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Preview {preview_id:id,kind:source.manifest.package_type,exported_at:source.manifest.exported_at.clone(),counts:source.manifest.record_counts.clone(),warnings,
            personas:nodes(c,&store.connection,"personas",true)?,profiles:nodes(c,&store.connection,"user_profiles",source.manifest.package_type==PackageKind::UserProfile)?,plugins})
    })
}
pub fn prepare(
    runtime: &Runtime,
    transfer: &dyn crate::platform::contracts::FileTransferPort,
) -> AppResult<Option<Preview>> {
    let _gate = runtime
        .backups
        .gate
        .lock()
        .map_err(|_| AppError::storage())?;
    let Some(path) = transfer.open_backup()? else {
        return Ok(None);
    };
    let source = stage::read(runtime, &path)?;
    let id = format!(
        "import-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let result = preview(runtime, &source, id)?;
    *runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())? = Some(Pending {
        stage: source,
        preview: result.clone(),
        plugins: BTreeMap::new(),
        scopes: BTreeMap::new(),
    });
    Ok(Some(result))
}
pub fn refresh(runtime: &Runtime, id: &str) -> AppResult<Preview> {
    let _gate = runtime
        .backups
        .gate
        .lock()
        .map_err(|_| AppError::storage())?;
    let mut pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    let p = pending
        .as_mut()
        .filter(|p| p.preview.preview_id == id)
        .ok_or_else(stale)?;
    p.preview = preview(runtime, &p.stage, id.to_owned())?;
    p.plugins.clear();
    p.scopes.clear();
    Ok(p.preview.clone())
}
pub fn cancel(runtime: &Runtime, id: &str) -> AppResult<()> {
    let _gate = runtime
        .backups
        .gate
        .lock()
        .map_err(|_| AppError::storage())?;
    let mut pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    if pending.as_ref().is_some_and(|p| p.preview.preview_id == id) {
        *pending = None;
    }
    Ok(())
}
pub fn plugins_to_close(runtime: &Runtime, id: &str) -> AppResult<Vec<String>> {
    let pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    let p = pending
        .as_ref()
        .filter(|p| p.preview.preview_id == id)
        .ok_or_else(stale)?;
    if p.preview.kind == PackageKind::Full {
        runtime.read(|store, _| {
            let mut q = store
                .connection
                .prepare("SELECT plugin_id FROM game_plugins")?;
            let result = q
                .query_map([], |r| r.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(result)
        })
    } else if p.preview.kind == PackageKind::PluginData {
        Ok(p.preview.plugins.clone())
    } else {
        Ok(Vec::new())
    }
}
pub fn confirm(runtime: &Runtime, input: Confirm) -> AppResult<Imported> {
    let _gate = runtime
        .backups
        .gate
        .lock()
        .map_err(|_| AppError::storage())?;
    let mut pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    let p = pending
        .as_mut()
        .filter(|p| p.preview.preview_id == input.preview_id)
        .ok_or_else(stale)?;
    if p.preview.kind == PackageKind::Full && !input.confirm_global_restore {
        return Err(AppError::validation(
            "confirmGlobalRestore",
            "请确认全局数据恢复范围",
        ));
    }
    let result = runtime.read(|store, _| apply::commit(runtime, store, p, &input));
    // 只检查本次导入发布的资源，不扫描其他操作尚未提交的图片或生图暂存区。
    cleanup_published(runtime, &p.stage.connection);
    if result.is_ok() {
        *pending = None;
    }
    result
}

fn cleanup_published(runtime: &Runtime, source: &Connection) {
    let Ok(mut statement) = source.prepare("SELECT role,filename FROM import_published") else {
        return;
    };
    let Ok(mut cursor) = statement.query([]) else { return; };
    while let Ok(Some(row)) = cursor.next() {
        let (Ok(role), Ok(filename)) = (row.get::<_, String>(0), row.get::<_, String>(1)) else {
            continue;
        };
        match role.as_str() {
            "background" => crate::persona::background::cleanup_one(runtime, &filename),
            "accessory" => crate::accessories::images::cleanup_one(runtime, &filename, &[]),
            "chat" => {let _=runtime.read(|store,_|crate::chat_images::cleanup(&store.connection,runtime.paths.as_ref().map_err(Clone::clone)?));},
            _ => {}
        }
    }
}

/// 在关闭游戏之前检查确认字段；最终事务仍会再检查一次。
pub fn check(runtime: &Runtime, input: &Confirm) -> AppResult<()> {
    let pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    let p = pending
        .as_ref()
        .filter(|p| p.preview.preview_id == input.preview_id)
        .ok_or_else(stale)?;
    runtime.read(|s, _| apply::check(&s.connection, p, input))
}
/// 每次只取一个插件的受限快照；迁移沿用隔离宿主，不能持有数据库锁等待。
pub fn plugin_job(
    runtime: &Runtime,
    id: &str,
    index: usize,
) -> AppResult<
    Option<(
        crate::game_plugins::Plugin,
        crate::game_plugins::package::Package,
        crate::game_plugins::PrivateSnapshot,
    )>,
> {
    let pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    let p = pending
        .as_ref()
        .filter(|p| p.preview.preview_id == id)
        .ok_or_else(stale)?;
    let Some(plugin_id) = p.preview.plugins.get(index) else {
        return Ok(None);
    };
    runtime.read(|store, _| {
        let target = match crate::game_plugins::get(&store.connection, plugin_id) {
            Ok(p) => p,
            Err(e) if e.code == "NOT_FOUND" => return Ok(None),
            Err(e) => return Err(e),
        };
        let root =
            crate::game_plugins::package::root(runtime.paths.as_ref().map_err(Clone::clone)?);
        let package = match crate::game_plugins::package::load(&root, &target) {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };
        let source = crate::game_plugins::snapshot(&p.stage.connection, plugin_id)?;
        if source.storage_version > package.manifest.storage_version {
            return Ok(None);
        }
        Ok(Some((target, package, source)))
    })
}
pub fn accept_plugin(
    runtime: &Runtime,
    id: &str,
    target: crate::game_plugins::Plugin,
    package: &crate::game_plugins::package::Package,
    snapshot: crate::game_plugins::PrivateSnapshot,
) -> AppResult<()> {
    crate::game_plugins::validate_snapshot(&package.manifest, &snapshot)?;
    let mut pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    let p = pending
        .as_mut()
        .filter(|p| p.preview.preview_id == id)
        .ok_or_else(stale)?;
    let tx = p.stage.connection.transaction()?;
    tx.execute(
        "UPDATE game_plugins SET storage_version=?1 WHERE plugin_id=?2",
        rusqlite::params![snapshot.storage_version, target.plugin_id],
    )?;
    crate::game_plugins::replace_snapshot(
        &tx,
        &package.manifest,
        &snapshot,
        runtime.clock.sample()?.utc,
    )?;
    tx.commit()?;
    p.scopes
        .insert(target.plugin_id.clone(), package.manifest.scopes());
    p.plugins.insert(target.plugin_id.clone(), target);
    Ok(())
}

pub fn current_preview(runtime: &Runtime, id: &str) -> AppResult<Preview> {
    let pending = runtime
        .backups
        .pending
        .lock()
        .map_err(|_| AppError::storage())?;
    Ok(pending
        .as_ref()
        .filter(|p| p.preview.preview_id == id)
        .ok_or_else(stale)?
        .preview
        .clone())
}
