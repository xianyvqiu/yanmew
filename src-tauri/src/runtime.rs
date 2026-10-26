use crate::{
    error::{AppError, AppResult},
    platform::contracts::{AppPaths, SecretProtector},
};
use std::sync::{Arc, Mutex};
#[cfg(test)]
mod tests;

pub struct Store {
    pub(crate) skill_previews: std::collections::HashMap<String, crate::skills::tools::SharedPreview>,
    pub(crate) accessory_crafts: crate::accessories::craft::State,
    pub chat_requests: std::collections::HashMap<i64, crate::chat::RequestSnapshot>,
    pub weather: crate::weather::Cache,
    pub connection: rusqlite::Connection,
    pub context_revision: u64,
    pub pet: crate::pet::controller::Controller,
}
/// 数据库锁只包围同步事务；联网调用必须先取配置快照再释放锁。
#[derive(Clone)]
pub struct Runtime {
    /// 受管资源目录沿用平台注入，业务不重新推断可执行文件或工作目录。
    pub paths: AppResult<AppPaths>,
    pub(crate) backups: Arc<crate::import_export::restore::State>,
    pub(crate) reminder_polish: Arc<crate::reminders::polish::Service>,
    pub(crate) reminder_email: Arc<crate::reminder_email::Service>,
    /// 仅表示本次全新库初始化成功；窗口层首次展示使用，不写入设置或备份。
    pub initial_session_created: bool,
    pub store: Arc<Mutex<AppResult<Store>>>,
    pub secrets: Arc<dyn SecretProtector>,
    pub ai: AppResult<crate::ai::client::AiClient>,
    pub requests: crate::state::RequestManager,
    pub clock: Arc<dyn crate::platform::contracts::Clock>,
    pub random: Arc<Mutex<Box<dyn crate::pet::RandomSource + Send>>>,
    pub jobs_cancel: crate::state::Cancellation,
    pub memory: Arc<crate::memory::GenerationState>,
    pub(crate) travel_facts: Arc<crate::travel::facts::Worker>,
    pub(crate) travel_journals: Arc<crate::travel_journals::generation::Worker>,
    pub(crate) travel_gifts: Arc<crate::souvenirs::generation::Worker>,
    pub letters: Arc<crate::letters::GenerationState>,
    pub embedding: Arc<crate::embedding::Engine>,
    pub memory_index: Arc<crate::memory_index::State>,
}
impl Runtime {
    pub fn initialize(
        paths: AppResult<AppPaths>,
        secrets: Arc<dyn SecretProtector>,
        clock: Arc<dyn crate::platform::contracts::Clock>,
        random: Box<dyn crate::pet::RandomSource + Send>,
    ) -> Self {
        let embedding_paths = paths.as_ref().map_err(Clone::clone).and_then(|p| p.embedding_resources());
        let opened = paths.as_ref().map_err(Clone::clone).and_then(|paths| {
            let (mut connection, created) = crate::db::open(paths)?;
            crate::preset_personas::install(&mut connection, paths)?;
            Ok((connection, created))
        });
        let initial_session_created = opened.as_ref().is_ok_and(|(_, created)| *created);
        let store = opened.map(|(connection, _)| Store {
            skill_previews: Default::default(),
            accessory_crafts: Default::default(),
            weather: Default::default(),
            chat_requests: Default::default(),
            connection,
            context_revision: 0,
            pet: Default::default(),
        });
        let travel_gifts=Arc::new(crate::souvenirs::generation::Worker::default());
        if let (Ok(store),Ok(sample))=(&store,clock.sample()) {
            // 分类失败时默认不生图，避免恢复任务在状态不明时再次付费。
            let _=travel_gifts.initialize(&store.connection,sample.utc);
        }
        let runtime = Self {
            paths,
            initial_session_created,
            backups: Default::default(),
            reminder_polish: Default::default(),
            reminder_email: Default::default(),
            store: Arc::new(Mutex::new(store)),
            secrets,
            ai: crate::ai::client::AiClient::new(Default::default()),
            requests: Default::default(),
            clock,
            random: Arc::new(Mutex::new(random)),
            jobs_cancel: Default::default(),
            memory: Default::default(),
            letters: Default::default(),
            travel_gifts,
            travel_journals: Default::default(),
        travel_facts: Default::default(),
            embedding: Arc::new(crate::embedding::Engine::new(embedding_paths)),
            memory_index: Default::default(),
        };
        // 只清理本程序生成且数据库确认未引用的背景文件；清理失败不阻塞启动。
        crate::persona::background::cleanup_unused(&runtime);
        crate::accessories::images::cleanup_startup(&runtime);
        crate::chat_images::cleanup_unused(&runtime);
        runtime
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
        if let Some(id) = &context.session_id {
            if !self.requests.is_active(&format!("proactive:{id}"))? { store.pet.proactive_expression = None; }
        }
        let busy = context
            .session_id
            .as_ref()
            .map(|id| self.requests.is_active(&format!("chat:{id}")))
            .transpose()?
            .unwrap_or(false);
        let mut random = self.random.lock().map_err(|_| AppError::storage())?;
        let view = store.pet.settle(
            &mut store.connection,
            store.context_revision,
            sample,
            busy,
            random.as_mut(),
        )?;
        if !view.can_wander {
            if let Some(id) = &view.session_id { self.requests.cancel_key(&format!("proactive:{id}")); }
        }
        Ok(view)
    }
    pub fn pet_view(&self) -> AppResult<crate::pet::controller::View> {
        self.read(|store, secrets| self.settle_pet(store, secrets))
    }
    pub fn pet_interact(
        &self,
        input: crate::pet::controller::InteractionInput,
    ) -> AppResult<crate::pet::controller::View> {
        self.pet_interact_with_petting(input,false)
    }
    pub(crate) fn pet_interact_with_petting(&self,input:crate::pet::controller::InteractionInput,petted:bool)->AppResult<crate::pet::controller::View> {
        self.read(|store, secrets| {
            self.settle_pet(store, secrets)?;
            let proactive_key = format!("proactive:{}", input.session_id);
            let sample = self.clock.sample()?;
            {
                let mut random = self.random.lock().map_err(|_| AppError::storage())?;
                store.pet.interact_with_petting(
                    &mut store.connection,
                    store.context_revision,
                    input,
                    petted,
                    sample,
                    random.as_mut(),
                )?;
            }
            self.requests.cancel_key(&proactive_key);
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
            store.accessory_crafts.discard();
            if !store.pet.stopping {
                self.settle_pet(store, secrets)?;
                store.pet.stopping = true;
            }
            Ok(())
        })?;
        crate::accessories::images::cleanup_unused(self,&[]);
        Ok(())
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
    let connection = crate::db::test_database();
    connection
        .execute(
            "INSERT INTO providers(name,base_url) VALUES('test',?1)",
            [url],
        )
        .unwrap();
    connection.execute_batch("INSERT INTO provider_models(provider_id,purpose,model) VALUES(1,'chat','fake-model'); INSERT INTO provider_roles(purpose,provider_id) VALUES('chat',1);").unwrap();
    connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'测试','a','a')",[]).unwrap();
    connection
        .execute("INSERT INTO settings VALUES('current_session_id','1')", [])
        .unwrap();
    Runtime {
        paths: Err(AppError::storage()),
        initial_session_created: false,
        backups: Default::default(),
        reminder_polish: Default::default(),
            reminder_email: Default::default(),
        store: Arc::new(Mutex::new(Ok(Store {
            skill_previews: Default::default(),
            accessory_crafts: Default::default(),
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
        letters: Default::default(),
        travel_gifts: Default::default(),
        travel_journals: Default::default(),
        travel_facts: Default::default(),
        embedding: Arc::new(crate::embedding::Engine::new(Ok(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models/bge-small-zh-v1.5")))),
        memory_index: Default::default(),
    }
}
