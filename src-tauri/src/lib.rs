use std::io;
mod commands;
mod pet_region;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub(crate) fn show_pet_window(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    commands::change_hidden(app, false).map_err(io::Error::other)?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "找不到 main 宠窗"))?;

    window.show()?;
    window.unminimize()?;
    window.set_focus()?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(std::sync::Mutex::new(commands::PetState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::get_pet_window_state,
            commands::set_pet_hidden,
            commands::update_pet_regions
        ])
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "显示宠窗", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", false, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "缺少默认托盘图标"))?;

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Err(error) = show_pet_window(app) {
                            eprintln!("显示宠窗失败：{error}");
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Err(error) = show_pet_window(tray.app_handle()) {
                            eprintln!("显示宠窗失败：{error}");
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
