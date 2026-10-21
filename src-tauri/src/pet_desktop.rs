//! 宠窗全屏游走与用户拖动停靠；复用位置适配、设置表和唯一 jobs 节奏。
use crate::{
    error::{AppError, AppResult},
    pet::{
        controller::View,
        wander::{Layout, Rect},
        RandomSource,
    },
    pet_position::{self, Area},
    runtime::Runtime,
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const KEY: &str = "pet_desktop_preferences";
const INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dock {
    #[default]
    None,
    PeekLeft,
    PeekRight,
    RestSit,
    RestLie,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Preferences {
    fullscreen_wander: bool,
    dock: Dock,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkDirection { Left, Right }
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Walking { pub direction: WalkDirection, pub persona_id: String, pub session_id: String, pub context_revision: u64 }
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub fullscreen_wander: bool,
    pub dock: Dock,
    pub revision: u64,
    pub walking: Option<Walking>,
}
#[derive(Default)]
struct Inner {
    preferences: Preferences,
    revision: u64,
    layout: Option<Layout>,
    dragging: bool,
    motion: Option<u64>,
    walking: Option<Walking>,
    last_check: Option<Duration>,
    target: Option<(i32, i32)>,
}
#[derive(Default)]
pub struct Desktop {
    inner: Mutex<Inner>,
    epoch: AtomicU64,
}
impl Inner {
    fn stop_motion(&mut self) { self.motion = None; if self.walking.take().is_some() { self.revision += 1; } }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            fullscreen_wander: self.preferences.fullscreen_wander,
            dock: self.preferences.dock,
            revision: self.revision,
            walking: self.walking.clone(),
        }
    }
    fn allowed(&self, view: &View) -> bool {
        self.preferences.fullscreen_wander
            && self.preferences.dock == Dock::None
            && !self.dragging
            && view.can_wander
            && self.layout.as_ref().is_some_and(|l| {
                !l.paused
                    && l.persona_id == view.persona_id
                    && view.session_id.as_deref() == Some(l.session_id.as_str())
                    && l.context_revision == view.context_revision
            })
    }
}
fn load(c: &Connection) -> AppResult<Preferences> {
    let raw: Option<String> = c
        .query_row("SELECT value FROM settings WHERE key=?1", [KEY], |r| {
            r.get(0)
        })
        .optional()?;
    raw.map(|s| {
        serde_json::from_str(&s).map_err(|_| {
            AppError::new(
                "DESKTOP_STATE_INVALID",
                "桌宠游走与停靠设置无效，请保留数据并检查应用版本",
            )
        })
    })
    .transpose()
    .map(|p| p.unwrap_or_default())
}
fn save(c: &Connection, p: &Preferences) -> AppResult<()> {
    let json = serde_json::to_string(p).map_err(|_| AppError::storage())?;
    c.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",rusqlite::params![KEY,json])?;
    Ok(())
}
impl Desktop {
    pub fn snapshot(&self) -> AppResult<Snapshot> {
        Ok(self
            .inner
            .lock()
            .map_err(|_| AppError::storage())?
            .snapshot())
    }
    pub fn update(&self, layout: Layout) -> AppResult<()> {
        layout.validate()?;
        let mut s = self.inner.lock().map_err(|_| AppError::storage())?;
        if s.layout
            .as_ref()
            .is_some_and(|old| old.layout_revision >= layout.layout_revision)
        {
            return Ok(());
        }
        let cancel = layout.paused || s.layout.as_ref().is_some_and(|old| old.persona_id != layout.persona_id || old.session_id != layout.session_id || old.context_revision != layout.context_revision || old.area.width != layout.area.width || old.area.height != layout.area.height);
        if cancel { self.epoch.fetch_add(1, Ordering::SeqCst); s.stop_motion(); }
        s.layout = Some(layout);
        Ok(())
    }
    pub fn begin_drag(&self) -> AppResult<()> {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let mut s = self.inner.lock().map_err(|_| AppError::storage())?;
        s.dragging = true;
        s.stop_motion();
        s.target = None;
        Ok(())
    }
    pub fn end_drag(&self) -> AppResult<()> {
        self.inner.lock().map_err(|_| AppError::storage())?.dragging = false;
        Ok(())
    }
    pub fn is_dragging(&self) -> AppResult<bool> {
        Ok(self.inner.lock().map_err(|_| AppError::storage())?.dragging)
    }
    /// 尺寸/DPI变化后保持既有停靠，不重新识别边缘或抽取动作；原生调用在锁外。
    pub fn realign_dock(&self, window: &WebviewWindow) -> AppResult<()> {
        let dock = {
            let s = self.inner.lock().map_err(|_| AppError::storage())?;
            if s.dragging {
                return Ok(());
            }
            s.preferences.dock
        };
        if dock == Dock::None || window.is_minimized().map_err(|_| AppError::storage())? {
            return Ok(());
        }
        let (point, size, area, _) = geometry(window)?;
        let target = snap(dock, point, size, area);
        if target != point {
            window.set_position(PhysicalPosition::new(target.0, target.1))
                .map_err(|_| AppError::storage())?;
        }
        Ok(())
    }
    pub fn initialize(&self, window: &WebviewWindow, runtime: &Runtime) -> AppResult<()> {
        let preferences = runtime.read(|s, _| load(&s.connection))?;
        let dock = preferences.dock;
        {
            let mut s=self.inner.lock().map_err(|_|AppError::storage())?;
            s.preferences=preferences;s.revision+=1;
        }
        if dock != Dock::None {
            let (point, size, area, _) = geometry(window)?;
            let target = snap(dock, point, size, area);
            window
                .set_position(PhysicalPosition::new(target.0, target.1))
                .map_err(|_| AppError::storage())?;
        }
        publish(window.app_handle(),&self.snapshot()?);
        Ok(())
    }
}
fn authorize(w: &WebviewWindow) -> AppResult<()> {
    if w.label() == "main" {
        Ok(())
    } else {
        Err(AppError::new(
            "PERMISSION_DENIED",
            "只有宠窗可以设置桌面游走与停靠",
        ))
    }
}
pub(crate) fn publish(app: &tauri::AppHandle, snapshot: &Snapshot) {
    let _ = app.emit_to("main", "pet-desktop-state", snapshot);
}
#[tauri::command]
pub fn get_pet_desktop_state(
    window: WebviewWindow,
    state: tauri::State<'_, Desktop>,
) -> AppResult<Snapshot> {
    authorize(&window)?;
    state.snapshot()
}
#[tauri::command]
pub async fn set_pet_fullscreen_wander(
    window: WebviewWindow,
    app: tauri::AppHandle,
    runtime: tauri::State<'_, Runtime>,
    enabled: bool,
) -> AppResult<Snapshot> {
    authorize(&window)?;
    let rt = runtime.inner().clone();
    let handle = app.clone();
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        rt.read(|store, _| {
            let managed = handle.state::<Desktop>();
            let preferences = Preferences {
                fullscreen_wander: enabled,
                ..managed
                    .inner
                    .lock()
                    .map_err(|_| AppError::storage())?
                    .preferences
                    .clone()
            };
            save(&store.connection, &preferences)?;
            let mut s = managed.inner.lock().map_err(|_| AppError::storage())?;
            managed.epoch.fetch_add(1, Ordering::SeqCst);
            s.preferences = preferences;
            s.stop_motion();
            s.last_check = None;
            s.revision += 1;
            Ok(s.snapshot())
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    publish(&app, &snapshot);
    Ok(snapshot)
}
#[tauri::command]
pub async fn get_pet_dock_expression(
    window: WebviewWindow,
    runtime: tauri::State<'_, Runtime>,
    persona_id: String,
    dock: Dock,
) -> AppResult<String> {
    authorize(&window)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| {
            let persona = crate::error::parse_id(&persona_id)?;
            let category = match dock {
                Dock::PeekLeft => "peek_left",
                Dock::PeekRight => "peek_right",
                Dock::RestSit => "rest_sit",
                Dock::RestLie => "rest_lie",
                Dock::None => return Ok(String::new()),
            };
            let mut random = rt.random.lock().map_err(|_| AppError::storage())?;
            if let Some(image) = crate::pet_creation::select(
                &s.connection,
                persona,
                category,
                None,
                random.as_mut(),
            )? {
                return Ok(image);
            }
            crate::expressions::choose_local(&s.connection, persona, category, random.as_mut())
        })
    })
    .await
    .map_err(|_| AppError::storage())?
}

