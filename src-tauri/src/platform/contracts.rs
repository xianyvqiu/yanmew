use crate::error::AppResult;
use std::path::PathBuf;

/// 业务只使用受管路径；可执行文件位置由各平台决定。
#[derive(Clone)]
pub struct AppPaths {
    pub data_root: PathBuf,
    /// 发布资源跟随exe；安装版数据根与此目录分离。
    pub resource_root: PathBuf,
}
impl AppPaths {
    /// 便携模式及隔离测试沿用原数据目录，资源在其同级resources。
    pub fn portable(data_root: PathBuf) -> Self {
        let resource_root = data_root.parent().unwrap_or(&data_root).join("resources");
        Self { data_root, resource_root }
    }
    /// 便携版随包插件资源；运行用的不可变版本副本仍在 data/plugins 下。
    pub fn plugin_resources(&self) -> AppResult<PathBuf> {
        if cfg!(debug_assertions) { Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/plugins")) }
        else { Ok(self.resource_root.join("plugins")) }
    }
    /// 开发资源显式定位到项目；发布版资源跟随可执行文件，不依赖工作目录。
    pub fn embedding_resources(&self) -> AppResult<PathBuf> {
        if cfg!(debug_assertions) {
            Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models/bge-small-zh-v1.5"))
        } else {
            Ok(self.resource_root.join("models/bge-small-zh-v1.5"))
        }
    }
    /// 预置图片人设随完整程序包分发，用户自己的图片保存在data受管目录。
    pub fn persona_resources(&self) -> AppResult<PathBuf> {
        if cfg!(debug_assertions) {
            Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/personas"))
        } else {
            Ok(self.resource_root.join("personas"))
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

/// 用户主动选择的备份文件端口；共享业务不依赖系统窗口句柄。
pub trait FileTransferPort: Send + Sync {
    fn save_backup(&self, suggested_name: &str) -> AppResult<Option<PathBuf>>;
    fn open_backup(&self) -> AppResult<Option<PathBuf>> {Err(crate::error::AppError::new("PLATFORM_UNSUPPORTED","当前平台尚未接入备份文件选择"))}
    /// 临时文件与正式文件在同一目录；失败时保留原正式文件。
    fn publish(&self, temporary: &std::path::Path, destination: &std::path::Path) -> AppResult<()>;
}
