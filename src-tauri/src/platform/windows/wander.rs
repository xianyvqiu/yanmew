//! 原生坐标采样与游走事件适配；规则和边界计算归 pet::wander。
use crate::{error::{AppError, AppResult}, pet::{controller::View, wander::{Controller, Layout, Point, Rect}}, runtime::Runtime};
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[tauri::command]
pub fn update_pet_wander_layout(window: tauri::WebviewWindow, app: tauri::AppHandle,
    runtime: tauri::State<'_, Runtime>, layout: Layout) -> AppResult<()> {
    if window.label() != "main" { return Err(AppError::new("FORBIDDEN", "只有宠窗可以更新游走布局")) }
    layout.validate()?;
    runtime.read(|store,_| {
        let context = crate::persona::context(&store.connection,store.context_revision)?;
        if context.persona_id != layout.persona_id || context.session_id.as_deref() != Some(layout.session_id.as_str())
            || context.context_revision != layout.context_revision { return Err(AppError::new("CONTEXT_CHANGED", "游走上下文已改变")) }
        app.state::<Mutex<Controller>>().lock().map_err(|_|AppError::storage())?.update(layout)
    })
}

pub fn tick(app: &tauri::AppHandle, view: &View) {
    let runtime = app.state::<Runtime>();
    let Ok(sample) = runtime.clock.sample() else { return };
    let managed = app.state::<Mutex<Controller>>();
    let Ok(mut controller) = managed.lock() else { return };
    let chat_open = app.get_webview_window("chat").is_some_and(|w| w.is_visible().unwrap_or(true));
    let Ok(mut random) = runtime.random.lock() else { return };
    // 业务暂停仍交给控制器处理；只有到低频检查点时才读取系统窗口与鼠标。
    let geometry = if controller.due(sample.running) && view.can_wander && !chat_open {
        sample_environment(app)
    } else { Some((None,None)) };
    let Some((mouse, foreground)) = geometry else { return };
    if let Ok(Some(target)) = controller.tick(view,sample.running,chat_open,mouse,foreground,random.as_mut()) {
        let _ = app.emit("pet-wander-target",target);
    }
}

#[cfg(windows)]
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
#[cfg(not(windows))]
fn sample_environment(_: &tauri::AppHandle) -> Option<(Option<Point>,Option<Rect>)> { None }

fn screen_rect(left:i32,top:i32,right:i32,bottom:i32,x:i32,y:i32,scale:f64) -> Rect {
    Rect {x:(f64::from(left)-f64::from(x))/scale,y:(f64::from(top)-f64::from(y))/scale,
        width:(f64::from(right)-f64::from(left))/scale,height:(f64::from(bottom)-f64::from(top))/scale}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_screen_coordinates_convert_once_with_negative_monitor_origin() {
        assert_eq!(screen_rect(-1900,150,-1750,300,-2000,100,1.5),
            Rect{x:100.0/1.5,y:50.0/1.5,width:100.0,height:100.0});
    }
}
