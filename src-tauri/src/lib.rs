use std::io;
pub mod ai;
pub mod chat;
mod commands;
mod desktop_windows;
mod config;
mod crypto;
mod db;
mod error;
mod events;
mod expressions;
mod jobs;
mod location;
mod weather;
pub mod models;
mod persona;
pub mod pet;
mod pet_region;
pub mod pet_store;
mod platform;
mod providers;
pub mod reminders;
pub mod reminder_extract;
mod runtime;
pub mod sessions;
pub mod state;

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

    pet_region::enforce_frameless(&window).map_err(io::Error::other)?;
    window.show()?;
    window.unminimize()?;
    window.set_focus()?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let runtime = runtime::Runtime::initialize(
        platform::windows::paths::application_paths(),
        std::sync::Arc::new(platform::windows::secrets::Dpapi),
        std::sync::Arc::new(platform::windows::time::WindowsClock),
        Box::new(platform::windows::time::WindowsRandom),
    );
    tauri::Builder::default()
        .manage(runtime)
        .manage(std::sync::Mutex::new(commands::PetState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::focus_or_create_window,
            commands::list_reminder_occurrences,
            commands::list_reminders,
            commands::create_reminder,
            commands::update_reminder,
            commands::delete_reminder,
            commands::get_missed_reminder_summary,
            commands::ack_missed_reminder_summary,
            commands::acknowledge_reminder_occurrences,
            commands::update_reminder_occurrence_status,
            commands::complete_reminder_occurrence,
            commands::get_pet_window_state,
            commands::get_pet_activity,
            commands::interact_pet,
            commands::set_pet_dragging,
            commands::refresh_pet_provider,
            commands::set_pet_hidden,
            commands::update_pet_regions,
            commands::get_configuration_status,
            commands::get_current_context,
            commands::list_sessions,
            commands::create_session,
            commands::set_active_session,
            commands::rename_session,
            commands::list_messages,
            commands::get_settings,
            commands::get_weather_settings,
            commands::get_weather,
            commands::refresh_weather,
            commands::cancel_weather_refresh,
            commands::save_weather_settings,
            commands::save_settings,
            commands::list_providers,
            commands::test_provider_connection,
            commands::create_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::set_active_chat_provider,
            commands::set_memory_provider,
            commands::list_personas,
            commands::create_persona,
            commands::generate_expressions,
            commands::cancel_expression_generation,
            commands::update_persona,
            commands::delete_persona,
            commands::set_active_persona
        ])
        .setup(|app| {
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "找不到 main 宠窗"))?;
            pet_region::enforce_frameless(&window).map_err(io::Error::other)?;
            let runtime = app.state::<runtime::Runtime>().inner().clone();
            tauri::async_runtime::spawn(jobs::run(
                runtime,
                std::sync::Arc::new(platform::windows::events::PetEvents(app.handle().clone())),
                std::sync::Arc::new(platform::windows::events::PetEvents(app.handle().clone())),
            ));
            let show = MenuItem::with_id(app, "show", "显示宠窗", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
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
                    "settings" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = desktop_windows::open(app, desktop_windows::Page::Settings).await {
                                eprintln!("打开设置失败：{error}");
                            }
                        });
                    }
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
        .build(tauri::generate_context!())
        .expect("启动颜喵失败")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                if let Err(error) = app.state::<runtime::Runtime>().shutdown() {
                    eprintln!("退出前保存桌宠状态失败：{error}");
                }
            }
        });
}
