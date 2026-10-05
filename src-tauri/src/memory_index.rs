//! 向量缓存补建，接入唯一 jobs 循环；失败后等待明确恢复，不紧密重试。
use crate::{
    embedding::{self, Vector},
    embedding_providers::{self, Stamp},
    error::{AppError, AppResult},
    runtime::Runtime,
};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::{collections::HashSet, sync::Mutex};

#[derive(Default)]
pub struct State {
    inner: Mutex<Inner>,
}
#[derive(Default)]
struct Inner {
    epoch: u64,
    failure: Option<AppError>,
    unavailable: HashSet<(i64, String)>,
    running: bool,
    model_key: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub total: usize,
    pub ready: usize,
    pub unavailable: usize,
    pub pending: usize,
    pub running: bool,
    pub error: Option<AppError>,
}
impl State {
    pub fn reset(&self) -> AppResult<()> {
        let mut state = self.inner.lock().map_err(|_| AppError::storage())?;
        state.epoch += 1;
        state.failure = None;
        state.unavailable.clear();
        state.model_key = None;
        Ok(())
    }
    pub fn report(&self, error: AppError) {
        if let Ok(mut state) = self.inner.lock() {
            state.failure = Some(error);
        }
    }
    /// 查询响应的维度就是实际模型身份；维度变化的旧缓存只视为待重建。
    pub fn observed(&self, key: &str) -> AppResult<()> {
        self.inner
            .lock()
            .map_err(|_| AppError::storage())?
            .model_key = Some(key.to_owned());
        Ok(())
    }
}
struct Item {
    id: i64,
    session: i64,
    title: String,
    content: String,
    hash: String,
    cache_key: Option<String>,
    cache_valid: bool,
}
fn items(runtime: &Runtime) -> AppResult<Vec<Item>> {
    runtime.read(|store, _| {
        let mut statement = store.connection.prepare("SELECT l.id,l.session_id,l.title,l.content,e.model_key,e.content_hash,e.dimensions,e.vector FROM long_memories l LEFT JOIN long_memory_embeddings e ON e.long_memory_id=l.id AND e.session_id=l.session_id ORDER BY (l.session_id=CAST((SELECT value FROM settings WHERE key='current_session_id') AS INTEGER)) DESC,l.id")?;
        let rows = statement.query_map([], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<usize>>(6)?,r.get::<_,Option<Vec<u8>>>(7)?)))?;
        let mut result = Vec::new();
        for row in rows {
            let (id, session, title, content, cache_key, cache_hash, dim, bytes) = row?;
            let hash = embedding::content_hash(&title,&content);
            let cache_valid = cache_hash.as_ref() == Some(&hash) && dim.zip(bytes.as_ref()).is_some_and(|(d,b)| embedding::decode(b,d).is_ok());
            result.push(Item {id,session,title,content,hash,cache_key,cache_valid});
        }
        Ok(result)
    })
}
fn valid(item: &Item, identity: &str, known_key: Option<&str>) -> bool {
    item.cache_valid
        && item
            .cache_key
            .as_deref()
            .is_some_and(|key| match known_key {
                Some(expected) => key == expected,
                None => key.starts_with(&format!("{identity}:")),
            })
}
pub fn view(runtime: &Runtime) -> AppResult<View> {
    let rows = items(runtime)?;
    let stamp = runtime.read(|store, _| embedding_providers::stamp(&store.connection, None))?;
    let identity = embedding::identity(&runtime.embedding, &stamp);
    let state = runtime
        .memory_index
        .inner
        .lock()
        .map_err(|_| AppError::storage())?;
    let ready = identity
        .as_ref()
        .map(|key| {
            rows.iter()
                .filter(|row| valid(row, key, state.model_key.as_deref()))
                .count()
        })
        .unwrap_or(0);
    let unavailable = rows
        .iter()
        .filter(|row| state.unavailable.contains(&(row.id, row.hash.clone())))
        .count();
    Ok(View {
        total: rows.len(),
        ready,
        unavailable,
        pending: rows.len().saturating_sub(ready + unavailable),
        running: state.running,
        error: state.failure.clone().or(identity.err()),
    })
}
struct Task {
    item: Item,
    stamp: Stamp,
    epoch: u64,
}
fn next(runtime: &Runtime) -> AppResult<Option<Task>> {
    let stamp = runtime.read(|store, _| embedding_providers::stamp(&store.connection, None))?;
    // 没有记忆不加载模型，也不把模型空闲当作错误。
    let rows = items(runtime)?;
    if rows.is_empty() {
        return Ok(None);
    }
    let state = runtime
        .memory_index
        .inner
        .lock()
        .map_err(|_| AppError::storage())?;
    if state.failure.is_some() {
        return Ok(None);
    }
    let identity = embedding::identity(&runtime.embedding, &stamp)?;
    Ok(rows
        .into_iter()
        .find(|row| {
            !valid(row, &identity, state.model_key.as_deref())
                && !state.unavailable.contains(&(row.id, row.hash.clone()))
        })
        .map(|item| Task {
            item,
            stamp,
            epoch: state.epoch,
        }))
}
fn commit(runtime: &Runtime, task: &Task, vector: Vector) -> AppResult<bool> {
    runtime.jobs_cancel.check()?;
    let state = runtime
        .memory_index
        .inner
        .lock()
        .map_err(|_| AppError::storage())?;
    if state.epoch != task.epoch {
        return Ok(false);
    }
    if vector.model_key
        != format!(
            "{}:{}",
            embedding::identity(&runtime.embedding, &task.stamp)?,
            vector.values.len()
        )
    {
        return Ok(false);
    }
    runtime.read(|store, _| {
        if embedding_providers::stamp(&store.connection,None)? != task.stamp { return Ok(false); }
        let row: Option<(i64,String,String)> = store.connection.query_row("SELECT session_id,title,content FROM long_memories WHERE id=?1",[task.item.id],|r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if !row.is_some_and(|(session,title,content)| session == task.item.session && embedding::content_hash(&title,&content) == task.item.hash) { return Ok(false); }
        let values = embedding::normalize(vector.values)?;
        store.connection.execute("INSERT INTO long_memory_embeddings(long_memory_id,session_id,model_key,dimensions,vector,content_hash,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(long_memory_id) DO UPDATE SET session_id=excluded.session_id,model_key=excluded.model_key,dimensions=excluded.dimensions,vector=excluded.vector,content_hash=excluded.content_hash,updated_at=excluded.updated_at", params![task.item.id,task.item.session,vector.model_key,values.len(),embedding::encode(&values),task.item.hash,runtime.clock.sample()?.utc.to_rfc3339()])?;
        Ok(true)
    })
}
#[derive(Default)]
pub struct Worker {
    running: Option<tokio::sync::oneshot::Receiver<()>>,
}
impl Worker {
    pub fn tick(
        &mut self,
        runtime: &Runtime,
        events: std::sync::Arc<dyn crate::events::MemoryEventSink>,
    ) {
        if let Some(receiver) = &mut self.running {
            match receiver.try_recv() {
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => return,
                _ => self.running = None,
            }
        }
        let runtime = runtime.clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.running = Some(receiver);
        tauri::async_runtime::spawn(async move {
            let worker = runtime.clone();
            let task = tauri::async_runtime::spawn_blocking(move || next(&worker))
                .await
                .unwrap_or_else(|_| Err(AppError::storage()));
            match task {
                Ok(Some(task)) => {
                    if let Ok(mut state) = runtime.memory_index.inner.lock() {
                        state.running = true;
                    }
                    events.embedding_changed();
                    let result = embedding::generate(
                        &runtime,
                        &task.stamp,
                        format!("{}\n{}", task.item.title, task.item.content),
                        false,
                        &runtime.jobs_cancel,
                    )
                    .await;
                    if let Ok(mut state) = runtime.memory_index.inner.lock() {
                        state.running = false;
                        if state.epoch == task.epoch {
                            if let Err(error) = &result {
                                if error.code == "EMBEDDING_TOO_LONG" {
                                    state
                                        .unavailable
                                        .insert((task.item.id, task.item.hash.clone()));
                                } else if error.code != "CANCELLED" {
                                    state.failure = Some(error.clone());
                                }
                            } else if let Ok(vector) = &result {
                                state.model_key = Some(vector.model_key.clone());
                            }
                        }
                    }
                    if let Ok(vector) = result {
                        let worker = runtime.clone();
                        let committed = tauri::async_runtime::spawn_blocking(move || {
                            commit(&worker, &task, vector)
                        })
                        .await
                        .unwrap_or_else(|_| Err(AppError::storage()));
                        if let Err(error) = committed {
                            if error.code != "CANCELLED" {
                                runtime.memory_index.report(error);
                            }
                        }
                    }
                    events.embedding_changed();
                }
                Err(error) => {
                    runtime.memory_index.report(error);
                    events.embedding_changed();
                }
                Ok(None) => {}
            }
            let _ = sender.send(());
        });
    }
}

#[cfg(test)]
pub(crate) mod tests;
