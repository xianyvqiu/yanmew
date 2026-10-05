use crate::error::AppResult;
use std::path::PathBuf;

/// 业务只使用受管路径；可执行文件位置由各平台决定。
#[derive(Clone)]
pub struct AppPaths {
    pub data_root: PathBuf,
}
impl AppPaths {
    /// 开发资源显式定位到项目；发布版资源跟随可执行文件，不依赖工作目录。
    pub fn embedding_resources(&self) -> AppResult<PathBuf> {
        if cfg!(debug_assertions) {
            Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models/bge-small-zh-v1.5"))
        } else {
            Ok(self.data_root.parent().ok_or_else(crate::error::AppError::storage)?.join("resources/models/bge-small-zh-v1.5"))
        }
    }
}

pub trait SecretProtector: Send + Sync {
    fn protect(&self, plain: &str) -> AppResult<String>;
    fn unprotect(&self, encrypted: &str) -> AppResult<String>;
}

/// 平台定位端口；业务层不依赖窗口句柄或 Windows 类型。
pub trait LocationSource: Send + Sync {
    fn locate(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<crate::location::Position>> + Send + '_>>;
}

#[derive(Clone, Copy)]
pub struct ClockSample {
    pub utc: chrono::DateTime<chrono::Utc>,
    /// 有效单调时间不包含系统睡眠；绝不保存到数据库或从备份恢复。
    pub running: std::time::Duration,
}
pub trait Clock: Send + Sync {
    fn sample(&self) -> AppResult<ClockSample>;
}
