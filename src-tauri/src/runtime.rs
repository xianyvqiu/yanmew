use crate::{
    error::{AppError, AppResult},
    platform::contracts::{AppPaths, SecretProtector},
};
use std::sync::{Arc, Mutex};

pub struct Store {
    pub connection: rusqlite::Connection,
    pub context_revision: u64,
}
/// 数据库锁只包围同步事务；联网调用必须先取配置快照再释放锁。
#[derive(Clone)]
pub struct Runtime {
    pub store: Arc<Mutex<AppResult<Store>>>,
    pub secrets: Arc<dyn SecretProtector>,
}
impl Runtime {
    pub fn initialize(paths: AppResult<AppPaths>, secrets: Arc<dyn SecretProtector>) -> Self {
        let store = paths
            .and_then(|p| crate::db::open(&p))
            .map(|connection| Store {
                connection,
                context_revision: 0,
            });
        Self {
            store: Arc::new(Mutex::new(store)),
            secrets,
        }
    }
    pub fn read<T>(
        &self,
        action: impl FnOnce(&mut Store, &dyn SecretProtector) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut guard = self.store.lock().map_err(|_| AppError::storage())?;
        let store = guard.as_mut().map_err(|e| e.clone())?;
        action(store, self.secrets.as_ref())
    }
}
