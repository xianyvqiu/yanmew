//! 逻辑备份协议：只导出已持久化业务快照，不复制数据库、密钥、模型或程序包。
use crate::{
    error::{AppError, AppResult},
    runtime::Runtime,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

mod archive;
mod file;
pub(crate) mod restore;
mod protocol;
mod snapshot;
#[cfg(test)]
mod tests;

pub const FORMAT_VERSION: u32 = 1;
pub const SCHEMA_VERSION: i64 = 26;
pub type Record = Map<String, Value>;
#[cfg(test)]
pub type Tables = BTreeMap<String, Vec<Record>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    Full,
    Session,
    UserProfile,
    PersonaProfile,
    PersonaFull,
    PluginData,
}
impl PackageKind {
    fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Session => "session",
            Self::UserProfile => "user_profile",
            Self::PersonaProfile => "persona_profile",
            Self::PersonaFull => "persona_full",
            Self::PluginData => "plugin_data",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRequest {
    pub kind: PackageKind,
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub include_sessions: bool,
}
impl ExportRequest {
    fn validate(&self) -> AppResult<()> {
        if (self.kind == PackageKind::Full) != self.ids.is_empty()
            || (self.include_sessions && self.kind != PackageKind::UserProfile)
            || self
                .ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.ids.len()
        {
            return Err(AppError::validation(
                "selection",
                "请按包类型选择不重复的导出对象",
            ));
        }
        if self.kind != PackageKind::PluginData {
            for id in &self.ids {
                crate::error::parse_id(id)?;
            }
        } else if self.ids.iter().any(|id| id.trim().is_empty()) {
            return Err(AppError::validation("ids", "请选择有效的插件"));
        }
        Ok(())
    }
}

/// 文件清单仅含包内路径；不记录来源机器的绝对路径。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Warning {
    pub code: String,
    pub table: String,
    pub record_id: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format_version: u32,
    pub schema_version: i64,
    pub app_version: String,
    pub package_type: PackageKind,
    pub exported_at: String,
    pub selection: ExportRequest,
    pub record_counts: BTreeMap<String, u64>,
    pub data_size: u64,
    pub data_sha256: String,
    pub files: Vec<FileEntry>,
    pub warnings: Vec<Warning>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportArchive {
    pub filename: String,
    pub size: u64,
    pub sha256: String,
    pub manifest: Manifest,
}
/// 平台选择保存位置，业务只返回摘要；不通过IPC传输整个ZIP/base64。
pub fn export(
    runtime: &Runtime,
    request: ExportRequest,
    transfer: &dyn crate::platform::contracts::FileTransferPort,
) -> AppResult<Option<ExportArchive>> {
    request.validate()?;
    let date = runtime.clock.sample()?.utc;
    let filename = format!(
        "yanmew-{}-{}.zip",
        request.kind.name(),
        date.format("%Y%m%d-%H%M%S")
    );
    let Some(destination) = transfer.save_backup(&filename)? else {
        return Ok(None);
    };
    file::save(runtime, request, &destination, transfer).map(Some)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOption {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    pub personas: Vec<ExportOption>,
    pub sessions: Vec<ExportOption>,
    pub user_profiles: Vec<ExportOption>,
    pub plugins: Vec<ExportOption>,
}
pub fn options(runtime: &Runtime) -> AppResult<ExportOptions> {
    runtime.read(|store,_| {
        fn list(c:&rusqlite::Connection,sql:&str)->AppResult<Vec<ExportOption>> {
            let mut stmt=c.prepare(sql)?;
            let rows=stmt.query_map([],|r|Ok(ExportOption{id:r.get(0)?,name:r.get(1)?}))?;
            Ok(rows.collect::<Result<Vec<_>,_>>()?)
        }
        let c=&store.connection;
        Ok(ExportOptions {
            personas:list(c,"SELECT CAST(id AS TEXT),name FROM personas ORDER BY id")?,
            sessions:list(c,"SELECT CAST(s.id AS TEXT),p.name || ' · ' || s.title FROM sessions s JOIN personas p ON p.id=s.persona_id ORDER BY s.id")?,
            user_profiles:list(c,"SELECT CAST(id AS TEXT),name FROM user_profiles ORDER BY id")?,
            plugins:list(c,"SELECT plugin_id,name FROM game_plugins ORDER BY plugin_id")?,
        })
    })
}

fn invalid() -> AppError {
    AppError::new(
        "BACKUP_RELATION_INVALID",
        "备份数据关联不完整，请修复数据后再导出",
    )
}
fn value<'a>(record: &'a Record, column: &str) -> AppResult<&'a str> {
    record
        .get(column)
        .and_then(Value::as_str)
        .ok_or_else(invalid)
}
