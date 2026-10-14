use super::*;
use std::collections::HashSet;
pub const MAX_VALUE: usize = 1024 * 1024;
pub const MAX_STORAGE: usize = 10 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub api_version: String,
    pub entry: String,
    pub storage_version: i64,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub network_origins: Vec<String>,
    #[serde(default)]
    pub settings_schema: Vec<Setting>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    pub release_page: Option<String>,
    /// 存储版本变化时作者提供的 ES module，导出 migrate(snapshot, targetVersion)。
    pub migration: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub default: Value,
    #[serde(default)]
    pub required: bool,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    #[serde(default)]
    pub options: Vec<SettingOption>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingOption {
    pub value: Value,
    pub label: String,
}
pub fn key(key: &str) -> AppResult<()> {
    if key.is_empty()
        || key.len() > 128
        || key.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        })
    {
        return Err(AppError::validation(
            "key",
            "键名须为小写字母、数字、下划线或短横线组成的非空段，以点分隔，最多 128 字节",
        ));
    }
    Ok(())
}
pub fn path(path: &str) -> AppResult<()> {
    if path.is_empty()
        || path.len() > 240
        || path.split('/').any(|p| {
            p.is_empty()
                || p == "."
                || p == ".."
                || p.ends_with('.')
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
                || [
                    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7",
                    "LPT8", "LPT9",
                ]
                .contains(
                    &p.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                )
        })
    {
        return Err(AppError::validation(
            "path",
            "包路径须为 ASCII 字母、数字、点、下划线或短横线组成的安全相对路径",
        ));
    }
    Ok(())
}
pub fn origin(raw: &str) -> AppResult<String> {
    let u = url::Url::parse(raw)
        .map_err(|_| AppError::validation("origin", "来源不是有效 HTTPS origin"))?;
    if u.scheme() != "https"
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.path() != "/"
        || u.query().is_some()
        || u.fragment().is_some()
        || raw.contains('*')
    {
        return Err(AppError::validation(
            "origin",
            "来源只能包含 HTTPS 协议、主机和端口",
        ));
    }
    Ok(u.origin().ascii_serialization())
}
impl Setting {
    pub fn validate_value(&self, v: &Value) -> AppResult<()> {
        let valid = match self.kind.as_str() {
            "boolean" => v.is_boolean(),
            "integer" => v.as_i64().is_some(),
            "number" => v.as_f64().is_some(),
            "string" => v.is_string(),
            "enum" => self.options.iter().any(|x| &x.value == v),
            _ => false,
        };
        if !valid
            || v.as_f64().is_some_and(|n| {
                self.minimum.is_some_and(|min| n < min) || self.maximum.is_some_and(|max| n > max)
            })
            || v.as_str().is_some_and(|s| s.len() > 4096)
        {
            return Err(AppError::validation(
                &self.key,
                "插件设置类型、范围或选项无效",
            ));
        }
        Ok(())
    }
}
impl Manifest {
    pub fn validate(&self) -> AppResult<()> {
        if self.id.len() > 128
            || self.id.split('.').count() < 2
            || self.id.split('.').any(|p| {
                p.is_empty()
                    || !p
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            })
        {
            return Err(AppError::validation(
                "id",
                "插件 ID 必须为稳定的小写反向域名格式",
            ));
        }
        for (key, value, max) in [
            ("name", &self.name, 100),
            ("description", &self.description, 2000),
            ("author", &self.author, 100),
        ] {
            if value.trim().is_empty() || value.chars().count() > max {
                return Err(AppError::validation(key, "插件信息为空或过长"));
            }
        }
        semver::Version::parse(&self.version)
            .map_err(|_| AppError::validation("version", "版本必须符合 SemVer"))?;
        if self.api_version != "1.0.0" {
            return Err(AppError::new(
                "PLUGIN_API_INCOMPATIBLE",
                "当前宿主仅支持插件协议 1.0.0",
            ));
        }
        if self.storage_version <= 0 {
            return Err(AppError::validation(
                "storage_version",
                "存档版本必须是正整数",
            ));
        }
        path(&self.entry)?;
        if !self.entry.starts_with("ui/") || !self.entry.ends_with(".html") {
            return Err(AppError::validation("entry", "入口须为 ui/ 内的 HTML 文件"));
        }
        if let Some(m) = &self.migration {
            path(m)?;
            if !m.ends_with(".js") {
                return Err(AppError::validation(
                    "migration",
                    "迁移入口须为 JavaScript ES module",
                ));
            }
        }
        let mut permissions = HashSet::new();
        for p in &self.permissions {
            if !["wallet.modify", "network.fetch"].contains(&p.as_str()) || !permissions.insert(p) {
                return Err(AppError::validation("permissions", "权限不支持或重复"));
            }
        }
        if self.network_origins.len() > 16
            || self.permissions.iter().any(|p| p == "network.fetch")
                != !self.network_origins.is_empty()
        {
            return Err(AppError::validation(
                "network_origins",
                "联网权限须对应 1–16 个来源",
            ));
        }
        let mut origins = HashSet::new();
        for o in &self.network_origins {
            if origin(o)? != *o || !origins.insert(o) {
                return Err(AppError::validation(
                    "network_origins",
                    "来源必须规范化且不重复",
                ));
            }
        }
        if self.settings_schema.len() > 64 {
            return Err(AppError::validation(
                "settings_schema",
                "设置项不能超过 64 个",
            ));
        }
        let mut keys = HashSet::new();
        for s in &self.settings_schema {
            key(&s.key)?;
            if !keys.insert(&s.key)
                || s.title.trim().is_empty()
                || s.title.len() > 300
                || s.description.len() > 2000
                || s.options.len() > 100
                || s.minimum.zip(s.maximum).is_some_and(|(a, b)| a > b)
            {
                return Err(AppError::validation(
                    "settings_schema",
                    "设置项重复、范围无效或过大",
                ));
            }
            s.validate_value(&s.default)?;
        }
        for raw in [&self.repository, &self.homepage, &self.release_page]
            .into_iter()
            .flatten()
        {
            let u =
                url::Url::parse(raw).map_err(|_| AppError::validation("link", "来源链接无效"))?;
            if u.scheme() != "https"
                || u.host_str().is_none()
                || !u.username().is_empty()
                || u.password().is_some()
                || raw.len() > 2048
            {
                return Err(AppError::validation(
                    "link",
                    "来源链接须为无凭据 HTTPS 地址",
                ));
            }
        }
        if self.repository.as_ref().is_some_and(|r| {
            url::Url::parse(r)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .as_deref()
                != Some("github.com")
        }) {
            return Err(AppError::validation(
                "repository",
                "源码仓库须为 GitHub 地址",
            ));
        }
        Ok(())
    }
    pub fn scopes(&self) -> Vec<(String, String)> {
        let mut s = vec![];
        if self.permissions.iter().any(|p| p == "wallet.modify") {
            s.push(("wallet.modify".into(), "".into()));
        }
        for origin in &self.network_origins {
            s.push(("network.fetch".into(), origin.clone()));
        }
        s
    }
}
