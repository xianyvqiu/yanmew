//! 全局插件使用数据；所有身份由宿主绑定，插件不能自行指定命名空间。
use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub mod manifest;
pub mod package;
pub mod runs;
mod storage;
pub use manifest::Manifest;
pub use storage::*;
#[cfg(test)]
mod tests;

pub(crate) fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub(crate) fn denied() -> AppError {
    AppError::new(
        "PLUGIN_PERMISSION_DENIED",
        "插件未获授权、已停用或版本已经改变",
    )
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Pending,
    Allowed,
    Denied,
}
impl Decision {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Allowed => "allowed",
            Self::Denied => "denied",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionChoice {
    pub permission: String,
    pub scope: String,
    pub decision: Decision,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub permission: String,
    pub scope: String,
    pub decision: String,
    pub plugin_version: String,
    pub decided_at: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub plugin_id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub installed_version: String,
    pub api_version: String,
    pub storage_version: i64,
    pub package_path: Option<String>,
    pub package_sha256: String,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub release_page: Option<String>,
    pub network_origins: Vec<String>,
    pub is_bundled: bool,
    pub package_status: String,
    pub is_enabled: bool,
    pub installed_at: String,
    pub updated_at: String,
}
fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Plugin> {
    let origins: String = row.get(12)?;
    Ok(Plugin {
        plugin_id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        author: row.get(3)?,
        installed_version: row.get(4)?,
        api_version: row.get(5)?,
        storage_version: row.get(6)?,
        package_path: row.get(7)?,
        package_sha256: row.get(8)?,
        repository: row.get(9)?,
        homepage: row.get(10)?,
        release_page: row.get(11)?,
        network_origins: serde_json::from_str(&origins).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(12, rusqlite::types::Type::Text, Box::new(e))
        })?,
        is_bundled: row.get(13)?,
        package_status: row.get(14)?,
        is_enabled: row.get(15)?,
        installed_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}
const COLUMNS:&str="plugin_id,name,description,author,installed_version,api_version,storage_version,package_path,package_sha256,repository,homepage,release_page,network_origins_json,is_bundled,package_status,is_enabled,installed_at,updated_at";
pub fn get(c: &Connection, id: &str) -> AppResult<Plugin> {
    c.query_row(
        &format!("SELECT {COLUMNS} FROM game_plugins WHERE plugin_id=?1"),
        [id],
        row,
    )
    .optional()?
    .ok_or_else(AppError::not_found)
}
pub fn list(c: &Connection) -> AppResult<Vec<Plugin>> {
    let mut s = c.prepare(&format!(
        "SELECT {COLUMNS} FROM game_plugins ORDER BY is_bundled DESC,name,plugin_id"
    ))?;
    let rows = s.query_map([], row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}
pub fn permissions(c: &Connection, id: &str) -> AppResult<Vec<Permission>> {
    get(c, id)?;
    let mut s=c.prepare("SELECT permission,permission_scope,decision,plugin_version,decided_at FROM game_permissions WHERE plugin_id=?1 ORDER BY permission,permission_scope")?;
    let rows = s.query_map([id], |r| {
        Ok(Permission {
            permission: r.get(0)?,
            scope: r.get(1)?,
            decision: r.get(2)?,
            plugin_version: r.get(3)?,
            decided_at: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
pub fn allowed(
    c: &Connection,
    id: &str,
    version: &str,
    permission: &str,
    scope: &str,
) -> AppResult<bool> {
    Ok(c.query_row("SELECT EXISTS(SELECT 1 FROM game_permissions WHERE plugin_id=?1 AND plugin_version=?2 AND permission=?3 AND permission_scope=?4 AND decision='allowed')",params![id,version,permission,scope],|r|r.get(0))?)
}
pub fn authorize(c: &Connection, id: &str, version: &str) -> AppResult<Plugin> {
    let p = get(c, id)?;
    if !p.is_enabled || p.package_status != "available" || p.installed_version != version {
        return Err(denied());
    }
    Ok(p)
}
pub fn decide(
    c: &mut Connection,
    id: &str,
    expected_version: &str,
    choice: &PermissionChoice,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let tx = c.transaction()?;
    let p = get(&tx, id)?;
    if p.installed_version != expected_version {
        return Err(denied());
    }
    // 只能修改已登记的申请；不能借管理接口创造 manifest 没声明的权限。
    let at = if choice.decision == Decision::Pending {
        None
    } else {
        Some(stamp(now))
    };
    if tx.execute("UPDATE game_permissions SET decision=?1,decided_at=?2 WHERE plugin_id=?3 AND plugin_version=?4 AND permission=?5 AND permission_scope=?6",params![choice.decision.as_str(),at,id,expected_version,choice.permission,choice.scope])?!=1 {return Err(denied())}
    tx.commit()?;
    Ok(())
}
pub fn set_enabled(
    c: &Connection,
    id: &str,
    expected_version: &str,
    enabled: bool,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let p = get(c, id)?;
    if p.installed_version != expected_version || enabled && p.package_status != "available" {
        return Err(denied());
    }
    c.execute(
        "UPDATE game_plugins SET is_enabled=?1,updated_at=?2 WHERE plugin_id=?3",
        params![enabled, stamp(now), id],
    )?;
    Ok(())
}
/// 只能由校验后的安装器调用；文件先发布，事务提交前旧包和数据仍可用。
pub(crate) fn register(
    c: &Connection,
    m: &Manifest,
    path: &str,
    sha: &str,
    bundled: bool,
    now: DateTime<Utc>,
    choices: &[PermissionChoice],
) -> AppResult<()> {
    m.validate()?;
    let old = match get(c, &m.id) {
        Ok(p) => Some(p),
        Err(e) if e.code == "NOT_FOUND" => None,
        Err(e) => return Err(e),
    };
    if old.as_ref().is_some_and(|p| p.is_bundled != bundled) {
        return Err(AppError::new(
            "PLUGIN_SOURCE_CONFLICT",
            "不能用外部插件替换随应用提供的插件",
        ));
    }
    let old_permissions = if old.is_some() {
        permissions(c, &m.id)?
    } else {
        vec![]
    };
    let scopes = m.scopes();
    let mut selected = std::collections::HashSet::new();
    for choice in choices {
        if !scopes.contains(&(choice.permission.clone(), choice.scope.clone()))
            || !selected.insert((choice.permission.clone(), choice.scope.clone()))
        {
            return Err(AppError::validation("permissions", "权限选择重复或未声明"));
        }
    }
    c.execute("INSERT INTO game_plugins(plugin_id,name,description,author,installed_version,api_version,storage_version,source_type,package_path,package_sha256,repository,homepage,release_page,network_origins_json,is_bundled,package_status,is_enabled,installed_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,'available',1,?16,?16) ON CONFLICT(plugin_id) DO UPDATE SET name=excluded.name,description=excluded.description,author=excluded.author,installed_version=excluded.installed_version,api_version=excluded.api_version,storage_version=excluded.storage_version,package_path=excluded.package_path,package_sha256=excluded.package_sha256,repository=excluded.repository,homepage=excluded.homepage,release_page=excluded.release_page,network_origins_json=excluded.network_origins_json,package_status='available',updated_at=excluded.updated_at",params![m.id,m.name,m.description,m.author,m.version,m.api_version,m.storage_version,if bundled{"bundled"}else{"local"},path,sha,m.repository,m.homepage,m.release_page,serde_json::to_string(&m.network_origins).map_err(|_|AppError::storage())?,bundled,stamp(now)])?;
    // 缺包恢复成功后仍保持用户之前的停用状态；启用由用户单独决定。
    c.execute("DELETE FROM game_permissions WHERE plugin_id=?1", [&m.id])?;
    for (permission, scope) in scopes {
        let explicit = choices
            .iter()
            .find(|x| x.permission == permission && x.scope == scope);
        let previous = old_permissions
            .iter()
            .find(|x| x.permission == permission && x.scope == scope);
        let decision = explicit
            .map(|x| x.decision.as_str())
            .or(previous.map(|x| x.decision.as_str()))
            .unwrap_or("pending");
        let at = if decision == "pending" {
            None
        } else {
            explicit
                .map(|_| stamp(now))
                .or_else(|| previous.and_then(|x| x.decided_at.clone()))
        };
        c.execute(
            "INSERT INTO game_permissions VALUES(?1,?2,?3,?4,?5,?6)",
            params![m.id, permission, scope, decision, m.version, at],
        )?;
    }
    Ok(())
}
pub fn uninstall(
    c: &mut Connection,
    id: &str,
    version: &str,
    clear: bool,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let tx = c.transaction()?;
    let p = get(&tx, id)?;
    if p.is_bundled {
        return Err(AppError::new(
            "BUILTIN_PLUGIN",
            "随应用提供的插件只能停用或清除存档",
        ));
    }
    if p.installed_version != version {
        return Err(denied());
    }
    runs::stop_plugin(&tx, id, "cancelled", now)?;
    if clear {
        tx.execute("DELETE FROM game_plugins WHERE plugin_id=?1", [id])?;
    } else {
        tx.execute("UPDATE game_plugins SET package_path=NULL,package_status='missing_package',is_enabled=0,updated_at=?2 WHERE plugin_id=?1",params![id,stamp(now)])?;
    }
    tx.commit()?;
    Ok(())
}
pub fn clear_private_data(c: &mut Connection, id: &str, version: &str) -> AppResult<()> {
    let tx = c.transaction()?;
    let p = get(&tx, id)?;
    if p.installed_version != version {
        return Err(denied());
    }
    tx.execute("DELETE FROM game_plugin_data WHERE plugin_id=?1", [id])?;
    tx.execute("DELETE FROM game_plugin_settings WHERE plugin_id=?1", [id])?;
    tx.execute("UPDATE game_permissions SET decision='pending',decided_at=NULL WHERE plugin_id=?1", [id])?;
    tx.commit()?;
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrivateSnapshot {
    pub storage_version: i64,
    pub settings: std::collections::BTreeMap<String, Value>,
    pub data: std::collections::BTreeMap<String, Value>,
}
