//! 短期食物游标层，不保存状态、不接收鼠标输入、不查询私人业务资料。
use crate::error::{AppError, AppResult};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tauri::{Emitter, Manager};
#[derive(Default)]
pub struct Cursor(pub Arc<AtomicU64>);
#[tauri::command]
pub async fn set_food_cursor(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    food_id: Option<String>,
) -> AppResult<()> {
    if window.label() != "main" {
        return Err(AppError::new("PERMISSION_DENIED", "只有宠窗可以持有食物"));
    }
    if let Some(id) = &food_id {
        if ![
            "token",
            "cloud_cake",
            "black_tea",
            "cat_cookie",
            "rice",
            "dried_fish",
        ]
        .contains(&id.as_str())
        {
            return Err(AppError::validation("foodId", "无效食物"));
        }
    }
    let generation = app.state::<Cursor>().0.clone();
    let token = generation.fetch_add(1, Ordering::SeqCst) + 1;
    let (send, recv) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    let present = food_id.is_some();
    app.run_on_main_thread(move || {
        let result = (|| -> AppResult<()> {
            if let Some(w) = handle.get_webview_window("food-cursor") {
                w.close().map_err(|_| AppError::storage())?;
            }
            if let Some(id) = food_id {
                let w = tauri::WebviewWindowBuilder::new(
                    &handle,
                    "food-cursor",
                    tauri::WebviewUrl::App(format!("index.html?food={id}").into()),
                )
                .title("投喂食物")
                .inner_size(190.0, 102.0)
                .decorations(false)
                .transparent(true)
                .background_color(tauri::window::Color(0, 0, 0, 0))
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .focusable(false)
                .visible(false)
                .build()
                .map_err(|_| AppError::storage())?;
                w.set_ignore_cursor_events(true)
                    .map_err(|_| AppError::storage())?;
                crate::pet_region::enforce_frameless(&w)
                    .map_err(|message| AppError::new("FOOD_CURSOR_ERROR", message))?;
                #[cfg(windows)]
                {
                    use windows::Win32::{
                        Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos,
                    };
                    let mut point = POINT::default();
                    if unsafe { GetCursorPos(&mut point) }.is_ok() {
                        w.set_position(tauri::PhysicalPosition::new(point.x + 12, point.y + 12))
                            .map_err(|_| AppError::storage())?;
                    }
                }
                w.show().map_err(|_| AppError::storage())?;
            }
            Ok(())
        })();
        let _ = send.send(result);
    })
    .map_err(|_| AppError::storage())?;
    recv.await.map_err(|_| AppError::storage())??;
    if present {
        std::thread::spawn(move || {
            while generation.load(Ordering::SeqCst) == token {
                #[cfg(windows)]
                {
                    use windows::Win32::{
                        Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos,
                    };
                    let mut point = POINT::default();
                    if unsafe { GetCursorPos(&mut point) }.is_ok() {
                        if let Some(w) = app.get_webview_window("food-cursor") {
                            let _ = w.set_position(tauri::PhysicalPosition::new(
                                point.x + 12,
                                point.y + 12,
                            ));
                        } else {
                            break;
                        }
                    }
                    if unsafe { GetAsyncKeyState(0x1B) } < 0 || unsafe { GetAsyncKeyState(0x02) } < 0 {
                        if generation
                            .compare_exchange(token, token + 1, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            if let Some(w) = app.get_webview_window("food-cursor") {
                                let _ = w.close();
                            }
                            let _ = app.emit_to("main", "food-cursor-cancelled", ());
                        }
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
        });
    }
    Ok(())
}
#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn GetAsyncKeyState(vkey: i32) -> i16;
}
