use crate::pet_region::{self, Rect};

use crate::{
    error::{parse_id, AppError, AppResult},
    runtime::{Runtime, Store},
};
use tauri::{State, WebviewWindow};

#[tauri::command]
pub async fn generate_expressions(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<crate::expressions::ExpressionLibrary> {
    authorize(&window)?;
    let result =
        crate::expressions::generate_persona_expressions(runtime.inner().clone(), parse_id(&id)?)
            .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(result)
}
#[tauri::command]
pub async fn cancel_expression_generation(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    runtime.requests.cancel_key(&format!("expressions:{id}"));
    Ok(())
}

#[tauri::command]
pub async fn get_current_context(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<crate::persona::Context> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::persona::context(&s.connection, s.context_revision)
    })
    .await
}

#[tauri::command]
pub async fn get_weather_settings(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<crate::config::WeatherSettings> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::config::get_weather(&s.connection)
    })
    .await
}
#[tauri::command]
pub async fn save_weather_settings(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    input: crate::config::WeatherInput,
) -> AppResult<crate::config::WeatherSettings> {
    authorize(&window)?;
    storage(runtime.inner().clone(), move |s, p| {
        crate::config::save_weather(&mut s.connection, input, p)
    })
    .await
}

#[tauri::command]
pub async fn test_provider_connection(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let config = storage(runtime.inner().clone(), move |s, p| {
        crate::providers::connection_test_config(&s.connection, id, p)
    })
    .await?;
    crate::providers::test_connection(&runtime.ai.clone()?, config).await
}

