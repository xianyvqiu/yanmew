//! 受限插件窗口；不授予主程序 capabilities，不加载颜喵前端或主题。
use crate::{
    error::{AppError, AppResult},
    game_host::{self, Instance},
    runtime::Runtime,
};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};
pub fn create(app: &AppHandle, runtime: &Runtime, i: &Instance, hidden: bool) -> AppResult<()> {
    let entry = if hidden {
        "__migration.html"
    } else {
        &i.package.manifest.entry
    };
    let prefix = format!("/{}/", i.label);
    let url = url::Url::parse(&format!("yanmew-game://localhost/{}/{entry}", i.label))
        .map_err(|_| AppError::storage())?;
    let native_profile = runtime
        .paths
        .as_ref()
        .map_err(Clone::clone)?
        .data_root
        .join("plugins/webview-profiles")
        .join(&i.label);
    std::fs::create_dir_all(&native_profile).map_err(|_| AppError::storage())?;
    let script = include_str!("../../../../plugins/sdk/bridge.js");
    let builder = WebviewWindowBuilder::new(app, &i.label, WebviewUrl::CustomProtocol(url))
        .title(&i.package.manifest.name)
        .inner_size(800.0, 600.0)
        .min_inner_size(320.0, 240.0)
        .resizable(true)
        .visible(!hidden)
        .data_directory(native_profile)
        .incognito(true)
        .devtools(false)
        .on_navigation(move |u| {
            (u.scheme() == "yanmew-game" && u.host_str() == Some("localhost")
                || u.scheme() == "http" && u.host_str() == Some("yanmew-game.localhost"))
                && u.path().starts_with(&prefix)
        })
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny);
    let window = if hidden {
        builder
    } else {
        builder.initialization_script(script)
    }
    .build()
    .map_err(|_| AppError::new("PLUGIN_WINDOW_FAILED", "无法创建插件窗口"))?;
    let profile_root =
        crate::game_plugins::package::root(runtime.paths.as_ref().map_err(Clone::clone)?);
    let owner = app.clone();
    let label = i.label.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            game_host::closed(&owner, &label, "cancelled");
        }
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let root = profile_root.clone();
            let label = label.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _ = crate::game_plugins::package::cleanup_profile(&root, &label);
            });
        }
    });
    Ok(())
}
