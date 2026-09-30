use crate::{
    error::{AppError, AppResult},
    platform::contracts::{AppPaths, SecretProtector},
};
use std::sync::{Arc, Mutex};
#[cfg(test)]
mod tests;

pub struct Store {
    pub chat_requests: std::collections::HashMap<i64, crate::chat::RequestSnapshot>,
    pub weather: crate::weather::Cache,
    pub connection: rusqlite::Connection,
    pub context_revision: u64,
    pub pet: crate::pet::controller::Controller,
}
/// 数据库锁只包围同步事务；联网调用必须先取配置快照再释放锁。
#[derive(Clone)]
pub struct Runtime {
    pub store: Arc<Mutex<AppResult<Store>>>,
    pub secrets: Arc<dyn SecretProtector>,
    pub ai: AppResult<crate::ai::client::AiClient>,
    pub requests: crate::state::RequestManager,
    pub clock: Arc<dyn crate::platform::contracts::Clock>,
    pub random: Arc<Mutex<Box<dyn crate::pet::RandomSource + Send>>>,
    pub jobs_cancel: crate::state::Cancellation,
    pub memory: Arc<crate::memory::GenerationState>,
}
impl Runtime {
    pub fn initialize(
        paths: AppResult<AppPaths>,
        secrets: Arc<dyn SecretProtector>,
        clock: Arc<dyn crate::platform::contracts::Clock>,
        random: Box<dyn crate::pet::RandomSource + Send>,
    ) -> Self {
        let store = paths
            .and_then(|p| crate::db::open(&p))
            .map(|connection| Store {
                weather: Default::default(),
                chat_requests: Default::default(),
                connection,
                context_revision: 0,
                pet: Default::default(),
            });
        Self {
            store: Arc::new(Mutex::new(store)),
            secrets,
            ai: crate::ai::client::AiClient::new(Default::default()),
            requests: Default::default(),
            clock,
            random: Arc::new(Mutex::new(random)),
            jobs_cancel: Default::default(),
            memory: Default::default(),
        }
    }

    /// 调用者持有唯一 store 锁，时钟在取得锁之后采样；不得在锁外计算旧增量。
    pub fn settle_pet(
        &self,
        store: &mut Store,
        secrets: &dyn SecretProtector,
    ) -> AppResult<crate::pet::controller::View> {
        store.pet.health(&store.connection, secrets)?;
        let sample = self.clock.sample()?;
        let context = crate::persona::context(&store.connection, store.context_revision)?;
        let busy = context
            .session_id
            .as_ref()
            .map(|id| self.requests.is_active(&format!("chat:{id}")))
            .transpose()?
            .unwrap_or(false);
        let mut random = self.random.lock().map_err(|_| AppError::storage())?;
        store.pet.settle(
            &mut store.connection,
            store.context_revision,
            sample,
            busy,
            random.as_mut(),
        )
    }
    pub fn pet_view(&self) -> AppResult<crate::pet::controller::View> {
        self.read(|store, secrets| self.settle_pet(store, secrets))
    }
    pub fn pet_interact(
        &self,
        input: crate::pet::controller::InteractionInput,
    ) -> AppResult<crate::pet::controller::View> {
        self.read(|store, secrets| {
            self.settle_pet(store, secrets)?;
            let sample = self.clock.sample()?;
            {
                let mut random = self.random.lock().map_err(|_| AppError::storage())?;
                store.pet.interact(
                    &mut store.connection,
                    store.context_revision,
                    input,
                    sample,
                    random.as_mut(),
                )?;
            }
            self.settle_pet(store, secrets)
        })
    }
    pub fn pet_visibility(&self, hidden: Option<bool>, dragging: Option<bool>) -> AppResult<()> {
        self.read(|store, secrets| {
            let checkpoint = self.settle_pet(store, secrets);
            if let Some(value) = hidden {
                store.pet.hidden = value;
            }
            if let Some(value) = dragging {
                store.pet.dragging = value;
            }
            if let Err(error) = checkpoint {
                store.pet.block_idle_interval();
                return Err(error);
            }
            self.settle_pet(store, secrets)?;
            Ok(())
        })
    }
    pub fn shutdown(&self) -> AppResult<()> {
        self.jobs_cancel.cancel();
        self.requests.shutdown();
        crate::chat::interrupt_active(self)?;
        self.read(|store, secrets| {
            if !store.pet.stopping {
                self.settle_pet(store, secrets)?;
                store.pet.stopping = true;
            }
            Ok(())
        })
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
    struct TestClock(std::time::Instant);
    impl crate::platform::contracts::Clock for TestClock {
        fn sample(&self) -> AppResult<crate::platform::contracts::ClockSample> {
            Ok(crate::platform::contracts::ClockSample {
                utc: chrono::Utc::now(),
                running: self.0.elapsed(),
            })
        }
    }
    struct TestRandom;
    impl crate::pet::RandomSource for TestRandom {
        fn below(&mut self, upper_exclusive: u64) -> AppResult<u64> {
            if upper_exclusive == 0 {
                return Err(AppError::storage());
            }
            Ok(0)
        }
    }
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
            weather: Default::default(),
                chat_requests: Default::default(),
            connection,
            context_revision: 0,
            pet: Default::default(),
        }))),
        secrets: Arc::new(Secret),
        ai: Ok(crate::ai::tests::client()),
        requests: Default::default(),
        clock: Arc::new(TestClock(std::time::Instant::now())),
        random: Arc::new(Mutex::new(Box::new(TestRandom))),
        jobs_cancel: Default::default(),
        memory: Default::default(),
    }
}
