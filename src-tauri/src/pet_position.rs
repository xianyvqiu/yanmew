//! 主宠窗物理坐标适配；系统恢复/校正不覆盖用户保存的首选位置。
use crate::{
    error::{AppError, AppResult},
    runtime::Runtime,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent,
};

type Point = (i32, i32);
const SAVE_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    pub(crate) fn intersection(self, point: Point, size: PhysicalSize<u32>) -> i64 {
        let width = (i64::from(self.x) + i64::from(self.width))
            .min(i64::from(point.0) + i64::from(size.width))
            - i64::from(self.x).max(i64::from(point.0));
        let height = (i64::from(self.y) + i64::from(self.height))
            .min(i64::from(point.1) + i64::from(size.height))
            - i64::from(self.y).max(i64::from(point.1));
        width.max(0).saturating_mul(height.max(0))
    }
    pub(crate) fn fit(self, point: Point, size: PhysicalSize<u32>) -> Point {
        // 超小工作区放在左上角，普通尺寸则使整个窗口留在工作区内。
        let x = i64::from(point.0).clamp(
            i64::from(self.x),
            i64::from(self.x) + i64::from(self.width.saturating_sub(size.width)),
        );
        let y = i64::from(point.1).clamp(
            i64::from(self.y),
            i64::from(self.y) + i64::from(self.height.saturating_sub(size.height)),
        );
        (
            x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        )
    }
    fn bottom_right(self, size: PhysicalSize<u32>) -> Point {
        self.fit((i32::MAX, i32::MAX), size)
    }
}

fn choose_position(
    saved: Option<Point>,
    size: PhysicalSize<u32>,
    areas: &[Area],
    fallback: usize,
) -> AppResult<Point> {
    let default = areas.get(fallback).ok_or_else(window_error)?;
    if let Some(point) = saved {
        if let Some(area) = areas
            .iter()
            .max_by_key(|area| area.intersection(point, size))
            .filter(|area| area.intersection(point, size) > 0)
        {
            return Ok(area.fit(point, size));
        }
    }
    Ok(default.bottom_right(size))
}

fn window_error() -> AppError {
    AppError::new(
        "WINDOW_POSITION_ERROR",
        "无法恢复宠窗位置，请检查显示器状态后重新显示宠窗",
    )
}

pub(crate) fn areas(window: &WebviewWindow) -> AppResult<(Vec<Area>, usize)> {
    let monitors = window.available_monitors().map_err(|_| window_error())?;
    let current = window
        .current_monitor()
        .map_err(|_| window_error())?
        .or(window.primary_monitor().map_err(|_| window_error())?);
    let selected = current.as_ref().map(|m| m.work_area().position);
    let mut fallback = 0;
    let mut areas = Vec::new();
    for m in &monitors {
        let area = m.work_area();
        if area.size.width == 0 || area.size.height == 0 {
            continue;
        }
        if selected == Some(area.position) {
            fallback = areas.len();
        }
        areas.push(Area {
            x: area.position.x,
            y: area.position.y,
            width: area.size.width,
            height: area.size.height,
        });
    }
    if areas.is_empty() {
        return Err(window_error());
    }
    Ok((areas, fallback))
}

#[derive(Default)]
struct Pending {
    drag_origin: Option<Point>,
    position: Option<Point>,
    timer_running: bool,
    stopping: bool,
}
impl Pending {
    fn moved(&mut self, position: Point) -> bool {
        if self.drag_origin.is_none() || self.stopping {
            return false;
        }
        if self.drag_origin != Some(position) || self.position.is_some() {
            self.position = Some(position);
        }
        true
    }
    fn finish(&mut self, position: Option<Point>) {
        if let Some(origin) = self.drag_origin.take() {
            if let Some(point) = position {
                if point != origin || self.position.is_some() {
                    self.position = Some(point);
                }
            }
        }
    }
    fn acknowledge(&mut self, position: Point) {
        if self.position == Some(position) {
            self.position = None;
        }
    }
}

#[derive(Default)]
struct Inner {
    pending: Mutex<Pending>,
    // 唯一写入顺序，旧工作器不能在退出补存之后写回旧坐标。
    writing: Mutex<()>,
}
#[derive(Clone, Default)]
pub struct WindowPosition(Arc<Inner>);

impl WindowPosition {
    fn persist(&self, runtime: &Runtime) -> AppResult<()> {
        let _writer = self.0.writing.lock().map_err(|_| AppError::storage())?;
        let point = self
            .0
            .pending
            .lock()
            .map_err(|_| AppError::storage())?
            .position;
        if let Some(point) = point {
            // 不在持有位置状态锁时等待 SQLite，窗口移动事件不被磁盘写入阻塞。
            runtime.read(|store, _| {
                crate::config::save_window_position(&mut store.connection, point)
            })?;
            self.0
                .pending
                .lock()
                .map_err(|_| AppError::storage())?
                .acknowledge(point);
        }
        Ok(())
    }