#[tauri::command]
pub async fn get_pet_walk_expression(window: WebviewWindow, runtime: tauri::State<'_, Runtime>, persona_id: String, direction: WalkDirection) -> AppResult<String> {
    authorize(&window)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rt.read(|s,_| {
        let category=match direction { WalkDirection::Left => "walk_left", WalkDirection::Right => "walk_right" };
        let mut random=rt.random.lock().map_err(|_|AppError::storage())?;
        let id=crate::error::parse_id(&persona_id)?;
        match crate::pet_creation::select(&s.connection,id,category,None,random.as_mut())? {
            Some(image) => Ok(image), None => crate::expressions::choose_local(&s.connection,id,category,random.as_mut()),
        }
    })).await.map_err(|_|AppError::storage())?
}

#[tauri::command]
pub async fn get_pet_lift_expression(window: WebviewWindow, runtime: tauri::State<'_, Runtime>, persona_id: String) -> AppResult<String> {
    authorize(&window)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rt.read(|s,_| {
        let category="lifted";
        let mut random=rt.random.lock().map_err(|_|AppError::storage())?;
        let id=crate::error::parse_id(&persona_id)?;
        match crate::pet_creation::select(&s.connection,id,category,None,random.as_mut())? {
            Some(image) => Ok(image), None => crate::expressions::choose_local(&s.connection,id,category,random.as_mut()),
        }
    })).await.map_err(|_|AppError::storage())?
}