fn authorize(window: &WebviewWindow) -> AppResult<()> {
    if window.label() != "main" {
        return Err(AppError::new("PERMISSION_DENIED", "此窗口无权管理应用配置"));
    }
    Ok(())
}
async fn storage<T: Send + 'static>(
    runtime: Runtime,
    action: impl FnOnce(&mut Store, &dyn crate::platform::contracts::SecretProtector) -> AppResult<T>
        + Send
        + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || runtime.read(action))
        .await
        .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn get_configuration_status(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<()> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |_, _| Ok(())).await
}
#[tauri::command]
pub async fn get_settings(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<crate::config::Settings> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::config::get(&s.connection)
    })
    .await
}
#[tauri::command]
pub async fn save_settings(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    changes: crate::config::Settings,
) -> AppResult<crate::config::Settings> {
    authorize(&window)?;
    let settings = storage(runtime.inner().clone(), move |s, _| {
        crate::config::save(&mut s.connection, changes)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(settings)
}

#[tauri::command]
pub async fn list_providers(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Vec<crate::providers::Provider>> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::providers::list(&s.connection)
    })
    .await
}
#[tauri::command]
pub async fn create_provider(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    input: crate::providers::ProviderInput,
) -> AppResult<crate::providers::Provider> {
    authorize(&window)?;
    storage(runtime.inner().clone(), move |s, p| {
        crate::providers::save(&mut s.connection, None, input, p)
    })
    .await
}
#[tauri::command]
pub async fn update_provider(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: String,
    input: crate::providers::ProviderInput,
) -> AppResult<crate::providers::Provider> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let requests = runtime.requests.clone();
    storage(runtime.inner().clone(), move |s, p| {
        let result = crate::providers::save(&mut s.connection, Some(id), input, p)?;
        if result.is_active {
            requests.cancel_all();
        }
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn list_personas(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Vec<crate::persona::Persona>> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::persona::list(&s.connection)
    })
    .await
}
#[tauri::command]
pub async fn create_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    input: crate::persona::PersonaInput,
) -> AppResult<crate::persona::Persona> {
    authorize(&window)?;
    let persona = storage(runtime.inner().clone(), move |s, _| {
        crate::persona::save(&mut s.connection, None, input)
    })
    .await?;
    let id = parse_id(&persona.id)?;
    let available = storage(runtime.inner().clone(), |s, _| {
        Ok(s.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM providers WHERE is_active=1)",
            [],
            |r| r.get::<_, bool>(0),
        )?)
    })
    .await;
    let available = match available {
        Ok(value) => value,
        Err(error) => {
            let _ = app.emit(
                "expression_generation_failed",
                serde_json::json!({"personaId":id.to_string(),"error":error}),
            );
            false
        }
    };
    if available {
        let runtime = runtime.inner().clone();
        tauri::async_runtime::spawn(async move {
            match crate::expressions::generate_persona_expressions(runtime, id).await {
                Ok(_) => {
                    let _ = app.emit("configuration_changed", ());
                }
                Err(error) => {
                    let _ = app.emit(
                        "expression_generation_failed",
                        serde_json::json!({"personaId":id.to_string(),"error":error}),
                    );
                }
            }
        });
    }
    Ok(persona)
}
#[tauri::command]
pub async fn update_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
    input: crate::persona::PersonaInput,
) -> AppResult<crate::persona::Persona> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let requests = runtime.requests.clone();
    let persona = storage(runtime.inner().clone(), move |s, _| {
        let result = crate::persona::save(&mut s.connection, Some(id), input)?;
        requests.cancel_persona(id);
        Ok(result)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(persona)
}

#[tauri::command]
pub async fn delete_provider(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    storage(runtime.inner().clone(), move |s, _| {
        crate::providers::delete(&mut s.connection, id)
    })
    .await
}
#[tauri::command]
pub async fn set_active_chat_provider(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let requests = runtime.requests.clone();
    storage(runtime.inner().clone(), move |s, _| {
        let same = s.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM providers WHERE id=?1 AND is_active=1)",
            [id],
            |r| r.get::<_, bool>(0),
        )?;
        crate::providers::select(&mut s.connection, Some(id), false)?;
        if !same {
            requests.cancel_all();
        }
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn set_memory_provider(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: Option<String>,
) -> AppResult<()> {
    authorize(&window)?;
    let id = id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(), move |s, _| {
        crate::providers::select(&mut s.connection, id, true)
    })
    .await
}
async fn change_persona(
    app: AppHandle,
    runtime: Runtime,
    id: i64,
    delete: bool,
) -> AppResult<crate::persona::Context> {
    let requests = runtime.requests.clone();
    let (context, changed) = storage(runtime, move |s, _| {
        let before = crate::persona::context(&s.connection, s.context_revision)?;
        if delete {
            crate::persona::delete(&mut s.connection, id)?;
            requests.cancel_persona(id);
        } else {
            crate::persona::select(&mut s.connection, id)?;
        }
        let mut after = crate::persona::context(&s.connection, s.context_revision)?;
        let changed =
            before.persona_id != after.persona_id || before.session_id != after.session_id;
        if changed {
            requests.cancel_persona(parse_id(&before.persona_id)?);
            s.context_revision += 1;
            after.context_revision = s.context_revision;
        }
        Ok((after, changed))
    })
    .await?;
    // 事务已提交；窗口可通过列表重新获取权威当前项。
    if changed && app.emit("window_context_changed", &context).is_err() {
        eprintln!("上下文变化通知发送失败，请刷新窗口");
    }
    Ok(context)
}
#[tauri::command]
pub async fn set_active_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<crate::persona::Context> {
    authorize(&window)?;
    change_persona(app, runtime.inner().clone(), parse_id(&id)?, false).await
}
#[tauri::command]
pub async fn delete_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<crate::persona::Context> {
    authorize(&window)?;
    change_persona(app, runtime.inner().clone(), parse_id(&id)?, true).await
}

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, LogicalSize, Manager};

pub const STATE_EVENT: &str = "pet-window-state";

#[derive(Clone, Copy, Default, Serialize)]
pub struct Snapshot {
    pub hidden: bool,
    pub revision: u64,
}

#[derive(Default)]
pub struct PetState {
    snapshot: Snapshot,
    full_size: Option<LogicalSize<f64>>,
    region: Option<Vec<[i32; 4]>>,
}

impl PetState {
    fn target_size(&self, hidden: bool) -> LogicalSize<f64> {
        if hidden {
            LogicalSize::new(112.0, 44.0)
        } else {
            self.full_size.unwrap_or(LogicalSize::new(320.0, 240.0))
        }
    }

    fn commit_hidden(&mut self, hidden: bool, full_size: Option<LogicalSize<f64>>) {
        if self.snapshot.hidden != hidden {
            self.full_size = full_size;
            self.snapshot.hidden = hidden;
            self.snapshot.revision += 1;
            self.region = None;
        }
    }

