use crate::error::AppResult;
use std::path::PathBuf;

/// 业务只使用受管路径；可执行文件位置由各平台决定。
#[derive(Clone)]
pub struct AppPaths {
    pub data_root: PathBuf,
}

pub trait SecretProtector: Send + Sync {
    fn protect(&self, plain: &str) -> AppResult<String>;
    fn unprotect(&self, encrypted: &str) -> AppResult<String>;
}
