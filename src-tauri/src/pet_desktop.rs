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
pub(crate) mod toss;
pub(crate) mod chase;
pub(crate) use toss::{track_drag, try_launch};

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
    Top,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Preferences {
    #[serde(default)]
    upper_layout:bool,
    #[serde(default)]
    floor_inset:f64,
    fullscreen_wander: bool,
    dock: Dock,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkDirection { Left, Right }
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Walking { pub direction: WalkDirection, pub persona_id: String, pub session_id: String, pub context_revision: u64 }
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub floor_inset:f64,
    pub upper_layout: bool,
    pub fullscreen_wander: bool,
    pub dock: Dock,
    pub revision: u64,
    pub walking: Option<Walking>,
    pub toss: Option<toss::Toss>,
    pub drag_velocity: Option<toss::Velocity>,
    pub chase: Option<chase::Chase>,
}
#[derive(Default)]
struct Inner {
    upper_layout: bool,
    preferences: Preferences,
    revision: u64,
    layout: Option<Layout>,
    dragging: bool,
    motion: Option<u64>,
    walking: Option<Walking>,
    last_check: Option<Duration>,
    target: Option<(i32, i32)>,
    toss: Option<toss::Toss>,
    chase: Option<chase::Chase>,
    drag_velocity: Option<toss::Velocity>,
    drag_trace: toss::DragTrace,
    drag_serial: u64,
    pending_anchor: Option<Rect>,
    drag_cursor_offset: Option<(i32,i32)>,
    chase_ended_at: Option<std::time::Instant>,
    finishing_drag: bool,
}
#[derive(Default)]
pub struct Desktop {
    inner: Mutex<Inner>,
    epoch: AtomicU64,
    toss_epoch: AtomicU64,
}
impl Inner {
    fn docking_body(&self)->Option<Rect> {
        let l=self.layout.as_ref()?;let anchor=l.anchor.unwrap_or(l.body);let body=self.pending_anchor.unwrap_or(anchor);
        if matches!(self.preferences.dock,Dock::Top|Dock::None) {if let Some(visible)=l.contact.as_ref().and_then(|c|c.visible_rect()) {return Some(Rect{y:visible.y+body.y-anchor.y,..visible})}}
        Some(body)
    }
    fn chase_due(&self,now:std::time::Instant,cooldown:Duration)->bool {self.chase_ended_at.is_none_or(|ended|now.saturating_duration_since(ended)>=cooldown)}
    fn stop_motion(&mut self) { self.motion = None; if self.chase.take().is_some() {self.chase_ended_at=Some(std::time::Instant::now());self.revision+=1;} if self.walking.take().is_some() { self.revision += 1; } }
    fn show_walking_after_move(&mut self, generation: u64, walking: &Walking) -> Option<Snapshot> {
        if self.motion != Some(generation) || self.walking.is_some() { return None; }
        self.walking = Some(walking.clone());
        self.revision += 1;
        Some(self.snapshot())
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            floor_inset:if matches!(self.preferences.dock,Dock::RestSit|Dock::RestLie){self.preferences.floor_inset}else{0.0},
            upper_layout:self.upper_layout && matches!(self.preferences.dock,Dock::None|Dock::Top),
            fullscreen_wander: self.preferences.fullscreen_wander,
            dock: self.preferences.dock,
            revision: self.revision,
            walking: self.walking.clone(),
            toss: self.toss.clone(),
            drag_velocity: self.drag_velocity,
            chase: self.chase.clone(),
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
        serde_json::from_str::<Preferences>(&s).and_then(|p|if p.floor_inset.is_finite() && (0.0..=100_000.0).contains(&p.floor_inset){Ok(p)}else{Err(serde::de::Error::custom("invalid floor inset"))}).map_err(|_| {
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
        if s.toss.as_ref().is_some_and(|t| t.persona_id!=layout.persona_id || t.session_id!=layout.session_id || t.context_revision!=layout.context_revision)
            || s.layout.as_ref().is_some_and(|old|old.area.width!=layout.area.width||old.area.height!=layout.area.height) {
            self.toss_epoch.fetch_add(1,Ordering::SeqCst);
            if s.toss.take().is_some(){s.revision+=1;}
        }
        if cancel { self.epoch.fetch_add(1, Ordering::SeqCst); s.stop_motion(); }
        if s.pending_anchor.is_some_and(|b|layout.anchor.is_some_and(|a|(a.y-b.y).abs()<1.0)) {s.pending_anchor=None;}
        s.layout = Some(layout);
        Ok(())
    }
    pub fn begin_drag(&self) -> AppResult<()> {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.toss_epoch.fetch_add(1,Ordering::SeqCst);
        let mut s = self.inner.lock().map_err(|_| AppError::storage())?;
        if let Some(t)=s.toss.as_mut(){t.phase=toss::Phase::Caught;s.revision+=1;}
        s.drag_cursor_offset=None;
        s.dragging = true;
        s.finishing_drag=false;
        s.drag_serial+=1;
        s.drag_trace=toss::DragTrace::default();s.drag_velocity=None;
        s.stop_motion();
        s.target = None;
        Ok(())
    }
    pub fn capture_drag_cursor(&self,window:&WebviewWindow)->AppResult<()> {
        let point=window.outer_position().map_err(|_|AppError::storage())?;
        self.inner.lock().map_err(|_|AppError::storage())?.drag_cursor_offset=toss::cursor().map(|c|(c.0-point.x,c.1-point.y));Ok(())
    }
    pub fn drag_position(&self,cursor:(i32,i32))->Option<(i32,i32)> {
        let s=self.inner.lock().ok()?;if !s.dragging{return None}let offset=s.drag_cursor_offset?;Some((cursor.0-offset.0,cursor.1-offset.1))
    }
    pub fn preserve_upper_position(&self,window:&WebviewWindow)->AppResult<bool> {
        let (dock,body,inset)={let s=self.inner.lock().map_err(|_|AppError::storage())?;if s.dragging||s.motion.is_some()||s.toss.is_some(){return Ok(true)}(s.preferences.dock,s.docking_body(),s.preferences.floor_inset)};
        if matches!(dock,Dock::RestSit|Dock::RestLie)&&inset>0.0 {self.realign_dock(window)?;return Ok(true)}
        let Some(body)=body else{return Ok(false)};if !matches!(dock,Dock::None|Dock::Top){return Ok(false)}
        let (point,size,area,scale)=geometry(window)?;
        if dock!=Dock::Top && point.1>=area.y{return Ok(false)}
        let next=snap_body(dock,point,size,area,body,scale);
        if next!=point {window.set_position(PhysicalPosition::new(next.0,next.1)).map_err(|_|AppError::storage())?;}Ok(true)
    }
    pub fn end_drag(&self) -> AppResult<()> {
        let mut s=self.inner.lock().map_err(|_| AppError::storage())?;s.dragging=false;s.drag_velocity=None;
        Ok(())
    }
    pub fn is_dragging(&self) -> AppResult<bool> {
        Ok(self.inner.lock().map_err(|_| AppError::storage())?.dragging)
    }
    pub fn drag_serial(&self)->Option<u64>{self.inner.lock().ok().map(|s|s.drag_serial)}
    pub fn drag_finished_for(&self,serial:Option<u64>)->bool {self.inner.lock().is_ok_and(|s|serial==Some(s.drag_serial)&&!s.dragging)}
    pub fn claim_drag_finish_for(&self,serial:Option<u64>) -> AppResult<bool> {
        let mut s=self.inner.lock().map_err(|_|AppError::storage())?;
        if !s.dragging||s.finishing_drag||serial.is_some_and(|value|value!=s.drag_serial){return Ok(false)}
        s.finishing_drag=true;Ok(true)
    }
    pub fn release_velocity(&self, scale:f64)->AppResult<Option<toss::Velocity>> {
        let mut s=self.inner.lock().map_err(|_|AppError::storage())?;
        if let Some(point)=toss::cursor(){s.drag_trace.record(std::time::Instant::now(),point);}
        Ok(s.drag_trace.upward_velocity(std::time::Instant::now(),scale))
    }
    pub fn has_toss(&self)->bool {self.inner.lock().is_ok_and(|s|s.toss.is_some())}
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
        let (point, size, area, scale) = geometry(window)?;
        let floor_inset=self.inner.lock().map_err(|_|AppError::storage())?.preferences.floor_inset;
        let body=self.inner.lock().map_err(|_|AppError::storage())?.docking_body();
        let target = if matches!(dock,Dock::RestSit|Dock::RestLie){snap_floor(dock,point,size,area,floor_inset,scale)}else{body.map(|b|snap_body(dock,point,size,area,b,scale)).unwrap_or_else(||snap(dock,point,size,area))};
        if target != point {
            window.set_position(PhysicalPosition::new(target.0, target.1))
                .map_err(|_| AppError::storage())?;
        }
        Ok(())
    }
    pub fn initialize(&self, window: &WebviewWindow, runtime: &Runtime) -> AppResult<()> {
        let (preferences,saved) = runtime.read(|s, _| Ok((load(&s.connection)?,crate::config::get_window_position(&s.connection)?)))?;
        let dock = preferences.dock;let floor_inset=preferences.floor_inset;
        {
            let mut s=self.inner.lock().map_err(|_|AppError::storage())?;
            s.upper_layout=preferences.upper_layout;s.preferences=preferences;s.revision+=1;
        }
        if dock != Dock::None {
            let (point, size, area, scale) = geometry(window)?;
            let target = snap_floor(dock,point,size,area,floor_inset,scale);
            window
                .set_position(PhysicalPosition::new(target.0, target.1))
                .map_err(|_| AppError::storage())?;
        }
        if dock==Dock::None && self.inner.lock().map_err(|_|AppError::storage())?.upper_layout {
            if let Some(saved)=saved {
                let (_,size,area,_)=geometry(window)?;
                if saved.1<area.y && area.intersection(saved,size)>0 {
                    let point=(area.fit(saved,size).0,saved.1);
                    window.set_position(PhysicalPosition::new(point.0,point.1)).map_err(|_|AppError::storage())?;
                }
            }
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
                Dock::Top => "normal",
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

#[tauri::command]
pub async fn get_pet_error_expression(window: WebviewWindow, runtime: tauri::State<'_, Runtime>, persona_id: String) -> AppResult<String> {
    authorize(&window)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rt.read(|s,_| {
        let category="error";
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
        Dock::Top => p.1=area.y,
        Dock::None => {}
    }
    p
}
// 透明窗口可以越过屏幕上缘；约束立绘，而不是把透明留白整个拉回屏幕。
fn snap_floor(dock:Dock,point:(i32,i32),size:PhysicalSize<u32>,area:Area,inset:f64,scale:f64)->(i32,i32) {
    let mut p=snap(dock,point,size,area);
    if matches!(dock,Dock::RestSit|Dock::RestLie){p.1=p.1.saturating_add((inset*scale).round() as i32);}p
}
fn reanchor(point:(i32,i32),body:Rect,next_y:f64,scale:f64)->((i32,i32),Rect) {
    ((point.0,point.1+((body.y-next_y)*scale).round() as i32),Rect{y:next_y,..body})
}
fn fit_body(point:(i32,i32),size:PhysicalSize<u32>,area:Area,body:Rect,scale:f64)->(i32,i32) {
    let mut fitted=area.fit(point,size);
    let min_y=(f64::from(area.y)-body.y*scale+4.0*scale).round() as i32;
    let max_y=area.fit((point.0,i32::MAX),size).1.max(min_y);
    fitted.1=point.1.clamp(min_y,max_y);fitted
}
fn snap_body(dock:Dock,point:(i32,i32),size:PhysicalSize<u32>,area:Area,body:Rect,scale:f64)->(i32,i32) {
    if matches!(dock,Dock::None|Dock::Top) {
        let mut p=fit_body(point,size,area,body,scale);
        if dock==Dock::Top {p.1=(f64::from(area.y)-body.y*scale+4.0*scale).round() as i32;}p
    }else {snap(dock,point,size,area)}
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
    if side.is_none() && !floor && top<=f64::from(area.y)+8.0*scale {return Ok(Dock::Top)}
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
    let (snapshot,body,point) = tauri::async_runtime::spawn_blocking(move || {
        runtime.read(|store, _| {
            let managed = handle.state::<Desktop>();
            let (body, visible, free_y, old_dock, old_upper, had_toss, mut preferences) = {
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
                    s.layout.as_ref().and_then(|l|l.contact.as_ref().and_then(|c|c.visible_rect())),
                    s.layout.as_ref().and_then(|l|l.free_y),
                    s.preferences.dock,
                    s.upper_layout,
                    s.toss.is_some(),
                    s.preferences.clone(),
                )
            };
            preferences.dock = {
                let mut random = runtime.random.lock().map_err(|_| AppError::storage())?;
                let dock=detect(point, body, area, scale, random.as_mut())?;
                if dock==Dock::Top && visible.is_some_and(|v|f64::from(point.1)+v.y*scale>f64::from(area.y)+8.0*scale) {Dock::None}else{dock}
            };
            let upper=matches!(preferences.dock,Dock::None|Dock::Top) && (preferences.dock==Dock::Top || f64::from(point.1)+body.y*scale < f64::from(area.y)+80.0*scale);
            let next_y=if upper {4.0} else if preferences.dock==Dock::None && (old_dock==Dock::Top||old_upper||had_toss) {free_y.unwrap_or(f64::from(size.height)/scale-body.height-56.0)} else {body.y};
            let (point,next_body)=reanchor(point,body,next_y,scale);
            preferences.upper_layout=upper;preferences.floor_inset=0.0;
            save(&store.connection, &preferences)?;
            let mut s = managed.inner.lock().map_err(|_| AppError::storage())?;
            s.pending_anchor=((next_y-body.y).abs()>0.5).then_some(next_body);
            s.upper_layout=upper;
            s.preferences = preferences;
            s.revision += 1;
            s.dragging = false;
            s.toss=None;
            s.target = None;
            s.stop_motion();
            managed.epoch.fetch_add(1, Ordering::SeqCst);
            let docking_body=if matches!(s.preferences.dock,Dock::Top|Dock::None) {visible.map(|v|Rect{y:v.y+next_y-body.y,..v}).unwrap_or(next_body)}else{next_body};
            Ok((s.snapshot(),docking_body,point))
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    let target = snap_body(snapshot.dock, point, size, area,body,scale);
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
fn outside_arrival_tolerance(start: (i32, i32), end: (i32, i32), scale: f64) -> bool {
    (f64::from(end.0) - f64::from(start.0)).hypot(f64::from(end.1) - f64::from(start.1)) > 8.0 * scale
}
fn walking_pose_ready(start: (i32, i32), current: (i32, i32), end: (i32, i32), scale: f64) -> bool {
    outside_arrival_tolerance(start, current, scale) && outside_arrival_tolerance(current, end, scale)
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
        .is_some_and(|w| w.is_visible().unwrap_or(true) && !w.is_minimized().unwrap_or(false))
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
            let changed=s.walking.is_some() || s.chase.is_some();
            s.stop_motion();
            s.last_check = Some(sample.running);
            if changed { let snapshot=s.snapshot(); drop(s); publish(app,&snapshot); }
            return;
        }
        if s.toss.is_some() || s.chase.is_some() || s.motion.is_some()
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
    let cooldown = runtime.read(|store, _| {
        let settings = crate::config::get(&store.connection)?;
        Ok(Duration::from_secs(settings["pet_chase_cooldown_minutes"].parse::<u64>().unwrap_or(10) * 60))
    }).unwrap_or(Duration::from_secs(600));
    let plan = (|| -> AppResult<_> {
        let mut random = runtime.random.lock().map_err(|_| AppError::storage())?;
        let mut s = managed.inner.lock().map_err(|_| AppError::storage())?;
        if !s.allowed(view) {
            return Ok(None);
        }
        let target = if let Some(p) = s
            .target
            .filter(|p| outside_arrival_tolerance(point, *p, scale))
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
        let body=s.docking_body();
        let end = body.map(|b|fit_body(next_step(point,target,140.0*scale),size,area,b,scale)).unwrap_or_else(||area.fit(next_step(point,target,140.0*scale),size));
        // 复用到达容差，不为几像素的剩余距离重播行走和站立表情。
        if !outside_arrival_tolerance(point, end, scale) {
            return Ok(None);
        }
        let chase = s.chase_due(std::time::Instant::now(),cooldown) && chase::draw(random.as_mut())?;
        let generation = managed.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        s.motion = Some(generation);
        let walking=Walking { direction: if end.0 < point.0 { WalkDirection::Left } else { WalkDirection::Right }, persona_id:view.persona_id.clone(),session_id:view.session_id.clone().unwrap_or_default(),context_revision:view.context_revision };
        if chase { s.chase=Some(chase::Chase::new(&walking));s.revision+=1; }
        Ok(Some((generation, end, walking, chase)))
    })();
    let Ok(Some((generation, end, walking, chase))) = plan else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        if chase { chase::run(&app,&window,generation,point,size,area,scale,walking);return; }
        let mut last_position = point;
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
            if position == last_position { continue; }
            if window
                .set_position(PhysicalPosition::new(position.0, position.1))
                .is_err()
            {
                break;
            }
            last_position = position;
            // 离开起点容差且尚未进入终点容差才显示行走，避免微移后暂停或收尾时闪出动作。
            if walking_pose_ready(point, position, end, scale) {
                let snapshot = state.inner.lock().ok().and_then(|mut s| s.show_walking_after_move(generation, &walking));
                if let Some(snapshot) = snapshot { publish(&app, &snapshot); }
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
                upper_layout:true,floor_inset:102.0,
            },
        )
        .unwrap();
        assert_eq!(load(&c).unwrap().dock, Dock::RestSit);assert!(load(&c).unwrap().upper_layout);assert_eq!(load(&c).unwrap().floor_inset,102.0);
        let legacy:Preferences=serde_json::from_str(r#"{"fullscreenWander":true,"dock":"none"}"#).unwrap();assert!(!legacy.upper_layout);assert_eq!(legacy.floor_inset,0.0);
        c.execute("UPDATE settings SET value='{}' WHERE key=?1", [KEY])
            .unwrap();
        assert!(load(&c).is_err());
    }
    #[test]
    fn tiny_wander_displacements_use_existing_arrival_tolerance_at_each_dpi() {
        for scale in [1.0, 1.5, 2.0] {
            let start = (-1400, -80);
            let tolerance = (8.0 * scale) as i32;
            for delta in [0, 1, tolerance] {
                assert!(!outside_arrival_tolerance(start, (start.0 + delta, start.1), scale));
            }
            assert!(outside_arrival_tolerance(start, (start.0 + tolerance + 1, start.1), scale));
            assert!(outside_arrival_tolerance(start, next_step(start, (-600, 600), 140.0 * scale), scale));
            let end = (start.0 + 140, start.1);
            assert!(!walking_pose_ready(start, (start.0 + 1, start.1), end, scale));
            assert!(walking_pose_ready(start, (start.0 + 60, start.1), end, scale));
            assert!(!walking_pose_ready(start, (end.0 - 1, end.1), end, scale));
            let tiny_end = (start.0 + tolerance + 1, start.1);
            for delta in 0..=tolerance + 1 {
                assert!(!walking_pose_ready(start, (start.0 + delta, start.1), tiny_end, scale));
            }
        }
    }
    #[test]
    fn walking_starts_after_move_and_cancelled_plans_cannot_restore_it() {
        let walking = Walking { direction: WalkDirection::Left, persona_id:"1".into(),session_id:"1".into(),context_revision:0 };
        let mut state = Inner { motion:Some(7), ..Inner::default() };
        assert!(state.snapshot().walking.is_none());
        assert!(state.show_walking_after_move(6, &walking).is_none());
        let snapshot = state.show_walking_after_move(7, &walking).unwrap();
        assert!(snapshot.walking.is_some());
        assert!(state.show_walking_after_move(7, &walking).is_none());
        assert_eq!(state.revision, snapshot.revision);
        state.stop_motion();
        assert!(state.show_walking_after_move(7, &walking).is_none());
        state.motion = Some(8);
        state.stop_motion();
        assert!(state.show_walking_after_move(8, &walking).is_none());
        assert!(state.snapshot().walking.is_none());
    }
    #[test]
    fn stale_release_cannot_finish_or_reset_a_new_drag() {
        let desktop=Desktop::default();desktop.begin_drag().unwrap();let first=desktop.drag_serial();
        assert!(desktop.claim_drag_finish_for(first).unwrap());desktop.end_drag().unwrap();assert!(desktop.drag_finished_for(first));
        desktop.begin_drag().unwrap();let second=desktop.drag_serial();assert_ne!(first,second);
        assert!(!desktop.claim_drag_finish_for(first).unwrap());assert!(!desktop.drag_finished_for(first));
        assert!(desktop.claim_drag_finish_for(second).unwrap());assert!(!desktop.drag_finished_for(second));
        desktop.end_drag().unwrap();assert!(desktop.drag_finished_for(second));
    }
    #[test]
    fn catching_cancels_toss_and_preserves_body_anchor_until_drag_release() {
        let desktop=Desktop::default();let token=desktop.toss_epoch.load(Ordering::SeqCst);
        let body=Rect{x:62.0,y:102.0,width:196.0,height:196.0};
        desktop.inner.lock().unwrap().toss=Some(toss::Toss{velocity:toss::Velocity::default(),phase:toss::Phase::Down,persona_id:"1".into(),session_id:"1".into(),context_revision:0,body,controls_below:true});
        desktop.begin_drag().unwrap();let snapshot=desktop.snapshot().unwrap();let pose=snapshot.toss.unwrap();
        assert_eq!(pose.phase,toss::Phase::Caught);assert_eq!(pose.body.x,body.x);assert_eq!(pose.body.y,body.y);assert_eq!(pose.body.height,body.height);
        assert_ne!(desktop.toss_epoch.load(Ordering::SeqCst),token);assert!(desktop.is_dragging().unwrap());
    }
    #[test]
    fn top_edge_uses_body_anchor_and_finalizes_drag_once() {
        let a=area();let size=PhysicalSize::new(320,400);
        assert!(matches!(detect((-900,-260),body(),a,1.0,&mut Draw(vec![])).unwrap(),Dock::Top));
        assert_eq!(snap(Dock::Top,(-900,-500),size,a),(-900,-100));
        assert!(matches!(detect((-900,100),body(),a,1.0,&mut Draw(vec![])).unwrap(),Dock::None));
        let desktop=Desktop::default();desktop.begin_drag().unwrap();assert!(desktop.claim_drag_finish_for(None).unwrap());assert!(!desktop.claim_drag_finish_for(None).unwrap());
    }
    #[test]
    fn walking_geometry_does_not_cancel_motion_but_pause_does() {
        let desktop=Desktop::default();
        let mut layout=Layout {persona_id:"1".into(),session_id:"1".into(),context_revision:0,layout_revision:1,
            area:Rect{x:0.0,y:0.0,width:320.0,height:280.0},body:body(),anchor:Some(body()),free_y:None,contact:None,
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
    #[test]
    fn top_detection_uses_body_and_release_reanchoring_keeps_screen_position() {
        let a=area();let size=PhysicalSize::new(320,400);let b=body();
        assert_eq!(detect((-900,a.y),b,a,1.0,&mut Draw(vec![])).unwrap(),Dock::None);
        let point=(-900,a.y-b.y as i32+5);assert_eq!(detect(point,b,a,1.0,&mut Draw(vec![])).unwrap(),Dock::Top);
        let top=Rect{y:4.0,..b};let snapped=snap_body(Dock::Top,point,size,a,top,1.0);assert_eq!(snapped.1+top.y as i32,a.y+4);
        let dragged=(snapped.0,snapped.1+40);let free=Rect{y:200.0,..b};let (corrected,free)=reanchor(dragged,top,free.y,1.0);
        let fitted=snap_body(Dock::None,corrected,size,a,free,1.0);assert_eq!(f64::from(fitted.1)+free.y,f64::from(dragged.1)+top.y);
        let d=Desktop::default();d.begin_drag().unwrap();d.inner.lock().unwrap().drag_cursor_offset=Some((120,18));assert_eq!(d.drag_position((600,7)),Some((480,-11)));d.end_drag().unwrap();assert!(d.drag_position((600,7)).is_none());
    }

    #[test]
    fn top_visible_body_compensation_tracks_transparent_margin_and_dpi() {
        let d=Desktop::default();let b=body();let layout=Layout{persona_id:"1".into(),session_id:"1".into(),context_revision:0,layout_revision:1,
            area:Rect{x:0.0,y:0.0,width:320.0,height:400.0},body:b,anchor:Some(b),free_y:Some(200.0),offset:crate::pet::wander::Point::default(),paused:false,
            contact:Some(crate::pet::wander::Contact{bounds:Rect{x:b.x,y:b.y+12.0,width:96.0,height:64.0},rows:vec![u32::MAX;32]})};
        d.update(layout).unwrap();{let mut s=d.inner.lock().unwrap();s.preferences.dock=Dock::Top;s.pending_anchor=Some(Rect{y:4.0,..b});}
        assert_eq!(d.inner.lock().unwrap().docking_body().unwrap().y,16.0);
        {let mut s=d.inner.lock().unwrap();s.preferences.dock=Dock::None;s.upper_layout=true;}
        let visible=d.inner.lock().unwrap().docking_body().unwrap();let point=(-900,area().y-7);
        assert_eq!(snap_body(Dock::None,point,PhysicalSize::new(320,400),area(),visible,1.0),point);
        for scale in [1.0,1.25,1.5,2.0] {let p=(100,130);let (next,body)=reanchor(p,b,4.0,scale);assert!((f64::from(next.1)+body.y*scale-(f64::from(p.1)+b.y*scale)).abs()<=0.5);}
    }

}