    fn accepts_region(&self, revision: u64) -> bool {
        self.snapshot.revision == revision
    }
}

async fn on_main<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(AppHandle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(operation(handle));
    })
    .map_err(|e| format!("调度宠窗操作失败：{e}"))?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv().map_err(|_| "宠窗操作已中断".to_string())
    })
    .await
    .map_err(|e| format!("等待宠窗操作失败：{e}"))??
}

// 调用方必须位于窗口主线程；托盘和 command 共用这一条恢复路径。
pub fn change_hidden(app: &AppHandle, hidden: bool) -> Result<Snapshot, String> {
    let window = app.get_webview_window("main").ok_or("找不到 main 宠窗")?;
    let managed = app.state::<Mutex<PetState>>();
    let mut state = managed.lock().map_err(|_| "宠窗状态不可用")?;
    if state.snapshot.hidden != hidden {
        let full_size = if hidden {
            Some(
                window
                    .inner_size()
                    .map_err(|e| format!("读取宠窗尺寸失败：{e}"))?
                    .to_logical(
                        window
                            .scale_factor()
                            .map_err(|e| format!("读取屏幕缩放失败：{e}"))?,
                    ),
            )
        } else {
            state.full_size
        };
        // 先解除旧模式裁剪，确保恢复期间不会被旧区域永久遮挡。
        pet_region::apply(&window, None)?;
        state.region = None;
        window
            .set_size(state.target_size(hidden))
            .map_err(|e| format!("调整宠窗尺寸失败：{e}"))?;
        state.commit_hidden(hidden, full_size);
    } else if !hidden {
        pet_region::apply(&window, None)?;
        state.region = None;
        state.snapshot.revision += 1;
    }
    let snapshot = state.snapshot;
    drop(state);
    window
        .emit(STATE_EVENT, snapshot)
        .map_err(|e| format!("同步宠窗状态失败：{e}"))?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_pet_window_state(app: AppHandle) -> Result<Snapshot, String> {
    on_main(app, |app| {
        Ok(app
            .state::<Mutex<PetState>>()
            .lock()
            .map_err(|_| "宠窗状态不可用")?
            .snapshot)
    })
    .await
}

#[tauri::command]
pub async fn set_pet_hidden(app: AppHandle, hidden: bool) -> Result<Snapshot, String> {
    on_main(app, move |app| change_hidden(&app, hidden)).await
}

#[tauri::command]
pub async fn update_pet_regions(
    app: AppHandle,
    revision: u64,
    scale: f64,
    rects: Vec<Rect>,
) -> Result<(), String> {
    on_main(app, move |app| {
        let managed = app.state::<Mutex<PetState>>();
        let mut state = managed.lock().map_err(|_| "宠窗状态不可用")?;
        if !state.accepts_region(revision) {
            return Ok(());
        }
        let window = app.get_webview_window("main").ok_or("找不到 main 宠窗")?;
        let region = pet_region::window_rects(&window, &rects, scale)?;
        if state.region.as_ref() == Some(&region) {
            return Ok(());
        }
        pet_region::apply(&window, Some(&region))?;
        state.region = Some(region);
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_hide_preserves_full_size_and_restore_revision() {
        let mut state = PetState::default();
        let full = LogicalSize::new(400.0, 300.0);
        state.commit_hidden(true, Some(full));
        state.commit_hidden(true, Some(LogicalSize::new(112.0, 44.0)));
        assert_eq!(state.target_size(false), full);
        assert_eq!(state.snapshot.revision, 1);
        state.commit_hidden(false, state.full_size);
        assert!(!state.snapshot.hidden);
        assert_eq!(state.snapshot.revision, 2);
        state.commit_hidden(false, state.full_size);
        assert_eq!(state.snapshot.revision, 2);
    }
    #[test]
    fn old_regions_are_rejected_after_mode_changes() {
        let mut state = PetState {
            region: Some(vec![[1, 2, 3, 4]]),
            ..Default::default()
        };
        state.commit_hidden(true, Some(LogicalSize::new(320.0, 240.0)));
        assert!(!state.accepts_region(0));
        assert!(state.accepts_region(1));
        assert!(state.region.is_none());
        state.commit_hidden(false, state.full_size);
        assert!(!state.accepts_region(1));
        assert!(state.accepts_region(2));
    }
}
