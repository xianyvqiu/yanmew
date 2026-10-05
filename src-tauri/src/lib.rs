use std::io;
pub mod ai;
pub mod chat;
mod chat_capacity;
mod autostart;
mod commands;
mod desktop_windows;
mod config;
mod crypto;
mod db;
mod error;
mod events;
mod expressions;
pub mod embedding;
mod embedding_providers;
pub mod memory_index;
pub mod memory_search;
mod jobs;
pub mod memory;
mod location;
mod weather;
pub mod models;
pub mod notebook;
pub mod notebook_tools;
pub mod travel_journal_tools;
mod persona;
pub mod pet;
mod pet_region;
mod pet_position;
pub mod pet_store;
mod platform;
mod providers;
pub mod reminders;
pub mod reminder_extract;
mod runtime;
pub mod sessions;
mod user_profiles;
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
    if let Err(error) = app.state::<pet_position::WindowPosition>().ensure_visible(&window, true) {
        pet_position::report(app, &error);
    }
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
        .manage(pet_position::WindowPosition::default())
        .manage(desktop_windows::SettingsNavigation::default())
        .manage(std::sync::Mutex::new(commands::PetState::default()))
        .invoke_handler(tauri::generate_handler![
            autostart::get_autostart_status,
            autostart::set_autostart,
            commands::list_embedding_providers,
            commands::save_embedding_provider,
            commands::delete_embedding_provider,
            commands::set_active_embedding_provider,
            commands::test_embedding_provider,
            commands::get_embedding_status,
            commands::get_memory_generation_status,
            commands::list_memories,
            commands::get_memory,
            commands::save_memory,
            commands::preview_memory,
            commands::confirm_memory_preview,
            commands::cancel_memory_preview,
            commands::retry_short_memories,
            commands::get_notebook,
            commands::save_notebook,
            commands::focus_or_create_window,
            commands::take_reminder_navigation,
            commands::submit_chat,
            commands::get_last_chat_round,
            commands::chat_continue_reply,
            commands::get_chat_request,
            commands::cancel_chat,
            commands::list_reminder_occurrences,
            commands::get_reminder_occurrence,
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
            commands::list_user_profiles,
            commands::create_user_profile,
            commands::update_user_profile,
            commands::delete_user_profile,
            commands::set_session_user_profile,
            commands::create_session,
            commands::set_active_session,
            commands::rename_session,
            commands::delete_session,
            commands::update_chat_message,
            commands::delete_tail_message,
            commands::regenerate_last_reply,
            commands::list_messages,
            commands::get_settings,
            commands::get_settings_defaults,
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
            #[cfg(windows)]
            app.handle().plugin(tauri_plugin_autostart::Builder::new()
                .app_name(autostart::ENTRY_NAME).build())?;
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "找不到 main 宠窗"))?;
            pet_region::enforce_frameless(&window).map_err(io::Error::other)?;
            let runtime = app.state::<runtime::Runtime>().inner().clone();
            if let Err(error) = app.state::<pet_position::WindowPosition>().restore(&window, &runtime) {
                pet_position::report(app.handle(), &error);
            }
            tauri::async_runtime::spawn(jobs::run(
                runtime,
                std::sync::Arc::new(platform::windows::events::PetEvents(app.handle().clone())),
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
            if let tauri::RunEvent::WindowEvent { label, event, .. } = &event {
                if label == "main" {
                    if let Some(window) = app.get_webview_window("main") {
                        if let Err(error) = app.state::<pet_position::WindowPosition>()
                            .handle(&window, event, app.state::<runtime::Runtime>().inner()) {
                            pet_position::report(app, &error);
                        }
                    }
                }
            }
            // 原生销毁窗口不保证执行前端卸载回调，主动取消设置页的天气请求。
            if matches!(&event, tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { .. }, .. } if label == "settings") {
                app.state::<runtime::Runtime>().requests.cancel_key("weather");
            }
            if matches!(&event, tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { .. }, .. } if label == "memory") {
                crate::memory::management::discard(app.state::<runtime::Runtime>().inner());
            }
            if matches!(&event, tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { .. }, .. } if label == "chat") {
                if let Err(error) = crate::chat::interrupt_active(app.state::<runtime::Runtime>().inner()) {
                    eprintln!("关闭聊天窗前保存回复失败：{error}");
                }
            }
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                if let Err(error) = app.state::<pet_position::WindowPosition>()
                    .flush_exit(app.state::<runtime::Runtime>().inner()) {
                    pet_position::report(app, &error);
                }
                if let Err(error) = app.state::<runtime::Runtime>().shutdown() {
                    eprintln!("退出前保存桌宠状态失败：{error}");
                }
            }
        });
}