type Geometry = ((i32, i32), PhysicalSize<u32>, Area, f64);
fn geometry(window: &WebviewWindow) -> AppResult<Geometry> {
    let p = window.outer_position().map_err(|_| AppError::storage())?;
    let size = window.outer_size().map_err(|_| AppError::storage())?;
    let (areas, fallback) = pet_position::areas(window)?;
    let area = *areas
        .iter()
        .max_by_key(|a| a.intersection((p.x, p.y), size))
        .filter(|a| a.intersection((p.x, p.y), size) > 0)
        .unwrap_or(&areas[fallback]);
    let scale = window.scale_factor().map_err(|_| AppError::storage())?;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(AppError::storage());
    }
    Ok(((p.x, p.y), size, area, scale))
}
fn snap(dock: Dock, point: (i32, i32), size: PhysicalSize<u32>, area: Area) -> (i32, i32) {
    let mut p = area.fit(point, size);
    match dock {
        Dock::PeekLeft => p.0 = area.x,
        Dock::PeekRight => p.0 = area.fit((i32::MAX, p.1), size).0,
        Dock::RestSit | Dock::RestLie => p.1 = area.fit((p.0, i32::MAX), size).1,
        Dock::None => {}
    }
    p
}
fn detect(
    point: (i32, i32),
    body: Rect,
    area: Area,
    scale: f64,
    random: &mut dyn RandomSource,
) -> AppResult<Dock> {
    let left = f64::from(point.0) + body.x * scale;
    let right = left + body.width * scale;
    let top = f64::from(point.1) + body.y * scale;
    let bottom = top + body.height * scale;
    let threshold = 24.0 * scale;
    let side = if left <= f64::from(area.x) + threshold {
        Some(Dock::PeekLeft)
    } else if right >= f64::from(area.x) + f64::from(area.width) - threshold
    {
        Some(Dock::PeekRight)
    } else {
        None
    };
    let floor = bottom >= f64::from(area.y) + f64::from(area.height) - threshold;
    if floor && (side.is_none() || random.below(2)? == 1) {
        return Ok(if random.below(2)? == 0 {
            Dock::RestSit
        } else {
            Dock::RestLie
        });
    }
    Ok(side.unwrap_or(Dock::None))
}

