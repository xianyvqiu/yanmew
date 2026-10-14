//! 插件实例协调与桥接业务；窗口创建交给平台适配，数据与网络规则独立于窗口。
use crate::{
    error::{AppError, AppResult},
    game_plugins::{self as plugins, package::Package, PrivateSnapshot},
    runtime::Runtime,
    state::Cancellation,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
pub mod install;
mod network;
#[cfg(test)]
mod tests;

pub struct Instance {
    pub label: String,
    pub package: Package,
    pub run_id: Option<String>,
    pub cancel: Cancellation,
    pub migration: Option<Migration>,
    pub network_slots: Arc<tokio::sync::Semaphore>,
}
pub struct Migration {
    pub snapshot: PrivateSnapshot,
    pub result: Mutex<Option<tokio::sync::oneshot::Sender<AppResult<PrivateSnapshot>>>>,
}
#[derive(Default)]
pub struct Host {
    pub ready: std::sync::atomic::AtomicBool,
    instances: Mutex<HashMap<String, Arc<Instance>>>,
    pub operations: tokio::sync::Mutex<()>,
    pub pending: Mutex<Option<install::Pending>>,
}
impl Host {
    pub fn ensure_ready(&self) -> AppResult<()> {
        if !self.ready.load(std::sync::atomic::Ordering::Acquire) {
            return Err(AppError::new(
                "PLUGIN_STARTING",
                "正在恢复插件状态，请稍后再试",
            ));
        }
        Ok(())
    }
    pub fn instance(&self, label: &str) -> AppResult<Arc<Instance>> {
        self.instances
            .lock()
            .map_err(|_| AppError::storage())?
            .get(label)
            .cloned()
            .ok_or_else(plugins::denied)
    }
    pub fn insert(&self, i: Arc<Instance>) -> AppResult<()> {
        self.instances
            .lock()
            .map_err(|_| AppError::storage())?
            .insert(i.label.clone(), i);
        Ok(())
    }
    pub fn game(&self, id: &str) -> AppResult<Option<Arc<Instance>>> {
        Ok(self
            .instances
            .lock()
            .map_err(|_| AppError::storage())?
            .values()
            .find(|i| i.migration.is_none() && i.package.manifest.id == id)
            .cloned())
    }
    pub fn remove(&self, label: &str) -> Option<Arc<Instance>> {
        self.instances.lock().ok()?.remove(label)
    }
    pub fn restricted(label: &str) -> bool {
        label.starts_with("game-") || label.starts_with("migration-")
    }
    pub fn command_allowed(label: &str, command: &str) -> bool {
        !Self::restricted(label) || command == "game_plugin_call"
    }
    /// 用主程序注入的随机源产生标签；标签只是映射键，不让插件提交标签来选择身份。
    pub fn token(runtime: &Runtime, prefix: &str) -> AppResult<String> {
        let mut random = runtime.random.lock().map_err(|_| AppError::storage())?;
        let mut token = prefix.to_owned();
        for _ in 0..4 {
            token.push_str(&format!("{:08x}", random.below(1u64 << 32)?));
        }
        Ok(token)
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub language: &'static str,
    pub game_run_id: String,
    pub wallet_permission_granted: bool,
    pub plugin_version: String,
    pub permissions: Vec<plugins::Permission>,
    pub settings: std::collections::BTreeMap<String, Value>,
}
pub async fn open(app: AppHandle, runtime: Runtime, id: String) -> AppResult<String> {
    let host = app.state::<Host>();
    let _operation = host.operations.lock().await;
    host.ensure_ready()?;
    if let Some(i) = host.game(&id)? {
        if let Some(w) = app.get_webview_window(&i.label) {
            w.show().map_err(|_| AppError::storage())?;
            w.set_focus().map_err(|_| AppError::storage())?;
            return Ok(i.label.clone());
        }
    }
    let root = plugins::package::root(runtime.paths.as_ref().map_err(Clone::clone)?);
    let worker = runtime.clone();
    let pkg = tauri::async_runtime::spawn_blocking(move || {
        let p = worker.read(|s, _| {
            let p = plugins::get(&s.connection, &id)?;
            plugins::authorize(&s.connection, &id, &p.installed_version)
        })?;
        plugins::package::load(&root, &p)
    })
    .await
    .map_err(|_| AppError::storage())??;
    let m = pkg.manifest.clone();
    let label = Host::token(&runtime, "game-")?;
    let run = runtime.read(|s, _| {
        plugins::runs::start(
            &s.connection,
            &m.id,
            &m.version,
            runtime.clock.sample()?.utc,
        )
    })?;
    let i = Arc::new(Instance {
        label: label.clone(),
        package: pkg,
        run_id: Some(run.id),
        cancel: Default::default(),
        migration: None,
        network_slots: Arc::new(tokio::sync::Semaphore::new(4)),
    });
    host.insert(i.clone())?;
    if let Err(e) = crate::platform::windows::game_host::create(&app, &runtime, &i, false) {
        closed(&app, &label, "error");
        return Err(e);
    }
    let _ = app.emit("game_plugins_changed", ());
    Ok(label)
}
/// 关闭、停用、更新前都先撤销实例；迟到联网结果不能访问旧版本。
pub fn closed(app: &AppHandle, label: &str, status: &str) {
    if let Some(i) = app.state::<Host>().remove(label) {
        i.cancel.cancel();
        if let Some(m) = &i.migration {
            if let Ok(mut result) = m.result.lock() {
                if let Some(sender) = result.take() {
                    let _ = sender.send(Err(AppError::new(
                        "PLUGIN_MIGRATION_FAILED",
                        "存档迁移实例已结束，原数据保留",
                    )));
                }
            }
        }
        if let Some(run) = &i.run_id {
            let runtime = app.state::<Runtime>();
            let _ = runtime.read(|s, _| {
                let at = runtime.clock.sample()?.utc;
                s.connection.execute(
                    "UPDATE game_runs SET status=?2,ended_at=?3 WHERE id=?1 AND status='running'",
                    rusqlite::params![crate::error::parse_id(run)?, status, plugins::stamp(at)],
                )?;
                Ok(())
            });
        }
    }
}
pub fn close_plugin(app: &AppHandle, id: &str) -> AppResult<()> {
    if let Some(i) = app.state::<Host>().game(id)? {
        closed(app, &i.label, "cancelled");
        if let Some(w) = app.get_webview_window(&i.label) {
            w.close().map_err(|_| AppError::storage())?;
        }
    }
    Ok(())
}
pub fn shutdown(app: &AppHandle) {
    let labels = app
        .state::<Host>()
        .instances
        .lock()
        .map(|map| map.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    for label in labels {
        closed(app, &label, "cancelled");
    }
}
pub async fn migrate(
    app: &AppHandle,
    runtime: &Runtime,
    package: Package,
    snapshot: PrivateSnapshot,
) -> AppResult<PrivateSnapshot> {
    if package.manifest.migration.is_none() {
        return Err(AppError::new(
            "PLUGIN_MIGRATION_REQUIRED",
            "新版存档结构改变，但开发者未提供迁移程序；原插件和存档保留",
        ));
    }
    let label = Host::token(runtime, "migration-")?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    let i = Arc::new(Instance {
        label: label.clone(),
        package,
        run_id: None,
        cancel: Default::default(),
        migration: Some(Migration {
            snapshot,
            result: Mutex::new(Some(tx)),
        }),
        network_slots: Arc::new(tokio::sync::Semaphore::new(0)),
    });
    app.state::<Host>().insert(i.clone())?;
    let result = if let Err(e) = crate::platform::windows::game_host::create(app, runtime, &i, true)
    {
        Err(e)
    } else {
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(Ok(v)) => v,
            _ => Err(AppError::new(
                "PLUGIN_MIGRATION_FAILED",
                "存档迁移失败或超时；原插件和存档保留",
            )),
        }
    };
    closed(app, &label, "error");
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.close();
    }
    result
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    key: String,
    value: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Complete {
    outcome: String,
    details: Option<Value>,
}
fn args<T: serde::de::DeserializeOwned>(v: Value) -> AppResult<T> {
    serde_json::from_value(v).map_err(|_| AppError::validation("args", "插件调用参数不符合协议"))
}
fn value(v: impl Serialize) -> AppResult<Value> {
    serde_json::to_value(v).map_err(|_| AppError::storage())
}
pub async fn call(
    app: AppHandle,
    runtime: Runtime,
    label: String,
    method: String,
    input: Value,
) -> AppResult<Value> {
    let i = app.state::<Host>().instance(&label)?;
    i.cancel.check()?;
    if [
        "game.ready",
        "game.getContext",
        "game.random",
        "game.close",
        "wallet.getBalance",
        "migration.input",
        "migration.failure",
    ]
    .contains(&method.as_str())
        && !input.as_object().is_some_and(|o| o.is_empty())
    {
        return Err(AppError::validation("args", "此接口不接受附加参数"));
    }
    let size = serde_json::to_vec(&input)
        .map_err(|_| AppError::storage())?
        .len();
    let max = if i.migration.is_some() {
        12 * 1024 * 1024
    } else {
        plugins::manifest::MAX_VALUE + 4096
    };
    if size > max {
        return Err(AppError::validation("args", "插件请求超过容量限制"));
    }
    if let Some(m) = &i.migration {
        return match method.as_str() {
            "migration.input" => Ok(
                json!({"snapshot":m.snapshot,"target_version":i.package.manifest.storage_version}),
            ),
            "migration.result" => {
                let s: PrivateSnapshot = args(input)?;
                plugins::validate_snapshot(&i.package.manifest, &s)?;
                let sender = m
                    .result
                    .lock()
                    .map_err(|_| AppError::storage())?
                    .take()
                    .ok_or_else(plugins::denied)?;
                sender.send(Ok(s)).map_err(|_| plugins::denied())?;
                Ok(Value::Null)
            }
            "migration.failure" => {
                if let Some(sender) = m.result.lock().map_err(|_| AppError::storage())?.take() {
                    let _ = sender.send(Err(AppError::new(
                        "PLUGIN_MIGRATION_FAILED",
                        "插件开发者提供的迁移程序执行失败；原数据保留",
                    )));
                }
                Ok(Value::Null)
            }
            _ => Err(plugins::denied()),
        };
    }
    if method == "network.fetch" {
        return network::fetch(&runtime, i.clone(), args(input)?).await;
    }
    if method == "game.close" {
        closed(&app, &i.label, "cancelled");
        if let Some(w) = app.get_webview_window(&i.label) {
            w.close().map_err(|_| AppError::storage())?;
        }
        return Ok(Value::Null);
    }
    let worker = runtime.clone();
    let original_method = method.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        worker.read(|s, _| {
            i.cancel.check()?;
            let m = &i.package.manifest;
            let id = &m.id;
            let version = &m.version;
            let run = i.run_id.as_deref().ok_or_else(plugins::denied)?;
            // 已记账的幂等查证由 change_coins 自己处理；其他调用先核对当前启用版本。
            if method != "wallet.changeCoins" {
                plugins::authorize(&s.connection, id, version)?;
            }
            match method.as_str() {
                "game.ready" => Ok(Value::Null),
                "game.getContext" => value(Context {
                    language: "zh-CN",
                    game_run_id: run.into(),
                    wallet_permission_granted: plugins::runs::get_run(&s.connection, run)?
                        .wallet_permission_granted,
                    plugin_version: version.clone(),
                    permissions: plugins::permissions(&s.connection, id)?,
                    settings: plugins::merged_settings(&s.connection, m)?,
                }),
                "game.random" => {
                    let n = worker
                        .random
                        .lock()
                        .map_err(|_| AppError::storage())?
                        .below(1u64 << 53)?;
                    Ok(json!((n as f64) / (1u64 << 53) as f64))
                }
                "game.complete" => {
                    let a: Complete = args(input)?;
                    value(plugins::runs::complete(
                        &s.connection,
                        id,
                        version,
                        run,
                        &a.outcome,
                        a.details,
                        worker.clock.sample()?.utc,
                    )?)
                }
                "storage.get" => {
                    let a: Key = args(input)?;
                    value(plugins::data_get(&s.connection, id, version, &a.key)?)
                }
                "storage.set" => {
                    let a: Set = args(input)?;
                    plugins::data_set(
                        &mut s.connection,
                        id,
                        version,
                        &a.key,
                        &a.value,
                        worker.clock.sample()?.utc,
                    )?;
                    Ok(Value::Null)
                }
                "storage.delete" => {
                    let a: Key = args(input)?;
                    plugins::data_delete(&s.connection, id, version, &a.key)?;
                    Ok(Value::Null)
                }
                "wallet.getBalance" => value(crate::currency::get_wallet(&s.connection)?),
                "wallet.changeCoins" => value(plugins::runs::change_coins(
                    &mut s.connection,
                    id,
                    version,
                    run,
                    &args(input)?,
                    worker.clock.sample()?.utc,
                )?),
                _ => Err(AppError::new(
                    "PLUGIN_METHOD_DENIED",
                    "插件接口不存在或未开放",
                )),
            }
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    if original_method == "wallet.changeCoins"
        && result.get("duplicate") == Some(&Value::Bool(false))
    {
        let _ = app.emit(crate::events::WALLET_CHANGED, ());
    }
    if matches!(
        original_method.as_str(),
        "game.complete" | "storage.set" | "storage.delete"
    ) {
        let _ = app.emit("game_plugins_changed", ());
    }
    Ok(result)
}
/// 每份资源都按浏览器传入的可信 webview 标签匹配；路径中的实例名不作为授权身份。
pub fn resource(app: &AppHandle, label: &str, path: &str) -> AppResult<(String, Vec<u8>)> {
    let i = app.state::<Host>().instance(label)?;
    i.cancel.check()?;
    let prefix = format!("/{label}/");
    let p = path.strip_prefix(&prefix).ok_or_else(plugins::denied)?;
    if i.migration.is_some() {
        if p == "__migration.html" {
            return Ok((
                "text/html; charset=utf-8".into(),
                b"<!doctype html><script type=\"module\" src=\"__migration.js\"></script>".to_vec(),
            ));
        }
        if p == "__migration.js" {
            let module = i
                .package
                .manifest
                .migration
                .as_ref()
                .ok_or_else(plugins::denied)?;
            let code=format!("const call=(method,args)=>window.__TAURI_INTERNALS__.invoke('game_plugin_call',{{method,args}});try{{const input=await call('migration.input',{{}});const {{migrate}}=await import({});const result=await migrate(input.snapshot,input.target_version);await call('migration.result',result);}}catch{{await call('migration.failure',{{}});}}",serde_json::to_string(&format!("./{module}")).map_err(|_|AppError::storage())?);
            return Ok(("text/javascript; charset=utf-8".into(), code.into_bytes()));
        }
    }
    plugins::manifest::path(p)?;
    let data = i.package.files.get(p).ok_or_else(AppError::not_found)?;
    Ok((
        plugins::package::mime(p)
            .ok_or_else(plugins::denied)?
            .into(),
        data.clone(),
    ))
}
pub fn csp(label: &str) -> String {
    format!("default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; media-src 'self'; connect-src 'self' ipc: http://ipc.localhost; frame-src 'none'; worker-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; navigate-to http://yanmew-game.localhost/{label}/")
}
pub(crate) fn expired(created: Instant) -> bool {
    created.elapsed() > Duration::from_secs(600)
}
pub fn command_gate<F>(
    handler: F,
) -> impl Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static
where
    F: Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync + 'static,
{
    move |invoke| {
        if !Host::command_allowed(
            invoke.message.webview_ref().label(),
            invoke.message.command(),
        ) {
            invoke.resolver.reject(AppError::new(
                "PLUGIN_METHOD_DENIED",
                "插件不能调用主程序命令",
            ));
            true
        } else {
            handler(invoke)
        }
    }
}
