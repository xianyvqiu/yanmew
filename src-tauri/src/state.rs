use crate::error::{AppError, AppResult};
use std::{
    collections::HashMap,
    future::Future,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::watch;

/// 取消值具有记忆性；取消早于等待注册时也不会丢失。
#[derive(Clone)]
pub struct Cancellation(watch::Sender<bool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    pub fn check(&self) -> AppResult<()> {
        if *self.0.borrow() {
            Err(AppError::new("CANCELLED", "请求已取消"))
        } else {
            Ok(())
        }
    }
    async fn cancelled(&self) {
        let mut rx = self.0.subscribe();
        loop {
            if *rx.borrow_and_update() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
    pub async fn wait<T>(
        &self,
        timeout: Duration,
        future: impl Future<Output = AppResult<T>>,
    ) -> AppResult<T> {
        tokio::select! { biased;
            _=self.cancelled()=>Err(AppError::new("CANCELLED","请求已取消")),
            result=tokio::time::timeout(timeout,future)=>result.map_err(|_|AppError::new("AI_TIMEOUT","模型响应超时，请检查网络或提供商状态"))?,
        }
    }
}
struct Active {
    id: u64,
    persona_id: i64,
    cancel: Cancellation,
}
#[derive(Default)]
struct Requests {
    next: AtomicU64,
    stopping: AtomicBool,
    active: Mutex<HashMap<String, Active>>,
}
#[derive(Clone, Default)]
pub struct RequestManager(Arc<Requests>);
pub struct RequestLease {
    pub id: u64,
    pub cancel: Cancellation,
    key: String,
    owner: RequestManager,
}
impl RequestManager {
    pub fn start(&self, key: String, persona_id: i64) -> AppResult<RequestLease> {
        let mut active = self.0.active.lock().map_err(|_| AppError::storage())?;
        if self.0.stopping.load(Ordering::Acquire) {
            return Err(AppError::new("CANCELLED", "应用正在退出，不再启动新请求"));
        }
        if active.contains_key(&key) {
            return Err(AppError::new("REQUEST_BUSY", "此任务正在进行，请稍后重试"));
        }
        let id = self.0.next.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = Cancellation::default();
        active.insert(
            key.clone(),
            Active {
                id,
                persona_id,
                cancel: cancel.clone(),
            },
        );
        Ok(RequestLease {
            id,
            cancel,
            key,
            owner: self.clone(),
        })
    }
    pub fn cancel_persona(&self, id: i64) {
        if let Ok(active) = self.0.active.lock() {
            for item in active.values().filter(|a| a.persona_id == id) {
                item.cancel.cancel();
            }
        }
    }
    pub fn cancel_all(&self) {
        if let Ok(active) = self.0.active.lock() {
            for item in active.values() {
                item.cancel.cancel();
            }
        }
    }
    pub fn shutdown(&self) {
        self.0.stopping.store(true, Ordering::Release);
        self.cancel_all();
    }
    pub fn cancel_key(&self, key: &str) {
        if let Ok(active) = self.0.active.lock() {
            if let Some(item) = active.get(key) {
                item.cancel.cancel();
            }
        }
    }
}
impl Drop for RequestLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.owner.0.active.lock() {
            if active.get(&self.key).is_some_and(|a| a.id == self.id) {
                active.remove(&self.key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_latched_and_registration_is_exclusive() {
        tauri::async_runtime::block_on(async {
            let manager = RequestManager::default();
            let request = manager.start("chat:1".into(), 1).unwrap();
            assert!(manager.start("chat:1".into(), 1).is_err());
            manager.cancel_persona(2);
            assert!(request.cancel.check().is_ok());
            manager.cancel_persona(1);
            assert_eq!(
                request
                    .cancel
                    .wait(Duration::from_secs(1), async { Ok(()) })
                    .await
                    .unwrap_err()
                    .code,
                "CANCELLED"
            );
            drop(request);
            assert!(manager.start("chat:1".into(), 1).is_ok());
            manager.shutdown();
            assert!(manager.start("chat:2".into(), 2).is_err());
            let cancel = Cancellation::default();
            assert_eq!(
                cancel
                    .wait(
                        Duration::from_millis(10),
                        std::future::pending::<AppResult<()>>()
                    )
                    .await
                    .unwrap_err()
                    .code,
                "AI_TIMEOUT"
            );
        });
    }
}
