#[cfg(test)]
use crate::pet::{wander::{Controller,Point,Rect,Target},RandomSource};
// 原生窗口游走入口；旧画布计算辅助仅保留用于兼容性测试。
use crate::{error::{AppError, AppResult}, pet::{controller::View, wander::Layout}, runtime::Runtime};
#[cfg(test)]
use std::{sync::Mutex, time::Duration};
use tauri::Manager;

#[tauri::command]
pub async fn update_pet_wander_layout(window: tauri::WebviewWindow, app: tauri::AppHandle,
    runtime: tauri::State<'_, Runtime>, layout: Layout) -> AppResult<()> {
    if window.label() != "main" { return Err(AppError::new("FORBIDDEN", "只有宠窗可以更新游走布局")) }
    layout.validate()?;
    let rt=runtime.inner().clone();
    let handle=app.clone();
    let snapshot=tauri::async_runtime::spawn_blocking(move || rt.read(|store,_| {
        let context = crate::persona::context(&store.connection,store.context_revision)?;
        if context.persona_id != layout.persona_id || context.session_id.as_deref() != Some(layout.session_id.as_str())
            || context.context_revision != layout.context_revision { return Err(AppError::new("CONTEXT_CHANGED", "游走上下文已改变")) }
        let desktop=handle.state::<crate::pet_desktop::Desktop>();
        desktop.update(layout)?; desktop.snapshot()
    })).await.map_err(|_|AppError::storage())??;
    app.state::<crate::pet_desktop::Desktop>().preserve_upper_position(&window)?;
    crate::pet_desktop::publish(&app,&snapshot);
    Ok(())
}

pub fn tick(app: &tauri::AppHandle, view: &View) {
    crate::pet_desktop::tick(app,view);
}

/// 窗口查询可能同步等待主线程，不能与游走锁或随机源锁交叠。
#[cfg(test)]
fn next_target(
    managed: &Mutex<Controller>, random: &Mutex<Box<dyn RandomSource + Send>>,
    view: &View, now: Duration, chat_open: impl FnOnce() -> bool,
    environment: impl FnOnce() -> Option<(Option<Point>, Option<Rect>)>,
) -> Option<Target> {
    let chat_open = chat_open();
    let due = { managed.lock().ok()?.due(now) };
    // 保留低频采样；锁只用于读取到期状态，系统窗口与鼠标查询在锁外完成。
    let geometry = if due && view.can_wander && !chat_open {
        Some(environment()?)
    } else { None };
    let mut controller = managed.lock().ok()?;
    // 查询期间布局可被主线程更新；若新布局现在到期，留到下一次带采样的检查。
    if geometry.is_none() && controller.due(now) && view.can_wander && !chat_open {
        return None;
    }
    let (mouse, foreground) = geometry.unwrap_or((None, None));
    let mut random = random.lock().ok()?;
    controller.tick(view, now, chat_open, mouse, foreground, random.as_mut()).ok().flatten()
}

#[cfg(all(test,windows))]
fn sample_environment(app: &tauri::AppHandle) -> Option<(Option<Point>,Option<Rect>)> {
    use windows::Win32::{Foundation::{HWND,POINT,RECT}, UI::WindowsAndMessaging::{GetClassNameW,GetCursorPos,GetForegroundWindow,GetWindowRect}};
    let window = app.get_webview_window("main")?;
    let origin = window.inner_position().ok()?;
    let scale = window.scale_factor().ok()?;
    if !scale.is_finite() || scale <= 0.0 { return None }
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) }.ok()?;
    let mouse = Point {x:(f64::from(cursor.x)-f64::from(origin.x))/scale,y:(f64::from(cursor.y)-f64::from(origin.y))/scale};
    let foreground = unsafe { GetForegroundWindow() };
    let own = HWND(window.hwnd().ok()?.0);
    let mut class_name = [0u16; 64];
    let count = if foreground.0.is_null() { 0 } else { unsafe { GetClassNameW(foreground,&mut class_name) } };
    let desktop = matches!(String::from_utf16_lossy(&class_name[..count.max(0) as usize]).as_str(),"Progman"|"WorkerW");
    let obstacle = if foreground.0.is_null() || foreground == own || desktop { None } else {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(foreground,&mut rect) }.ok()?;
        Some(screen_rect(rect.left,rect.top,rect.right,rect.bottom,origin.x,origin.y,scale))
    };
    Some((Some(mouse),obstacle))
}
#[cfg(all(test,not(windows)))]
fn sample_environment(_: &tauri::AppHandle) -> Option<(Option<Point>,Option<Rect>)> { None }

