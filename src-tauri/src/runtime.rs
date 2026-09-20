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
    pub ai: AppResult<crate::ai::client::AiClient>,
    pub requests: crate::state::RequestManager,
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
            ai: crate::ai::client::AiClient::new(Default::default()),
            requests: Default::default(),
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

#[cfg(test)]
pub(crate) fn test_runtime(url: &str) -> Runtime {
    struct Secret;
    impl SecretProtector for Secret {
        fn protect(&self, value: &str) -> AppResult<String> {
            Ok(value.into())
        }
        fn unprotect(&self, value: &str) -> AppResult<String> {
            Ok(value.into())
        }
    }
    let mut connection = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::initialize(&mut connection).unwrap();
    connection
        .execute(
            "INSERT INTO providers(name,base_url,model,is_active) VALUES('test',?1,'fake-model',1)",
            [url],
        )
        .unwrap();
    connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'测试','a','a')",[]).unwrap();
    connection
        .execute("INSERT INTO settings VALUES('current_session_id','1')", [])
        .unwrap();
    Runtime {
        store: Arc::new(Mutex::new(Ok(Store {
            connection,
            context_revision: 0,
        }))),
        secrets: Arc::new(Secret),
        ai: Ok(crate::ai::tests::client()),
        requests: Default::default(),
    }
}
