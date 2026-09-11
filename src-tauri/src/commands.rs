use crate::pet_region::{self, Rect};
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
        let region = pet_region::physical_rects(&rects, scale)?;
        if state.region.as_ref() == Some(&region) {
            return Ok(());
        }
        let window = app.get_webview_window("main").ok_or("找不到 main 宠窗")?;
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
        let mut state = PetState::default();
        state.region = Some(vec![[1, 2, 3, 4]]);
        state.commit_hidden(true, Some(LogicalSize::new(320.0, 240.0)));
        assert!(!state.accepts_region(0));
        assert!(state.accepts_region(1));
        assert!(state.region.is_none());
        state.commit_hidden(false, state.full_size);
        assert!(!state.accepts_region(1));
        assert!(state.accepts_region(2));
    }
}