#[cfg(test)]
fn screen_rect(left:i32,top:i32,right:i32,bottom:i32,x:i32,y:i32,scale:f64) -> Rect {
    Rect {x:(f64::from(left)-f64::from(x))/scale,y:(f64::from(top)-f64::from(y))/scale,
        width:(f64::from(right)-f64::from(left))/scale,height:(f64::from(bottom)-f64::from(top))/scale}
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed;
    impl RandomSource for Fixed {
        fn below(&mut self, upper: u64) -> AppResult<u64> { Ok(1 % upper) }
    }
    fn layout() -> Layout {
        Layout { persona_id: "1".into(), session_id: "1".into(), context_revision: 0,
            layout_revision: 1, area: Rect { x: 0.0, y: 0.0, width: 300.0, height: 250.0 },
            body: Rect { x: 100.0, y: 100.0, width: 60.0, height: 50.0 },
            offset: Point::default(), paused: false, anchor:None,free_y:None,contact:None}
    }
    fn view() -> View {
        View { persona_id: "1".into(), session_id: Some("1".into()), context_revision: 0,
            revision: 1, activity: crate::pet::controller::Activity::Normal,
            expression: "(・ω・)".into(), intimacy: Some(0), sleep_until: None, error: None,
            can_interact: true, can_wander: true }
    }
    #[test]
    fn window_queries_can_update_layout_and_use_random_without_deadlock() {
        let managed = Mutex::new(Controller::default());
        managed.lock().unwrap().update(layout()).unwrap();
        let random: Mutex<Box<dyn RandomSource + Send>> = Mutex::new(Box::new(Fixed));
        let sampled = std::cell::Cell::new(false);
        let target = next_target(&managed, &random, &view(), Duration::ZERO, || {
            // 模拟窗口主线程回调；查询可等待它完成，不能持有回调需要的锁。
            let mut controller = managed.try_lock().expect("查询聊天窗口时不能持有游走锁");
            let _random = random.try_lock().expect("查询聊天窗口时不能持有随机源锁");
            controller.update(Layout { layout_revision: 2, ..layout() }).unwrap();
            false
        }, || {
            let _random = random.try_lock().expect("采样窗口时不能持有随机源锁");
            managed.try_lock().expect("采样窗口时不能持有游走锁")
                .update(Layout { layout_revision: 3, ..layout() }).unwrap();
            sampled.set(true);
            Some((None, None))
        }).unwrap();
        assert!(sampled.get());
        assert_eq!(target.layout_revision, 3);
        assert_eq!(target.offset, Point { x: 12.0, y: 0.0 });
        assert!(managed.try_lock().is_ok());
        assert!(random.try_lock().is_ok());
    }
    #[test]
    fn window_sampling_stays_low_frequency_and_latest_pause_wins() {
        let managed = Mutex::new(Controller::default());
        managed.lock().unwrap().update(layout()).unwrap();
        let random: Mutex<Box<dyn RandomSource + Send>> = Mutex::new(Box::new(Fixed));
        assert!(next_target(&managed, &random, &view(), Duration::ZERO, || false,
            || Some((None, None))).is_some());
        assert!(next_target(&managed, &random, &view(), Duration::from_secs(1), || false,
            || panic!("未到低频间隔，不应采样系统窗口")).is_none());
        assert!(next_target(&managed, &random, &view(), Duration::from_secs(5), || false, || {
            managed.lock().unwrap().update(Layout { layout_revision: 2, paused: true, ..layout() }).unwrap();
            Some((None, None))
        }).is_none());
        assert!(next_target(&managed, &random, &view(), Duration::from_secs(6), || true,
            || panic!("聊天窗口打开时不采样游走环境")).is_none());
    }
    #[test]
    fn physical_screen_coordinates_convert_once_with_negative_monitor_origin() {
        assert_eq!(screen_rect(-1900,150,-1750,300,-2000,100,1.5),
            Rect{x:100.0/1.5,y:50.0/1.5,width:100.0,height:100.0});
    }
}