/// 必须在既有 WindowPosition::finish_drag 之前调用，最终保存的是吸附后的窗口位置。
pub async fn finish_drag(window: &WebviewWindow, runtime: Runtime) -> AppResult<()> {
    let (point, size, area, scale) = geometry(window)?;
    let app = window.app_handle().clone();
    let handle = app.clone();
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        runtime.read(|store, _| {
            let managed = handle.state::<Desktop>();
            let (body, mut preferences) = {
                let s = managed.inner.lock().map_err(|_| AppError::storage())?;
                (
                    s.layout
                        .as_ref()
                        .map(|l| l.anchor.unwrap_or(l.body))
                        .unwrap_or(Rect {
                            x: f64::from(size.width) / scale / 2.0 - 48.0,
                            y: f64::from(size.height) / scale - 120.0,
                            width: 96.0,
                            height: 84.0,
                        }),
                    s.preferences.clone(),
                )
            };
            preferences.dock = {
                let mut random = runtime.random.lock().map_err(|_| AppError::storage())?;
                detect(point, body, area, scale, random.as_mut())?
            };
            save(&store.connection, &preferences)?;
            let mut s = managed.inner.lock().map_err(|_| AppError::storage())?;
            s.preferences = preferences;
            s.revision += 1;
            s.dragging = false;
            s.target = None;
            s.stop_motion();
            managed.epoch.fetch_add(1, Ordering::SeqCst);
            Ok(s.snapshot())
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    let target = snap(snapshot.dock, point, size, area);
    window
        .set_position(PhysicalPosition::new(target.0, target.1))
        .map_err(|_| AppError::storage())?;
    publish(&app, &snapshot);
    Ok(())
}
fn next_step(start: (i32, i32), target: (i32, i32), limit: f64) -> (i32, i32) {
    let (x, y) = (
        f64::from(target.0) - f64::from(start.0),
        f64::from(target.1) - f64::from(start.1),
    );
    let length = x.hypot(y);
    if length <= limit {
        return target;
    }
    (
        (f64::from(start.0) + x / length * limit).round() as i32,
        (f64::from(start.1) + y / length * limit).round() as i32,
    )
}
#[cfg(windows)]
fn near_mouse(window: &WebviewWindow, scale: f64) -> bool {
    use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let (Ok(p), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return true;
    };
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) }.is_err() {
        return true;
    }
    let margin = 32.0 * scale;
    f64::from(cursor.x) >= f64::from(p.x) - margin
        && f64::from(cursor.x) <= f64::from(p.x) + f64::from(size.width) + margin
        && f64::from(cursor.y) >= f64::from(p.y) - margin
        && f64::from(cursor.y) <= f64::from(p.y) + f64::from(size.height) + margin
}
#[cfg(not(windows))]
fn near_mouse(_: &WebviewWindow, _: f64) -> bool {
    true
}
fn chat_open(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("chat")
        .is_some_and(|w| w.is_visible().unwrap_or(true))
}
pub fn tick(app: &tauri::AppHandle, view: &View) {
    let runtime = app.state::<Runtime>();
    let Ok(sample) = runtime.clock.sample() else {
        return;
    };
    let managed = app.state::<Desktop>();
    {
        let Ok(mut s) = managed.inner.lock() else {
            return;
        };
        if !s.allowed(view) {
            managed.epoch.fetch_add(1, Ordering::SeqCst);
            let changed=s.walking.is_some();
            s.stop_motion();
            s.last_check = Some(sample.running);
            if changed { let snapshot=s.snapshot(); drop(s); publish(app,&snapshot); }
            return;
        }
        if s.motion.is_some()
            || s.last_check
                .is_some_and(|last| sample.running.saturating_sub(last) < INTERVAL)
        {
            return;
        }
        s.last_check = Some(sample.running);
    }
    // 原生查询在所有业务/状态锁之外；不让主线程和 SQLite 相互等待。
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok((point, size, area, scale)) = geometry(&window) else {
        return;
    };
    if !window.is_visible().unwrap_or(false)
        || window.is_minimized().unwrap_or(true)
        || chat_open(app)
        || near_mouse(&window, scale)
    {
        return;
    }
    let plan = (|| -> AppResult<_> {
        let mut random = runtime.random.lock().map_err(|_| AppError::storage())?;
        let mut s = managed.inner.lock().map_err(|_| AppError::storage())?;
        if !s.allowed(view) {
            return Ok(None);
        }
        let target = if let Some(p) = s
            .target
            .filter(|p| ((p.0 - point.0) as f64).hypot((p.1 - point.1) as f64) > 8.0 * scale)
        {
            area.fit(p, size)
        } else {
            let x = random.below(u64::from(area.width.saturating_sub(size.width)) + 1)?;
            let y = random.below(u64::from(area.height.saturating_sub(size.height)) + 1)?;
            area.fit(
                (
                    (i64::from(area.x) + x as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                    (i64::from(area.y) + y as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                ),
                size,
            )
        };
        s.target = Some(target);
        let end = area.fit(next_step(point, target, 140.0 * scale), size);
        if point == end {
            return Ok(None);
        }
        let generation = managed.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        s.motion = Some(generation);
        s.walking=Some(Walking { direction: if end.0 < point.0 { WalkDirection::Left } else { WalkDirection::Right }, persona_id:view.persona_id.clone(),session_id:view.session_id.clone().unwrap_or_default(),context_revision:view.context_revision });
        s.revision+=1;
        Ok(Some((generation, end, s.snapshot())))
    })();
    let Ok(Some((generation, end, snapshot))) = plan else {
        return;
    };
    publish(app,&snapshot);
    let app = app.clone();
    std::thread::spawn(move || {
        for step in 1..=24 {
            std::thread::sleep(Duration::from_millis(60));
            let state = app.state::<Desktop>();
            if state.epoch.load(Ordering::SeqCst) != generation
                || !window.is_visible().unwrap_or(false)
                || window.is_minimized().unwrap_or(true)
                || near_mouse(&window, scale)
                || chat_open(&app)
            {
                break;
            }
            let fraction = f64::from(step) / 24.0;
            let position = (
                (f64::from(point.0) + f64::from(end.0 - point.0) * fraction).round() as i32,
                (f64::from(point.1) + f64::from(end.1 - point.1) * fraction).round() as i32,
            );
            if window
                .set_position(PhysicalPosition::new(position.0, position.1))
                .is_err()
            {
                break;
            }
        }
        let state = app.state::<Desktop>();
        if let Ok(mut s) = state.inner.lock() {
            if s.motion == Some(generation) {
                s.stop_motion();
                let snapshot=s.snapshot(); drop(s); publish(&app,&snapshot);
            }
        };
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Draw(Vec<u64>);
    impl RandomSource for Draw {
        fn below(&mut self, max: u64) -> AppResult<u64> {
            let v = self.0.remove(0);
            assert!(v < max);
            Ok(v)
        }
    }
    fn area() -> Area {
        Area {
            x: -1600,
            y: -100,
            width: 1600,
            height: 1000,
        }
    }
    #[test]
    fn release_past_work_area_still_docks_and_recovers_entire_window() {
        let a=area(); let size=PhysicalSize::new(480,600);
        for (point,expected) in [((-2000,100),Dock::PeekLeft),((100,100),Dock::PeekRight),((-900,900),Dock::RestSit)] {
            let dock=detect(point,body(),a,1.5,&mut Draw(vec![0])).unwrap();
            assert_eq!(dock,expected);
            let recovered=snap(dock,point,size,a);
            assert_eq!(recovered,a.fit(recovered,size));
            assert!(a.intersection(recovered,size)==i64::from(size.width)*i64::from(size.height));
        }
    }
    fn body() -> Rect {
        Rect {
            x: 100.0,
            y: 160.0,
            width: 120.0,
            height: 80.0,
        }
    }
    #[test]
    fn body_edges_and_corner_draw_only_when_needed() {
        assert_eq!(
            detect((-1700, 0), body(), area(), 1.0, &mut Draw(vec![])).unwrap(),
            Dock::PeekLeft
        );
        assert_eq!(
            detect((-200, 0), body(), area(), 1.0, &mut Draw(vec![])).unwrap(),
            Dock::PeekRight
        );
        assert_eq!(
            detect((-900, 660), body(), area(), 1.0, &mut Draw(vec![0])).unwrap(),
            Dock::RestSit
        );
        assert_eq!(
            detect((-1700, 660), body(), area(), 1.0, &mut Draw(vec![0])).unwrap(),
            Dock::PeekLeft
        );
        assert_eq!(
            detect((-1700, 660), body(), area(), 1.0, &mut Draw(vec![1, 1])).unwrap(),
            Dock::RestLie
        );
        assert_eq!(
            detect((-900, 0), body(), area(), 1.0, &mut Draw(vec![])).unwrap(),
            Dock::None
        );
    }
    #[test]
    fn snap_uses_work_area_and_motion_stays_bounded() {
        let size = PhysicalSize::new(480, 420);
        assert_eq!(snap(Dock::RestLie, (-900, 1000), size, area()), (-900, 480));
        assert_eq!(snap(Dock::PeekRight, (-200, 0), size, area()), (-480, 0));
        let step = next_step((-1400, 0), (-600, 600), 140.0);
        assert!(((step.0 + 1400) as f64).hypot(step.1 as f64) <= 141.0);
    }
    #[test]
    fn docking_stays_on_edge_after_input_and_hidden_size_changes() {
        let normal = PhysicalSize::new(320, 280);
        let input = PhysicalSize::new(320, 400);
        let hidden = PhysicalSize::new(112, 44);
        let a = area();
        let original = snap(Dock::RestSit, (-900, 0), normal, a);
        let expanded = snap(Dock::RestSit, original, input, a);
        assert_eq!(expanded.1 + input.height as i32, a.y + a.height as i32);
        assert_eq!(snap(Dock::RestSit, expanded, normal, a), original);
        let small = snap(Dock::RestSit, original, hidden, a);
        assert_eq!(snap(Dock::RestSit, small, normal, a), original);
        let side = snap(Dock::PeekRight, (-900, 0), normal, a);
        let small = snap(Dock::PeekRight, side, hidden, a);
        assert_eq!(snap(Dock::PeekRight, small, normal, a), side);
    }
    #[test]
    fn preferences_persist_and_reject_unknown_dock() {
        let c = crate::db::test_database();
        assert!(!load(&c).unwrap().fullscreen_wander);
        save(
            &c,
            &Preferences {
                fullscreen_wander: true,
                dock: Dock::RestSit,
            },
        )
        .unwrap();
        assert_eq!(load(&c).unwrap().dock, Dock::RestSit);
        c.execute("UPDATE settings SET value='{}' WHERE key=?1", [KEY])
            .unwrap();
        assert!(load(&c).is_err());
    }
    #[test]
    fn walking_geometry_does_not_cancel_motion_but_pause_does() {
        let desktop=Desktop::default();
        let mut layout=Layout {persona_id:"1".into(),session_id:"1".into(),context_revision:0,layout_revision:1,
            area:Rect{x:0.0,y:0.0,width:320.0,height:280.0},body:body(),anchor:Some(body()),
            offset:crate::pet::wander::Point{x:0.0,y:0.0},paused:false};
        desktop.update(layout.clone()).unwrap();
        { let mut s=desktop.inner.lock().unwrap();s.motion=Some(7);s.walking=Some(Walking{direction:WalkDirection::Left,persona_id:"1".into(),session_id:"1".into(),context_revision:0}); }
        layout.layout_revision=2;layout.body.height+=8.0;
        desktop.update(layout.clone()).unwrap();
        assert!(desktop.snapshot().unwrap().walking.is_some());
        layout.layout_revision=3;layout.paused=true;
        desktop.update(layout).unwrap();
        assert!(desktop.snapshot().unwrap().walking.is_none());
    }
}