    fn schedule(&self, app: &AppHandle, runtime: Runtime) -> AppResult<()> {
        let mut pending = self.0.pending.lock().map_err(|_| AppError::storage())?;
        if pending.timer_running || pending.stopping || pending.position.is_none() {
            return Ok(());
        }
        pending.timer_running = true;
        drop(pending);
        let state = self.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(SAVE_INTERVAL).await;
                let worker = state.clone();
                let runtime = runtime.clone();
                let result = tauri::async_runtime::spawn_blocking(move || worker.persist(&runtime))
                    .await
                    .unwrap_or_else(|_| Err(AppError::storage()));
                if let Err(error) = &result {
                    report(&app, error);
                }
                let Ok(mut pending) = state.0.pending.lock() else {
                    break;
                };
                if result.is_err() || pending.position.is_none() || pending.stopping {
                    pending.timer_running = false;
                    break;
                }
            }
        });
        Ok(())
    }

    pub fn begin_drag(&self, window: &WebviewWindow) -> AppResult<()> {
        let position = window.outer_position().map_err(|_| window_error())?;
        self.0
            .pending
            .lock()
            .map_err(|_| AppError::storage())?
            .drag_origin = Some((position.x, position.y));
        Ok(())
    }

    pub async fn finish_drag(&self, window: &WebviewWindow, runtime: Runtime) -> AppResult<()> {
        let position = window
            .is_minimized()
            .and_then(|minimized| {
                if minimized {
                    Ok(None)
                } else {
                    window.outer_position().map(Some)
                }
            })
            .map_err(|_| window_error());
        self.0
            .pending
            .lock()
            .map_err(|_| AppError::storage())?
            .finish(
                position
                    .as_ref()
                    .ok()
                    .and_then(|p| p.as_ref())
                    .map(|p| (p.x, p.y)),
            );
        let worker = self.clone();
        let saved = tauri::async_runtime::spawn_blocking(move || worker.persist(&runtime))
            .await
            .map_err(|_| AppError::storage())?;
        position?;
        saved
    }

    pub fn flush_exit(&self, runtime: &Runtime) -> AppResult<()> {
        self.0
            .pending
            .lock()
            .map_err(|_| AppError::storage())?
            .stopping = true;
        self.persist(runtime)
    }

    pub fn handle(
        &self,
        window: &WebviewWindow,
        event: &WindowEvent,
        runtime: &Runtime,
    ) -> AppResult<()> {
        match event {
            WindowEvent::Moved(position) => {
                if window.is_minimized().map_err(|_| window_error())? {
                    return Ok(());
                }
                let user_move = self
                    .0
                    .pending
                    .lock()
                    .map_err(|_| AppError::storage())?
                    .moved((position.x, position.y));
                if user_move {
                    self.schedule(window.app_handle(), runtime.clone())?;
                } else {
                    self.ensure_visible(window, false)?;
                }
            }
            WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::Focused(true) => {
                self.ensure_visible(window, true)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn restore(&self, window: &WebviewWindow, runtime: &Runtime) -> AppResult<()> {
        let saved = runtime.read(|store, _| crate::config::get_window_position(&store.connection));
        let (areas, fallback) = areas(window)?;
        let size = window.outer_size().map_err(|_| window_error())?;
        let point = choose_position(
            saved.as_ref().ok().copied().flatten(),
            size,
            &areas,
            fallback,
        )?;
        window
            .set_position(PhysicalPosition::new(point.0, point.1))
            .map_err(|_| window_error())?;
        // 底层数据不可读时仍先放到可见默认位置，再报告存储错误。
        saved.map(|_| ())
    }

    pub fn ensure_visible(&self, window: &WebviewWindow, fit_entire_window: bool) -> AppResult<()> {
        if window.is_minimized().map_err(|_| window_error())? {
            return Ok(());
        }
        if self
            .0
            .pending
            .lock()
            .map_err(|_| AppError::storage())?
            .drag_origin
            .is_some()
        {
            return Ok(());
        }
        let current = window.outer_position().map_err(|_| window_error())?;
        let size = window.outer_size().map_err(|_| window_error())?;
        let (areas, fallback) = areas(window)?;
        let point = (current.x, current.y);
        if !fit_entire_window && areas.iter().any(|a| a.intersection(point, size) > 0) {
            return Ok(());
        }
        let target = choose_position(Some(point), size, &areas, fallback)?;
        if point != target {
            window
                .set_position(PhysicalPosition::new(target.0, target.1))
                .map_err(|_| window_error())?;
        }
        if fit_entire_window {
            window.state::<crate::pet_desktop::Desktop>().realign_dock(window)?;
        }
        Ok(())
    }
}

pub fn report(app: &AppHandle, error: &AppError) {
    eprintln!("宠窗位置更新失败：{error}");
    let message = AppError::new(
        error.code,
        format!("宠窗位置恢复或保存失败：{}", error.message),
    );
    let _ = app.emit(crate::events::PET_ACTIVITY_ERROR, message);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn write_failure_keeps_latest_pair_for_exit_retry_without_saving_system_corrections() {
        let runtime = crate::runtime::test_runtime("http://127.0.0.1:1");
        runtime
            .read(|s, _| crate::config::save_window_position(&mut s.connection, (-1400, -100)))
            .unwrap();
        let state = WindowPosition::default();
        // 系统回位没有用户拖动标记，退出也不得把默认位置写回。
        assert!(!state.0.pending.lock().unwrap().moved((1600, 800)));
        state.persist(&runtime).unwrap();
        assert_eq!(
            runtime
                .read(|s, _| crate::config::get_window_position(&s.connection))
                .unwrap(),
            Some((-1400, -100))
        );
        runtime.read(|s,_| {
            s.connection.execute_batch("CREATE TRIGGER reject_position BEFORE UPDATE ON settings WHEN NEW.key='window_pos_y' BEGIN SELECT RAISE(ABORT,'test failure'); END;")?;
            Ok(())
        }).unwrap();
        {
            let mut pending = state.0.pending.lock().unwrap();
            pending.drag_origin = Some((1600, 800));
            pending.moved((-1200, 100));
        }
        assert!(state.persist(&runtime).is_err());
        assert_eq!(state.0.pending.lock().unwrap().position, Some((-1200, 100)));
        assert_eq!(
            runtime
                .read(|s, _| crate::config::get_window_position(&s.connection))
                .unwrap(),
            Some((-1400, -100))
        );
        runtime
            .read(|s, _| {
                s.connection
                    .execute_batch("DROP TRIGGER reject_position;")?;
                Ok(())
            })
            .unwrap();
        state.0.pending.lock().unwrap().moved((-1000, 200));
        state.flush_exit(&runtime).unwrap();
        assert_eq!(
            runtime
                .read(|s, _| crate::config::get_window_position(&s.connection))
                .unwrap(),
            Some((-1000, 200))
        );
        assert!(!state.0.pending.lock().unwrap().moved((-900, 300)));
        state.persist(&runtime).unwrap();
        assert_eq!(
            runtime
                .read(|s, _| crate::config::get_window_position(&s.connection))
                .unwrap(),
            Some((-1000, 200))
        );
    }
    fn area(x: i32, y: i32, width: u32, height: u32) -> Area {
        Area {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn initial_position_uses_work_area_and_real_physical_window_size() {
        let areas = [area(0, 40, 1920, 1000)];
        assert_eq!(
            choose_position(None, PhysicalSize::new(320, 240), &areas, 0).unwrap(),
            (1600, 800)
        );
        assert_eq!(
            choose_position(None, PhysicalSize::new(480, 360), &areas, 0).unwrap(),
            (1440, 680)
        );
    }
    #[test]
    fn negative_monitor_restores_and_partial_offscreen_is_clamped() {
        let areas = [area(0, 0, 1920, 1040), area(-1600, -200, 1600, 900)];
        let size = PhysicalSize::new(320, 240);
        assert_eq!(
            choose_position(Some((-1400, -100)), size, &areas, 0).unwrap(),
            (-1400, -100)
        );
        assert_eq!(
            choose_position(Some((-1550, -250)), size, &areas, 0).unwrap(),
            (-1550, -200)
        );
        assert_eq!(
            choose_position(Some((1800, 1000)), size, &areas, 0).unwrap(),
            (1600, 800)
        );
    }
    #[test]
    fn removed_monitor_uses_current_work_area_and_tiny_areas_do_not_overflow() {
        let areas = [area(0, 0, 1920, 1040), area(1920, 0, 1280, 900)];
        assert_eq!(
            choose_position(Some((-1500, 100)), PhysicalSize::new(320, 240), &areas, 1).unwrap(),
            (2880, 660)
        );
        assert_eq!(
            choose_position(
                None,
                PhysicalSize::new(500, 400),
                &[area(-10, -20, 100, 80)],
                0
            )
            .unwrap(),
            (-10, -20)
        );
        assert!(choose_position(None, PhysicalSize::new(1, 1), &[], 0).is_err());
        assert_eq!(
            choose_position(
                Some((i32::MAX, 0)),
                PhysicalSize::new(640, 480),
                &[area(i32::MAX - 100, 0, 1000, 1000)],
                0
            )
            .unwrap(),
            (i32::MAX, 0)
        );
    }
    #[test]
    fn only_user_moves_dirty_the_pair_and_latest_position_survives_old_acknowledgement() {
        let mut state = Pending::default();
        assert!(!state.moved((10, 20)));
        assert_eq!(state.position, None);
        state.drag_origin = Some((10, 20));
        assert!(state.moved((30, 40)));
        state.moved((50, 60));
        state.acknowledge((30, 40));
        assert_eq!(state.position, Some((50, 60)));
        state.finish(Some((70, 80)));
        assert_eq!(state.position, Some((70, 80)));
        state.acknowledge((70, 80));
        assert_eq!(state.position, None);
        assert!(!state.moved((90, 100)));
        state.drag_origin = Some((90, 100));
        state.finish(Some((90, 100)));
        assert_eq!(state.position, None);
        state.stopping = true;
        state.drag_origin = Some((90, 100));
        assert!(!state.moved((110, 120)));
    }
}
