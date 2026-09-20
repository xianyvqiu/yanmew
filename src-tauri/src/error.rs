use serde::Serialize;

/// 对外错误只包含稳定代码和可展示信息，不泄漏 SQL、路径或凭据。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}
pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field: None,
        }
    }
    pub fn validation(field: &str, message: &str) -> Self {
        Self {
            code: "VALIDATION_ERROR",
            message: message.into(),
            field: Some(field.into()),
        }
    }
    pub fn storage() -> Self {
        Self::new(
            "STORAGE_ERROR",
            "数据读写失败，请检查数据目录权限与磁盘空间",
        )
    }
    pub fn not_found() -> Self {
        Self::new("NOT_FOUND", "该记录不存在，请刷新后重试")
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for AppError {}
impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        Self::storage()
    }
}

pub fn parse_id(value: &str) -> AppResult<i64> {
    let id = value
        .parse::<i64>()
        .map_err(|_| AppError::validation("id", "记录 ID 必须是正整数"))?;
    if id <= 0 || id.to_string() != value {
        return Err(AppError::validation("id", "记录 ID 格式无效"));
    }
    Ok(id)
}
