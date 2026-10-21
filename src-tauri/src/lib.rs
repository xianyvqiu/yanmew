mod native_tools;
use std::io;
pub mod ai;
pub mod chat;
mod chat_capacity;
mod autostart;
mod commands;
mod desktop_windows;
mod config;
mod crypto;
pub mod currency;
mod real_money;
mod accessories;
mod souvenirs;
mod image_generation;
mod import_export;
pub mod game_plugins;
mod game_host;
mod db;
mod error;
mod events;
mod expressions;
mod pet_creation;
mod preset_personas;
mod foods;
mod skills;
mod companion_commands;
mod food_cursor;
pub mod embedding;
mod embedding_providers;
pub mod memory_index;
pub mod memory_search;
mod jobs;
pub mod lunar;
mod festivals;
mod proactive;
pub mod memory;
mod location;
mod weather;
pub mod web_search;
pub mod models;
pub mod notebook;
pub mod notebook_tools;
pub mod travel_journal_tools;
mod travel_journals;
pub mod travel;
pub mod letters;
mod persona;
pub mod pet;
mod pet_region;
mod pet_position;
mod pet_desktop;
pub mod pet_store;
mod platform;
mod providers;
mod provider_roles;
mod chat_images;
mod card_browser;
pub mod reminders;
mod reminder_email;
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
    let paths = platform::windows::paths::application_paths();
    // 先取得 data 使用权，再打开 SQLite 或启动任何后台任务。
    let instance = match paths.as_ref() {
        Ok(paths) => match platform::windows::instance::acquire(&paths.data_root) {
            Ok(Some(owner)) => Some(owner),
            Ok(None) => return,
            Err(error) => {
                use windows::{core::PCWSTR, Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK}};
                let message: Vec<u16> = format!("无法打开颜喵：{error}\0").encode_utf16().collect();
                let title: Vec<u16> = "颜喵 YanMew\0".encode_utf16().collect();
                unsafe { MessageBoxW(None, PCWSTR(message.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR); }
                return;
            }
        },
        Err(_) => None, // 原有初始化错误界面处理无效路径，不尝试访问数据库。
    };
    let runtime = runtime::Runtime::initialize(
        paths,
        std::sync::Arc::new(platform::windows::secrets::Dpapi),
        std::sync::Arc::new(platform::windows::time::WindowsClock),
        Box::new(platform::windows::time::WindowsRandom),
    );
    tauri::Builder::default()
        .manage(instance)
        .manage(runtime)
        .manage(food_cursor::Cursor::default())
        .manage(game_host::Host::default())
        .manage(pet_position::WindowPosition::default())
        .manage(pet_desktop::Desktop::default())
        .manage(desktop_windows::SettingsNavigation::default())
        .manage(desktop_windows::TravelNavigation::default())
        .manage(desktop_windows::LetterNavigation::default())
        .manage(std::sync::Mutex::new(commands::PetState::default()))
        .register_uri_scheme_protocol("yanmew-game", |context, request| {
            let label=context.webview_label();
            let result=if request.method()==tauri::http::Method::GET {game_host::resource(context.app_handle(),label,request.uri().path())}else{Err(error::AppError::new("PLUGIN_RESOURCE_DENIED","插件资源仅允许读取"))};
            let(status,mime,body)=match result{Ok((mime,body))=>(200,mime,body),Err(_)=>(403,"text/plain; charset=utf-8".into(),b"Plugin resource denied".to_vec())};
            tauri::http::Response::builder().status(status).header("Content-Type",mime).header("Content-Security-Policy",game_host::csp(label)).header("X-Content-Type-Options","nosniff").header("Cache-Control","no-store").body(body).unwrap_or_default()
        })
        .invoke_handler(game_host::command_gate(tauri::generate_handler![
            food_cursor::set_food_cursor,
            pet_desktop::get_pet_desktop_state,
            pet_desktop::set_pet_fullscreen_wander,
            pet_desktop::get_pet_dock_expression,
            pet_desktop::get_pet_walk_expression,
            pet_desktop::get_pet_lift_expression,
            companion_commands::get_food_expression, companion_commands::list_foods, companion_commands::get_food_preferences, companion_commands::save_food_preferences, companion_commands::buy_food, companion_commands::get_food_action, companion_commands::feed_food, companion_commands::list_skills, companion_commands::save_skill, companion_commands::delete_skill, companion_commands::bind_skill, companion_commands::get_skill_previews, companion_commands::resolve_skill_preview,
            commands::get_backup_export_options,
            commands::prepare_backup_import,
            commands::refresh_backup_import,
            commands::cancel_backup_import,
            commands::confirm_backup_import,
            commands::export_backup,
            commands::export_game_plugin_data,
            commands::get_image_generation_config,
            commands::save_image_generation_provider,
            commands::select_image_generation_provider,
            commands::set_image_generation_enabled,
            commands::delete_image_generation_provider,
            commands::get_accessory_craft_preview,
            commands::get_current_accessory_craft_preview,
            commands::confirm_accessory_craft,
            commands::cancel_accessory_craft,
            commands::list_accessories,
            commands::get_accessory_image,
            commands::save_accessory_placement,
            commands::move_accessory_layer,
            commands::upload_accessory_craft,
            commands::get_pet_mail_status,
            commands::get_pet_creation,
            commands::save_pet_artwork,
            commands::set_pet_creation_enabled,
            commands::get_pet_artwork_image,
            commands::upload_pet_artwork,
            commands::finish_pet_artwork,
            commands::generate_pet_artwork,
            commands::cancel_pet_artwork,
            commands::wear_accessory,
            commands::unwear_accessory,
            commands::sell_accessory,
            commands::delete_accessory,
            commands::list_travels,
            commands::list_festival_catalog,
            commands::get_calendar_month,
            commands::browse_cards,
            commands::list_custom_festivals,
            commands::get_custom_festival,
            commands::create_custom_festival,
            commands::update_custom_festival,
            commands::delete_custom_festival,
            commands::list_travel_journals,
            commands::get_travel_journal,
            commands::update_travel_journal,
            commands::delete_travel_journal,
            commands::get_journal_generation_status,
            commands::retry_journal_generation,
            commands::list_souvenirs,
            commands::get_souvenir,
            commands::set_souvenir_favorite,
            commands::delete_souvenir,
            commands::get_gift_generation_status,
            commands::retry_gift_generation,
            commands::get_current_travel,
            commands::get_travel_generation_status,
            commands::retry_travel_generation,
            commands::open_letter_window,
            commands::take_letter_navigation,
            commands::take_travel_navigation,
            commands::list_letters,
            commands::get_letter,
            commands::send_letter,
            commands::set_letter_flags,
            commands::delete_letter,
            commands::get_letter_generation_status,
            commands::retry_letter_generation,
            commands::list_game_plugins,
            commands::get_game_plugin,
            commands::preview_game_plugin_install,
            commands::confirm_game_plugin_install,
            commands::cancel_game_plugin_install,
            commands::set_game_plugin_enabled,
            commands::set_game_plugin_permission,
            commands::save_game_plugin_setting,
            commands::uninstall_game_plugin,
            commands::clear_game_plugin_data,
            commands::list_game_plugin_runs,
            commands::list_game_plugin_ledger,
            commands::open_game_plugin,
            commands::game_plugin_call,
            commands::get_web_search_settings,
            commands::save_web_search_provider,
            commands::delete_web_search_provider,
            commands::set_active_web_search_provider,
            commands::set_web_search_enabled,
            commands::test_web_search_provider,
            commands::open_web_source,
            platform::windows::wander::update_pet_wander_layout,
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
            commands::take_settings_navigation,
            commands::submit_chat,
            commands::get_chat_image,
            commands::get_last_chat_round,
            commands::chat_continue_reply,
            commands::get_chat_request,
            commands::cancel_chat,
            commands::list_reminder_occurrences,
            commands::get_reminder_occurrence,
            commands::polish_reminder_occurrence,
            commands::get_app_info,
            commands::request_app_exit,
            commands::get_reminder_email,
            commands::save_reminder_email,
            commands::send_reminder_email_code,
            commands::verify_reminder_email,
            commands::set_reminder_email_enabled,
            commands::retry_reminder_email,
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
            commands::set_pet_chat_open,
            commands::set_pet_artwork_layout,
            commands::update_pet_regions,
            commands::get_configuration_status,
            commands::get_current_context,
            commands::get_wallet,
            companion_commands::query_currency_ledger,
            real_money::commands::get_real_money_overview,
            real_money::commands::query_real_money,
            real_money::commands::mutate_real_money,
            real_money::commands::list_real_money_tasks,
            real_money::commands::get_real_money_calendar,
            real_money::commands::get_real_money_pending,
            real_money::commands::get_real_money_operation,
            commands::get_sign_in_status,
            commands::daily_sign_in,
            commands::get_session_intimacy,
            commands::list_sessions,
            commands::list_user_profiles,
            commands::create_user_profile,
            commands::update_user_profile,
            commands::delete_user_profile,
            commands::set_session_user_profile,
            commands::create_session,
            commands::set_active_session,
            commands::rename_session,
            commands::set_session_pinned,
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
            commands::get_provider_roles,
            commands::save_provider_role,
            commands::test_provider_connection,
            commands::create_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::set_active_chat_provider,
            commands::set_memory_provider,
            commands::list_personas,
            commands::get_chat_background,
            commands::save_chat_background,
            commands::create_persona,
            commands::generate_expressions,
            commands::save_persona_expressions,
            commands::cancel_expression_generation,
            commands::update_persona,
            commands::delete_persona,
            commands::set_active_persona
        ]))
        .setup(|app| {
            #[cfg(windows)]
            app.handle().plugin(tauri_plugin_autostart::Builder::new()
                .app_name(autostart::ENTRY_NAME).build())?;
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "找不到 main 宠窗"))?;
            pet_region::enforce_frameless(&window).map_err(io::Error::other)?;
            if let Some(instance) = app.state::<Option<platform::windows::instance::Instance>>().inner() {
                instance.publish(&window)?;
            }
            let runtime = app.state::<runtime::Runtime>().inner().clone();
            let plugin_app=app.handle().clone();let plugin_runtime=runtime.clone();
            tauri::async_runtime::spawn(async move { if let Err(error)=game_host::install::initialize(plugin_app.clone(),plugin_runtime).await {use tauri::Emitter;let _=plugin_app.emit("game_plugin_error",error);} plugin_app.state::<game_host::Host>().ready.store(true,std::sync::atomic::Ordering::Release); });
            let open_initial_chat = runtime.initial_session_created;
            if let Err(error) = app.state::<pet_position::WindowPosition>().restore(&window, &runtime) {
                pet_position::report(app.handle(), &error);
            }
            if let Err(error) = app.state::<pet_desktop::Desktop>().initialize(&window, &runtime) { pet_position::report(app.handle(), &error); }
            tauri::async_runtime::spawn(jobs::run(
                runtime,
                std::sync::Arc::new(platform::windows::events::PetEvents(app.handle().clone())),
                std::sync::Arc::new(platform::windows::events::PetEvents(app.handle().clone())),
                std::sync::Arc::new(platform::windows::events::PetEvents(app.handle().clone())),
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

            if open_initial_chat {
                let app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = desktop_windows::open(app, desktop_windows::Page::Chat).await {
                        // 会话已经提交；开窗失败不回滚或重新生成，仍可从聊天入口打开。
                        eprintln!("打开初始教程失败：{error}");
                    }
                });
            }

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
                game_host::shutdown(app);
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
