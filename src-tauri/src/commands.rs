use crate::pet_region::{self, Rect};
use rusqlite::OptionalExtension;

use crate::{
    error::{parse_id, AppError, AppResult},
    runtime::{Runtime, Store},
};
use tauri::{State, WebviewWindow};
use crate::web_search;
use crate::{game_host, game_plugins};

fn authorize_plugin_management(window:&WebviewWindow)->AppResult<()> {
    authorize(window)?;if window.label()!="settings"{return Err(AppError::new("PERMISSION_DENIED","请在设置窗口管理小游戏插件"))}Ok(())
}

/// 导出仅供可信设置窗口；不允许插件或聊天工具取得应用备份。
fn authorize_backup(window: &WebviewWindow) -> AppResult<()> {
    authorize(window)?;
    if window.label() != "settings" { return Err(AppError::new("PERMISSION_DENIED", "请在设置窗口管理备份")); }
    Ok(())
}
fn backup_files(window:&WebviewWindow)->AppResult<Box<dyn crate::platform::contracts::FileTransferPort>> {
    #[cfg(windows)] {Ok(Box::new(crate::platform::windows::files::BackupFiles{owner:window.hwnd().map_err(|_|AppError::storage())?.0 as isize}))}
    #[cfg(not(windows))] {let _=window;Err(AppError::new("PLATFORM_UNSUPPORTED","当前平台尚未接入系统文件保存"))}
}
#[tauri::command]
pub async fn export_backup(window:WebviewWindow,runtime:State<'_,Runtime>,input:crate::import_export::ExportRequest)->AppResult<Option<crate::import_export::ExportArchive>> {
    authorize_backup(&window)?;
    let transfer=backup_files(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::import_export::export(&worker,input,transfer.as_ref())).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn export_game_plugin_data(window:WebviewWindow,runtime:State<'_,Runtime>,ids:Vec<String>)->AppResult<Option<crate::import_export::ExportArchive>> {
    export_backup(window,runtime,crate::import_export::ExportRequest{kind:crate::import_export::PackageKind::PluginData,ids,include_sessions:false}).await
}
#[tauri::command]
pub async fn get_backup_export_options(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<crate::import_export::ExportOptions> {
    authorize_backup(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::import_export::options(&worker)).await.map_err(|_|AppError::storage())?
}

#[tauri::command]
pub async fn prepare_backup_import(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<Option<crate::import_export::restore::Preview>> {
    authorize_backup(&window)?;let transfer=backup_files(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::import_export::restore::prepare(&worker,transfer.as_ref())).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn refresh_backup_import(window:WebviewWindow,runtime:State<'_,Runtime>,preview_id:String)->AppResult<crate::import_export::restore::Preview> {
    authorize_backup(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::import_export::restore::refresh(&worker,&preview_id)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn cancel_backup_import(window:WebviewWindow,runtime:State<'_,Runtime>,preview_id:String)->AppResult<()> {
    authorize_backup(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::import_export::restore::cancel(&worker,&preview_id)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn confirm_backup_import(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,input:crate::import_export::restore::Confirm)->AppResult<crate::import_export::restore::Imported> {
    authorize_backup(&window)?;
    let host=app.state::<game_host::Host>();let _operation=host.operations.lock().await;
    crate::import_export::restore::check(runtime.inner(),&input)?;
    for id in crate::import_export::restore::plugins_to_close(runtime.inner(),&input.preview_id)? {game_host::close_plugin(&app,&id)?;}
    let preview=crate::import_export::restore::current_preview(runtime.inner(),&input.preview_id)?;
    for index in 0..preview.plugins.len() {
        let worker=runtime.inner().clone();let token=input.preview_id.clone();
        let job=tauri::async_runtime::spawn_blocking(move||crate::import_export::restore::plugin_job(&worker,&token,index)).await.map_err(|_|AppError::storage())??;
        if let Some((target,package,mut snapshot))=job {
            if snapshot.storage_version!=package.manifest.storage_version && (!snapshot.settings.is_empty()||!snapshot.data.is_empty()) {
                snapshot=game_host::migrate(&app,runtime.inner(),package.clone(),snapshot).await?;
            }else{snapshot.storage_version=package.manifest.storage_version;game_plugins::validate_snapshot(&package.manifest,&snapshot)?;}
            crate::import_export::restore::accept_plugin(runtime.inner(),&input.preview_id,target,&package,snapshot)?;
        }
    }
    let full=preview.kind==crate::import_export::PackageKind::Full;
    let previous=runtime.read(|store,_|Ok(store.context_revision))?;
    let worker=runtime.inner().clone();
    let result=tauri::async_runtime::spawn_blocking(move||crate::import_export::restore::confirm(&worker,input)).await.map_err(|_|AppError::storage())??;
    // 已提交的数据不能因刷新通知或后台重试失败而被报告成未导入。
    if full {let _=runtime.memory_index.reset();let _=runtime.memory.retry(None);}
    runtime.memory.wake.notify_one();
    for event in ["configuration_changed","game_plugins_changed",crate::events::WALLET_CHANGED,crate::events::REMINDERS_CHANGED,crate::events::MEMORY_GENERATION_CHANGED,crate::events::NOTEBOOK_CHANGED,crate::events::EMBEDDING_CHANGED,crate::events::IMAGE_GENERATION_CONFIG_CHANGED] {let _=app.emit(event,());}
    if let Ok(context)=runtime.read(|store,_|crate::persona::context(&store.connection,store.context_revision)) {
        if context.context_revision!=previous {let _=app.emit("window_context_changed",&context);}
        if let Some(session)=context.session_id {
            for event in [crate::events::TRAVEL_CHANGED,crate::events::LETTERS_CHANGED,crate::events::ACCESSORIES_CHANGED,"chat-messages-changed"] {let _=app.emit(event,serde_json::json!({"sessionId":session}));}
        }
    }
    Ok(result)
}
#[derive(serde::Serialize)]#[serde(rename_all="camelCase")]
pub struct PluginDetail {pub plugin:game_plugins::Plugin,pub manifest:Option<game_plugins::Manifest>,pub permissions:Vec<game_plugins::Permission>,pub usage:game_plugins::PrivateSnapshot}
#[tauri::command]
pub async fn list_game_plugins(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<Vec<game_plugins::Plugin>> {
    authorize_plugin_management(&window)?;storage(runtime.inner().clone(),|s,_|game_plugins::list(&s.connection)).await
}
#[tauri::command]
pub async fn get_game_plugin(window:WebviewWindow,runtime:State<'_,Runtime>,id:String)->AppResult<PluginDetail> {
    authorize_plugin_management(&window)?;let worker=runtime.inner().clone();let paths=worker.paths.clone()?;
    tauri::async_runtime::spawn_blocking(move||{let (plugin,permissions,usage)=worker.read(|s,_|Ok((game_plugins::get(&s.connection,&id)?,game_plugins::permissions(&s.connection,&id)?,game_plugins::snapshot(&s.connection,&id)?)))?;
        let manifest=if plugin.package_status=="available"{Some(game_plugins::package::load(&game_plugins::package::root(&paths),&plugin)?.manifest)}else{None};Ok(PluginDetail{plugin,manifest,permissions,usage})}).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn preview_game_plugin_install(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,filename:String,bytes_base64:String)->AppResult<game_host::install::Preview> {
    authorize_plugin_management(&window)?;let app=app.clone();let runtime=runtime.inner().clone();tauri::async_runtime::spawn_blocking(move||game_host::install::preview(app.state::<game_host::Host>().inner(),&runtime,&filename,&bytes_base64)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn confirm_game_plugin_install(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,token:String,choices:Vec<game_plugins::PermissionChoice>)->AppResult<game_plugins::Plugin> {
    authorize_plugin_management(&window)?;let result=game_host::install::confirm(&app,runtime.inner(),&token,choices).await?;let _=app.emit("game_plugins_changed",());Ok(result)
}
#[tauri::command]
pub fn cancel_game_plugin_install(window:WebviewWindow,app:AppHandle,token:String)->AppResult<()> {authorize_plugin_management(&window)?;game_host::install::cancel(app.state::<game_host::Host>().inner(),&token)}
#[tauri::command]
pub async fn set_game_plugin_enabled(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,version:String,enabled:bool)->AppResult<()> {
    authorize_plugin_management(&window)?;let host=app.state::<game_host::Host>();let _operation=host.operations.lock().await;
    runtime.read(|s,_|game_plugins::set_enabled(&s.connection,&id,&version,enabled,runtime.clock.sample()?.utc))?;
    if !enabled {game_host::close_plugin(&app,&id)?;}let _=app.emit("game_plugins_changed",());Ok(())
}
#[tauri::command]
pub async fn set_game_plugin_permission(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,version:String,choice:game_plugins::PermissionChoice)->AppResult<()> {
    authorize_plugin_management(&window)?;let host=app.state::<game_host::Host>();let _operation=host.operations.lock().await;
    storage(runtime.inner().clone(),move|s,_|game_plugins::decide(&mut s.connection,&id,&version,&choice,chrono::Utc::now())).await?;let _=app.emit("game_plugins_changed",());Ok(())
}
#[tauri::command]
pub async fn save_game_plugin_setting(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,version:String,key:String,value:serde_json::Value)->AppResult<()> {
    authorize_plugin_management(&window)?;let host=app.state::<game_host::Host>();let _operation=host.operations.lock().await;let worker=runtime.inner().clone();let paths=worker.paths.clone()?;
    tauri::async_runtime::spawn_blocking(move||{let p=worker.read(|s,_|game_plugins::get(&s.connection,&id))?;if p.installed_version!=version{return Err(game_plugins::denied())}let m=game_plugins::package::load(&game_plugins::package::root(&paths),&p)?.manifest;worker.read(|s,_|game_plugins::save_setting(&s.connection,&m,&key,&value,worker.clock.sample()?.utc))}).await.map_err(|_|AppError::storage())??;let _=app.emit("game_plugins_changed",());Ok(())
}
#[tauri::command]
pub async fn uninstall_game_plugin(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,version:String,clear_data:bool)->AppResult<()> {
    authorize_plugin_management(&window)?;let host=app.state::<game_host::Host>();let _operation=host.operations.lock().await;
    runtime.read(|s,_|{let p=game_plugins::get(&s.connection,&id)?;if p.is_bundled||p.installed_version!=version{return Err(AppError::new("PLUGIN_UNINSTALL_DENIED","随包插件不可卸载，或版本已改变"))}Ok(())})?;
    game_host::close_plugin(&app,&id)?;storage(runtime.inner().clone(),move|s,_|game_plugins::uninstall(&mut s.connection,&id,&version,clear_data,chrono::Utc::now())).await?;let _=app.emit("game_plugins_changed",());Ok(())
}
#[tauri::command]
pub async fn clear_game_plugin_data(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,version:String)->AppResult<()> {
    authorize_plugin_management(&window)?;let host=app.state::<game_host::Host>();let _operation=host.operations.lock().await;
    runtime.read(|s,_|{if game_plugins::get(&s.connection,&id)?.installed_version!=version{return Err(game_plugins::denied())}Ok(())})?;
    game_host::close_plugin(&app,&id)?;storage(runtime.inner().clone(),move|s,_|game_plugins::clear_private_data(&mut s.connection,&id,&version)).await?;let _=app.emit("game_plugins_changed",());Ok(())
}
#[tauri::command]
pub async fn list_game_plugin_runs(window:WebviewWindow,runtime:State<'_,Runtime>,id:String,before:Option<String>,limit:usize)->AppResult<Vec<game_plugins::runs::Run>> {authorize_plugin_management(&window)?;storage(runtime.inner().clone(),move|s,_|game_plugins::runs::history(&s.connection,&id,before,limit)).await}
#[tauri::command]
pub async fn list_game_plugin_ledger(window:WebviewWindow,runtime:State<'_,Runtime>,id:String,before:Option<String>,limit:usize)->AppResult<Vec<game_plugins::runs::LedgerEntry>> {authorize_plugin_management(&window)?;storage(runtime.inner().clone(),move|s,_|game_plugins::runs::ledger(&s.connection,&id,before,limit)).await}
#[tauri::command]
pub async fn open_game_plugin(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String)->AppResult<String> {authorize_plugin_management(&window)?;game_host::open(app,runtime.inner().clone(),id).await}
#[tauri::command]
pub async fn game_plugin_call(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,method:String,args:serde_json::Value)->AppResult<serde_json::Value> {game_host::call(app,runtime.inner().clone(),window.label().into(),method,args).await}

fn authorize_search_settings(window:&WebviewWindow)->AppResult<()> {
    authorize(window)?;
    if window.label()!="settings" {return Err(AppError::new("PERMISSION_DENIED","请在设置窗口管理搜索提供商"));}
    Ok(())
}
#[tauri::command]
pub async fn get_web_search_settings(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<web_search::View> {
    authorize(&window)?;
    storage(runtime.inner().clone(),|s,_|web_search::view(&s.connection)).await
}
#[tauri::command]
pub async fn save_web_search_provider(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:Option<String>,input:web_search::Input)->AppResult<()> {
    authorize_search_settings(&window)?;
    storage(runtime.inner().clone(),move|s,secrets|web_search::save(&mut s.connection,id,input,secrets)).await?;
    let _=app.emit("configuration_changed",());Ok(())
}
#[tauri::command]
pub async fn delete_web_search_provider(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String)->AppResult<()> {
    authorize_search_settings(&window)?;
    storage(runtime.inner().clone(),move|s,_|web_search::delete(&mut s.connection,&id)).await?;
    let _=app.emit("configuration_changed",());Ok(())
}
#[tauri::command]
pub async fn set_active_web_search_provider(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String)->AppResult<()> {
    authorize_search_settings(&window)?;
    storage(runtime.inner().clone(),move|s,_|web_search::select(&mut s.connection,&id)).await?;
    let _=app.emit("configuration_changed",());Ok(())
}
#[tauri::command]
pub async fn set_web_search_enabled(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,enabled:bool)->AppResult<()> {
    authorize_search_settings(&window)?;
    storage(runtime.inner().clone(),move|s,_|web_search::enable(&mut s.connection,enabled)).await?;
    let _=app.emit("configuration_changed",());Ok(())
}
#[tauri::command]
pub async fn test_web_search_provider(window:WebviewWindow,runtime:State<'_,Runtime>,id:String)->AppResult<usize> {
    authorize_search_settings(&window)?;
    let runtime=runtime.inner().clone();let selected=id.clone();
    let snapshot=storage(runtime.clone(),move|s,secrets|web_search::snapshot(&s.connection,secrets,Some(&selected))).await?.ok_or_else(AppError::not_found)?;
    let lease=runtime.requests.start(format!("web-search-test:{id}"),0)?;
    let client=runtime.ai.clone()?;
    let count=snapshot.test(&client,&lease.cancel).await?;
    lease.cancel.check()?;
    storage(runtime,move|s,_|snapshot.check(&s.connection)).await?;
    Ok(count)
}
#[tauri::command]
pub async fn open_web_source(window:WebviewWindow,url:String)->AppResult<()> {
    authorize(&window)?;
    let url=crate::web_search::source_url(&url)?;
    tauri::async_runtime::spawn_blocking(move||crate::platform::windows::links::open(&url)).await.map_err(|_|AppError::storage())?
}
use crate::memory::management as memory_management;

#[tauri::command]
pub async fn list_memories(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    context: memory_management::Context,
    kind: memory_management::Kind,
    after_id: Option<String>,
    limit: Option<u32>,
) -> AppResult<memory_management::Page> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        memory_management::list(&worker, context, kind, after_id, limit.unwrap_or(30))
    })
    .await
    .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn get_memory(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    context: memory_management::Context,
    kind: memory_management::Kind,
    id: String,
) -> AppResult<memory_management::Detail> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || memory_management::get(&worker, context, kind, id))
        .await
        .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn save_memory(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    context: memory_management::Context,
    expected: memory_management::Detail,
    title: Option<String>,
    content: String,
) -> AppResult<memory_management::Detail> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        memory_management::save(&worker, context, expected, title, content)
    })
    .await
    .map_err(|_| AppError::storage())??;
    notify_memory_management(&app, result.record.kind);
    Ok(result)
}
#[tauri::command]
pub async fn preview_memory(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    context: memory_management::Context,
    kind: memory_management::Kind,
    id: String,
) -> AppResult<memory_management::Preview> {
    authorize(&window)?;
    memory_management::preview(runtime.inner().clone(), context, kind, id).await
}
#[tauri::command]
pub async fn confirm_memory_preview(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    context: memory_management::Context,
    preview_id: String,
    title: Option<String>,
    content: String,
) -> AppResult<memory_management::Detail> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        memory_management::confirm(&worker, context, preview_id, title, content)
    })
    .await
    .map_err(|_| AppError::storage())??;
    notify_memory_management(&app, result.record.kind);
    Ok(result)
}
#[tauri::command]
pub async fn cancel_memory_preview(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    context: memory_management::Context,
) -> AppResult<()> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || memory_management::cancel(&worker, context))
        .await
        .map_err(|_| AppError::storage())?
}
fn notify_memory_management(app: &AppHandle, kind: memory_management::Kind) {
    let _ = app.emit(crate::events::MEMORY_GENERATION_CHANGED, ());
    if kind == memory_management::Kind::Long {
        let _ = app.emit(crate::events::EMBEDDING_CHANGED, ());
    }
}

#[tauri::command]
pub async fn list_embedding_providers(window: WebviewWindow, runtime: State<'_, Runtime>) -> AppResult<Vec<crate::embedding_providers::Provider>> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s,_| crate::embedding_providers::list(&s.connection)).await
}
#[tauri::command]
pub async fn save_embedding_provider(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, id: Option<String>, input: crate::providers::ProviderInput) -> AppResult<crate::embedding_providers::Provider> {
    authorize(&window)?;
    let id = id.as_deref().map(parse_id).transpose()?;
    let result = storage(runtime.inner().clone(), move |s,secrets| crate::embedding_providers::save(&mut s.connection,id,input,secrets)).await?;
    if result.is_active { runtime.memory_index.reset()?; runtime.memory.wake.notify_one(); }
    let _ = app.emit("configuration_changed", ());
    let _ = app.emit(crate::events::EMBEDDING_CHANGED, ());
    Ok(result)
}
#[tauri::command]
pub async fn delete_embedding_provider(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, id: String) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    storage(runtime.inner().clone(),move |s,_| crate::embedding_providers::delete(&mut s.connection,id)).await?;
    let _ = app.emit("configuration_changed", ());
    Ok(())
}
#[tauri::command]
pub async fn set_active_embedding_provider(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, id: String) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    storage(runtime.inner().clone(),move |s,_| crate::embedding_providers::select(&mut s.connection,id)).await?;
    runtime.memory_index.reset()?;
    runtime.memory.wake.notify_one();
    let _ = app.emit("configuration_changed", ());
    let _ = app.emit(crate::events::EMBEDDING_CHANGED, ());
    Ok(())
}
#[tauri::command]
pub async fn get_embedding_status(window: WebviewWindow, runtime: State<'_, Runtime>) -> AppResult<crate::memory_index::View> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || crate::memory_index::view(&worker)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn test_embedding_provider(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, id: String) -> AppResult<usize> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let runtime = runtime.inner().clone();
    let stamp = storage(runtime.clone(), move |s,_| crate::embedding_providers::stamp(&s.connection,Some(id))).await?;
    if stamp.kind == "local" {
        let engine = runtime.embedding.clone();
        tauri::async_runtime::spawn_blocking(move || engine.reset_failure()).await.map_err(|_|AppError::storage())?;
    }
    let lease = runtime.requests.start(format!("embedding-test:{id}"), 0)?;
    let vector = crate::embedding::generate(&runtime,&stamp,"这是一条用于测试本地或联网向量模型的记忆。".into(),false,&lease.cancel).await?;
    let key = vector.model_key.clone();
    let active = storage(runtime.clone(),move |s,_| {
        if crate::embedding_providers::stamp(&s.connection,Some(id))? != stamp { return Err(AppError::new("CONTEXT_CHANGED", "Embedding 配置已改变，请重新测试")); }
        if crate::embedding_providers::stamp(&s.connection,None)? != stamp { return Ok(false); }
        s.connection.execute("DELETE FROM long_memory_embeddings WHERE model_key!=?1",[key])?;
        Ok(true)
    }).await?;
    if active { runtime.memory_index.reset()?; runtime.memory_index.observed(&vector.model_key)?; runtime.memory.wake.notify_one(); }
    let _ = app.emit(crate::events::EMBEDDING_CHANGED, ());
    Ok(vector.values.len())
}

struct ChatToolEvents(AppHandle);
#[derive(serde::Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ChatContextInput {persona_id:String,session_id:String,context_revision:u64}

#[tauri::command]
pub async fn update_chat_message(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,message_id:String,expected_content:String,content:String)->AppResult<crate::models::Message> {
    authorize(&window)?;
    let (persona,session,message)=(parse_id(&context.persona_id)?,parse_id(&context.session_id)?,parse_id(&message_id)?);
    let worker=runtime.inner().clone();
    let saved=tauri::async_runtime::spawn_blocking(move||crate::sessions::messages::edit(&worker,persona,session,context.context_revision,message,&expected_content,&content)).await.map_err(|_|AppError::storage())??;
    notify_message_change(&app,session);
    Ok(saved)
}

#[tauri::command]
pub async fn delete_tail_message(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,message_id:String,expected_content:String)->AppResult<()> {
    authorize(&window)?;
    let (persona,session,message)=(parse_id(&context.persona_id)?,parse_id(&context.session_id)?,parse_id(&message_id)?);
    let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::sessions::messages::delete_tail(&worker,persona,session,context.context_revision,message,&expected_content)).await.map_err(|_|AppError::storage())??;
    crate::chat_images::cleanup_unused(runtime.inner());
    notify_message_change(&app,session);
    Ok(())
}

fn notify_message_change(app:&AppHandle,session:i64) {
    let _=app.emit("chat-messages-changed",serde_json::json!({"sessionId":session.to_string()}));
    let _=app.emit("configuration_changed",());
    let _=app.emit(crate::events::MEMORY_GENERATION_CHANGED,());
    let _=app.emit(crate::events::EMBEDDING_CHANGED,());
}

#[tauri::command]
pub async fn regenerate_last_reply(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,round_id:String)->AppResult<crate::chat::RequestSnapshot> {
    authorize(&window)?;
    let (persona,session,round)=(parse_id(&context.persona_id)?,parse_id(&context.session_id)?,parse_id(&round_id)?);
    let worker=runtime.inner().clone();let client=worker.ai.clone()?;
    let (prepared,receipt,memory)=tauri::async_runtime::spawn_blocking(move||crate::chat::regenerate(&worker,persona,session,context.context_revision,round)).await.map_err(|_|AppError::storage())??;
    tauri::async_runtime::spawn(async move {
        let result=crate::chat::run_regenerated(&client,prepared,memory,|event|{let _=app.emit("chat-reply",&event);}).await;
        if result.is_ok() {notify_message_change(&app,session);}
    });
    Ok(receipt)
}

#[tauri::command]
pub async fn get_last_chat_round(window:WebviewWindow,runtime:State<'_,Runtime>,context:ChatContextInput)->AppResult<Option<crate::models::RoundSnapshot>> {
    authorize(&window)?;
    let (persona,session)=(parse_id(&context.persona_id)?,parse_id(&context.session_id)?);
    storage(runtime.inner().clone(),move|store,_| {
        validate_persona_context(store,persona,context.context_revision)?;
        let current=crate::persona::context(&store.connection,store.context_revision)?;
        if current.session_id!=Some(session.to_string()) {return Err(AppError::new("CONTEXT_CHANGED","当前会话已经改变"));}
        crate::sessions::last_round(&store.connection,persona,session)
    }).await
}

#[tauri::command]
pub async fn chat_continue_reply(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,round_id:String)->AppResult<crate::chat::RequestSnapshot> {
    authorize(&window)?;
    let (persona,session,round)=(parse_id(&context.persona_id)?,parse_id(&context.session_id)?,parse_id(&round_id)?);
    let worker=runtime.inner().clone();let client=worker.ai.clone()?;
    let title_runtime = worker.clone();
    let revision = context.context_revision;
    let (mut prepared,receipt)=tauri::async_runtime::spawn_blocking(move||crate::chat::continue_reply(&worker,persona,session,context.context_revision,round)).await.map_err(|_|AppError::storage())??;
    prepared.set_tool_events(std::sync::Arc::new(ChatToolEvents(app.clone())));
    tauri::async_runtime::spawn(async move {
        if let Ok(saved) = crate::chat::run_persisted(&client,prepared,|event| {let _=app.emit("chat-reply",&event);}).await {
            notify_first_title(&app,title_runtime,persona,session,revision,&saved).await;
        }
    });
    Ok(receipt)
}
impl crate::events::ReminderEventSink for ChatToolEvents {
    fn reminders_changed(&self) { let _ = self.0.emit(crate::events::REMINDERS_CHANGED, ()); }
    fn reminder_failed(&self, error: &AppError) { let _ = self.0.emit(crate::events::REMINDER_ERROR, error); }
}
impl crate::events::ChatToolEventSink for ChatToolEvents {
    fn accessory_preview_changed(&self,session:&str,preview:&str) {
        let _=self.0.emit(crate::events::ACCESSORY_CRAFT_PREVIEW_CHANGED,serde_json::json!({"sessionId":session,"previewId":preview}));
    }
    fn notebook_changed(&self) { let _ = self.0.emit(crate::events::NOTEBOOK_CHANGED, ()); }
    fn persona_updated(&self, update: &crate::events::PersonaUpdated) {
        let _ = self.0.emit("configuration_changed", ());
        let _ = self.0.emit(crate::events::PERSONA_UPDATED, update);
    }
    fn persona_evolution_failed(&self, error: &AppError) {
        let _ = self.0.emit(crate::events::PET_ACTIVITY_ERROR, error);
    }
}

#[tauri::command]
pub async fn get_notebook(window: WebviewWindow, runtime: State<'_, Runtime>, persona_id: String, session_id: String, context_revision: u64) -> AppResult<crate::notebook::NotebookView> {
    authorize(&window)?;
    let (persona_id, session_id) = (parse_id(&persona_id)?, parse_id(&session_id)?);
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || crate::notebook::current(&runtime, persona_id, session_id, context_revision)).await.map_err(|_| AppError::storage())?
}

#[tauri::command]
pub async fn save_notebook(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, persona_id: String, session_id: String,
    context_revision: u64, expected: Option<crate::models::Notebook>, content: String) -> AppResult<crate::notebook::NotebookView> {
    authorize(&window)?;
    let (persona_id, session_id) = (parse_id(&persona_id)?, parse_id(&session_id)?);
    let runtime = runtime.inner().clone();
    let saved = tauri::async_runtime::spawn_blocking(move || crate::notebook::save_current(&runtime, persona_id, session_id, context_revision, expected, content)).await.map_err(|_| AppError::storage())??;
    let _ = app.emit(crate::events::NOTEBOOK_CHANGED, ());
    Ok(saved)
}

#[tauri::command]
pub async fn submit_chat(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>,
    persona_id: String, session_id: String, context_revision: u64, input: String, images:Option<Vec<crate::chat_images::Input>>,
) -> AppResult<crate::chat::RequestSnapshot> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let runtime = runtime.inner().clone();
    let client = runtime.ai.clone()?;
    let title_runtime = runtime.clone();
    let (mut prepared, receipt) = tauri::async_runtime::spawn_blocking(move ||
        crate::chat::submit_images(&runtime, persona_id, session_id, context_revision, input,images.unwrap_or_default())
    ).await.map_err(|_| AppError::storage())??;
    prepared.set_tool_events(std::sync::Arc::new(ChatToolEvents(app.clone())));
    tauri::async_runtime::spawn(async move {
        // 界面可能已经关闭；事件发送失败不撤销已经提交的数据库事务。
        let saved = crate::chat::run_persisted(&client, prepared, |event| {
            let _ = app.emit("chat-reply", &event);
        }).await;
        if let Ok(saved) = saved {
            notify_first_title(&app,title_runtime,persona_id,session_id,context_revision,&saved).await;
        }
    });
    Ok(receipt)
}

/// 标题失败保留已完成聊天，不切换健康状态；成功仅刷新会话资料。
async fn notify_first_title(
    app: &AppHandle,
    runtime: Runtime,
    persona_id: i64,
    session_id: i64,
    revision: u64,
    saved: &crate::models::RoundSnapshot,
) {
    if saved.round.status != crate::models::ChatRoundStatus::Completed { return; }
    let Ok(round_id) = parse_id(&saved.round.id) else { return; };
    if matches!(crate::sessions::generate_first_title(runtime,persona_id,session_id,revision,round_id).await, Ok(true)) {
        let _ = app.emit("configuration_changed", ());
    }
}

#[tauri::command]
pub async fn get_chat_request(window: WebviewWindow, runtime: State<'_, Runtime>,
    persona_id: String, session_id: String, context_revision: u64,
) -> AppResult<Option<crate::chat::RequestSnapshot>> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || crate::chat::current(&runtime, persona_id, session_id, context_revision))
        .await.map_err(|_| AppError::storage())?
}

#[tauri::command]
pub async fn cancel_chat(window: WebviewWindow, runtime: State<'_, Runtime>,
    persona_id: String, session_id: String, request_id: String, round_id: String,
) -> AppResult<crate::models::RoundSnapshot> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let request_id = parse_id(&request_id)? as u64;
    let round_id = parse_id(&round_id)?;
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || crate::chat::cancel(&runtime, persona_id, session_id, request_id, round_id))
        .await.map_err(|_| AppError::storage())?
}

#[tauri::command]
pub async fn list_reminders(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    after_id: Option<String>,
    limit: u32,
) -> AppResult<Vec<crate::models::Reminder>> {
    authorize(&window)?;
    let after_id = after_id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::list_rules(&s.connection, after_id, limit)
    })
    .await
}

#[tauri::command]
pub async fn create_reminder(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    input: crate::reminders::ReminderInput,
) -> AppResult<crate::models::Reminder> {
    authorize(&window)?;
    let clock = runtime.clock.clone();
    let result = storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::save_rule(
            &mut s.connection,
            None,
            input,
            "manual",
            clock.sample()?.utc,
            &chrono::Local,
        )
    })
    .await?;
    let _ = app.emit(crate::events::REMINDERS_CHANGED, ());
    Ok(result)
}

#[tauri::command]
pub async fn update_reminder(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
    input: crate::reminders::ReminderInput,
) -> AppResult<crate::models::Reminder> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let clock = runtime.clock.clone();
    let result = storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::save_rule(
            &mut s.connection,
            Some(id),
            input,
            "manual",
            clock.sample()?.utc,
            &chrono::Local,
        )
    })
    .await?;
    let _ = app.emit(crate::events::REMINDERS_CHANGED, ());
    Ok(result)
}

#[tauri::command]
pub async fn delete_reminder(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::delete_rule(&s.connection, id)
    })
    .await?;
    let _ = app.emit(crate::events::REMINDERS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub async fn get_missed_reminder_summary(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    after_id: Option<String>,
) -> AppResult<crate::reminders::MissedReminderSummary> {
    authorize(&window)?;
    let after_id = after_id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::missed_summary(&s.connection, after_id)
    })
    .await
}

#[tauri::command]
pub async fn ack_missed_reminder_summary(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    ids: Vec<String>,
) -> AppResult<()> {
    acknowledge_reminder_occurrences(
        window,
        app,
        runtime,
        ids,
        crate::models::ReminderDeliveryState::Missed,
    )
    .await
}

#[tauri::command]
pub async fn list_reminder_occurrences(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    reminder_id: Option<String>,
    delivery_state: Option<crate::models::ReminderDeliveryState>,
    after_id: Option<String>,
    limit: u32,
) -> AppResult<crate::models::ReminderOccurrencePage> {
    authorize(&window)?;
    let reminder_id = reminder_id.as_deref().map(parse_id).transpose()?;
    let after_id = after_id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::list_occurrences(
            &s.connection,
            reminder_id,
            delivery_state,
            after_id,
            limit,
        )
    })
    .await
}

#[tauri::command]
pub async fn acknowledge_reminder_occurrences(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    ids: Vec<String>,
    delivery_state: crate::models::ReminderDeliveryState,
) -> AppResult<()> {
    authorize(&window)?;
    if ids.len() > 200 {
        return Err(AppError::validation("ids", "每次最多确认 200 条提醒"));
    }
    let ids = ids
        .iter()
        .map(|id| parse_id(id))
        .collect::<AppResult<Vec<_>>>()?;
    storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::acknowledge(&mut s.connection, &ids, delivery_state)
    })
    .await?;
    let _ = app.emit(crate::events::REMINDERS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub async fn update_reminder_occurrence_status(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
    status: crate::models::ReminderStatus,
) -> AppResult<crate::models::ReminderOccurrence> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let clock = runtime.clock.clone();
    let result = storage(runtime.inner().clone(), move |s, _| {
        crate::reminders::update_occurrence_status(
            &mut s.connection,
            id,
            status,
            clock.sample()?.utc,
        )
    })
    .await?;
    let _ = app.emit(crate::events::REMINDERS_CHANGED, ());
    Ok(result)
}

#[tauri::command]
pub async fn complete_reminder_occurrence(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<crate::models::ReminderOccurrence> {
    update_reminder_occurrence_status(
        window,
        app,
        runtime,
        id,
        crate::models::ReminderStatus::Done,
    )
    .await
}

#[tauri::command]
pub async fn get_pet_activity(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<crate::pet::controller::View> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    storage(runtime.inner().clone(), move |s, p| worker.settle_pet(s, p)).await
}
#[tauri::command]
pub async fn interact_pet(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    input: crate::pet::controller::InteractionInput,
) -> AppResult<crate::pet::controller::View> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    let view = tauri::async_runtime::spawn_blocking(move || worker.pet_interact(input))
        .await
        .map_err(|_| AppError::storage())??;
    let _ = app.emit(crate::events::PET_ACTIVITY_CHANGED, &view);
    Ok(view)
}
#[tauri::command]
pub async fn set_pet_dragging(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    dragging: bool,
) -> AppResult<()> {
    authorize(&window)?;
    if window.label() != "main" {
        return Err(AppError::new("PERMISSION_DENIED", "只有主宠窗可以更新拖动状态"));
    }
    let position = window.state::<crate::pet_position::WindowPosition>().inner().clone();
    if dragging { position.begin_drag(&window)?; }
    let worker = runtime.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || worker.pet_visibility(None, Some(dragging)))
        .await
        .unwrap_or_else(|_| Err(AppError::storage()));
    if !dragging {
        let saved = position.finish_drag(&window, runtime.inner().clone()).await;
        result?;
        return saved;
    }
    result
}
#[tauri::command]
pub async fn refresh_pet_provider(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<()> {
    authorize(&window)?;
    let id = storage(runtime.inner().clone(), |s, _| {
        use rusqlite::OptionalExtension;
        s.connection
            .query_row("SELECT id FROM provider_effective WHERE is_active=1", [], |r| {
                r.get::<_, i64>(0)
            })
            .optional()?
            .ok_or_else(|| {
                AppError::new(
                    "PROVIDER_UNAVAILABLE",
                    "暂无聊天提供商，请先在设置中添加并选择",
                )
            })
    })
    .await?;
    check_provider(runtime.inner().clone(), id).await
}

async fn check_provider(runtime: Runtime, id: i64) -> AppResult<()> {
    let (config, token) = storage(runtime.clone(), move |s, p| {
        let config = crate::providers::connection_test_config(&s.connection, id, p)?;
        let active =
            s.connection
                .query_row("SELECT is_active FROM provider_effective WHERE id=?1", [id], |r| {
                    r.get::<_, bool>(0)
                })?;
        let token = if active {
            Some(s.pet.health(&s.connection, p)?)
        } else {
            None
        };
        Ok((config, token))
    })
    .await?;
    let client=runtime.ai.clone()?;
    let result = crate::providers::test_connection(&client, crate::providers::ConnectionTest {
        base_url:config.base_url.clone(),model:config.model.clone(),api_key:config.api_key.clone(),
        max_output_tokens:config.max_output_tokens,context_window_override:config.context_window_override,
    }).await;
    if result.is_ok() {client.discover_capacity(&config).await;}
    let health_result = result.clone();
    let worker = runtime.clone();
    storage(runtime, move |s, p| {
        s.pet.health(&s.connection, p)?;
        let testing_chat=token.is_some();
        if let Some(token) = token {
            if !s.pet.complete_health(&token, &health_result) {
                return Err(AppError::new("CONTEXT_CHANGED", "提供商配置或恢复状态已改变，旧连接测试结果已忽略"));
            }
        }
        // 只对实际记忆提供商的成功测试解除失败等待，不重复请求不相关模型。
        if health_result.is_ok() {
            let selected: Option<i64> = s.connection.query_row(
                "SELECT id FROM provider_effective WHERE memory_is_active=1 OR is_active=1 ORDER BY memory_is_active DESC LIMIT 1",
                [], |row| row.get(0),
            ).optional()?;
            if selected == Some(id) { worker.memory.retry(None)?; }
            if testing_chat {worker.travel_gifts.provider_ready()?;worker.travel_journals.provider_ready()?;}
        }
        Ok(())
    })
    .await?;
    result
}

#[tauri::command]
pub async fn generate_expressions(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<crate::expressions::ExpressionLibrary> {
    authorize(&window)?;
    let result =
        crate::expressions::generate_persona_expressions(runtime.inner().clone(), parse_id(&id)?)
            .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(result)
}
#[tauri::command]
pub async fn cancel_expression_generation(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    runtime.requests.cancel_key(&format!("expressions:{id}"));
    Ok(())
}

#[tauri::command]
pub async fn get_current_context(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<crate::persona::Context> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::persona::context(&s.connection, s.context_revision)
    })
    .await
}

fn validate_persona_context(
    store: &Store,
    persona_id: i64,
    context_revision: u64,
) -> AppResult<crate::persona::Context> {
    let context = crate::persona::context(&store.connection, store.context_revision)?;
    if context.persona_id != persona_id.to_string() || context.context_revision != context_revision
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设或会话已经改变，请刷新后重试",
        ));
    }
    Ok(context)
}

#[tauri::command]
pub async fn get_wallet(window: WebviewWindow, runtime: State<'_, Runtime>) -> AppResult<crate::models::Wallet> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |store, _| crate::currency::get_wallet(&store.connection)).await
}

#[tauri::command]
pub fn get_app_info(window: WebviewWindow, app: AppHandle) -> AppResult<crate::models::AppInfo> {
    authorize(&window)?;
    Ok(crate::models::AppInfo { name: app.config().product_name.clone().unwrap_or_else(|| "颜喵 YanMew".into()), version: app.package_info().version.to_string() })
}

#[tauri::command]
pub fn request_app_exit(window: WebviewWindow, app: AppHandle) -> AppResult<()> {
    authorize(&window)?;
    if window.label() != "settings" { return Err(AppError::new("PERMISSION_DENIED", "请从设置窗口退出应用")); }
    // 和托盘相同，退出事件调用已有Runtime.shutdown完成保存及请求取消。
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn polish_reminder_occurrence(window: WebviewWindow, runtime: State<'_, Runtime>, id: String, persona_id: String, context_revision: u64) -> AppResult<Option<crate::reminders::polish::Presentation>> {
    authorize(&window)?;
    if window.label() != "main" { return Err(AppError::new("PERMISSION_DENIED", "仅宠窗可润色到点提醒")); }
    crate::reminders::polish::get(runtime.inner().clone(), parse_id(&id)?, persona_id, context_revision).await
}

#[tauri::command]
pub async fn get_sign_in_status(window: WebviewWindow, runtime: State<'_, Runtime>) -> AppResult<crate::models::SignInStatus> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    storage(worker.clone(), move |store, _| {
        let now = worker.clock.sample()?.utc;
        crate::currency::get_sign_in_status(&store.connection, now, &chrono::Local)
    }).await
}

#[tauri::command]
pub async fn daily_sign_in(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>) -> AppResult<crate::models::DailySignInResult> {
    authorize(&window)?;
    if window.label() != "settings" {
        return Err(AppError::new("PERMISSION_DENIED", "请从设置的桌宠状态页面签到"));
    }
    let worker = runtime.inner().clone();
    let result = storage(worker.clone(), move |store, _| {
        let now = worker.clock.sample()?.utc;
        let mut random = worker.random.lock().map_err(|_| AppError::storage())?;
        crate::currency::daily_sign_in(&mut store.connection, now, &chrono::Local, random.as_mut())
    }).await?;
    if result.granted_coins.is_some() { let _ = app.emit(crate::events::WALLET_CHANGED, ()); }
    Ok(result)
}

pub(crate) fn query_session_intimacy(store: &Store, persona_id: i64, session_id: i64, context_revision: u64) -> AppResult<crate::models::SessionIntimacy> {
    let context = validate_persona_context(store, persona_id, context_revision)?;
    if context.session_id.as_deref() != Some(session_id.to_string().as_str()) {
        return Err(AppError::new("CONTEXT_CHANGED", "当前会话已改变，请重新读取亲密度"));
    }
    crate::currency::get_session_intimacy(&store.connection, persona_id, session_id, context_revision)
}

#[tauri::command]
pub async fn get_session_intimacy(window: WebviewWindow, runtime: State<'_, Runtime>, persona_id: String, session_id: String,
    context_revision: u64) -> AppResult<crate::models::SessionIntimacy> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    storage(runtime.inner().clone(), move |store, _| query_session_intimacy(store, persona_id, session_id, context_revision)).await
}

fn parse_optional_id(value: Option<String>, field: &str) -> AppResult<Option<i64>> {
    value
        .map(|value| {
            parse_id(&value).map_err(|mut error| {
                error.field = Some(field.into());
                error
            })
        })
        .transpose()
}

#[tauri::command]
pub async fn list_sessions(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    persona_id: String,
    context_revision: u64,
) -> AppResult<Vec<crate::models::Session>> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    storage(runtime.inner().clone(), move |store, _| {
        validate_persona_context(store, persona_id, context_revision)?;
        crate::sessions::list(&store.connection, persona_id)
    })
    .await
}

#[tauri::command]
pub async fn create_session(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    persona_id: String,
    context_revision: u64,
    user_profile_id: Option<String>,
) -> AppResult<crate::models::Session> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let user_profile_id = user_profile_id.as_deref().map(parse_id).transpose()?;
    let worker = runtime.inner().clone();
    let requests = worker.requests.clone();
    let (session, context, pet_error) = storage(worker.clone(), move |store, secrets| {
        worker.settle_pet(store, secrets)?;
        let before = validate_persona_context(store, persona_id, context_revision)?;
        let session = crate::sessions::create_with_profile(&mut store.connection, persona_id, user_profile_id)?;
        memory_management::discard(&worker);
        store.accessory_crafts.discard();
        if let Some(old_session) = before.session_id {
            requests.cancel_key(&format!("chat:{old_session}")); requests.cancel_key(&format!("proactive:{old_session}"));
        }
        store.context_revision += 1;
        store.pet.reset_context();
        let context = crate::persona::context(&store.connection, store.context_revision)?;
        let pet_error = worker.settle_pet(store, secrets).err();
        Ok((session, context, pet_error))
    })
    .await?;
    crate::accessories::images::cleanup_unused(runtime.inner(), &[]);
    crate::chat_images::cleanup_unused(runtime.inner());
    if let Some(error) = pet_error {
        let _ = app.emit(crate::events::PET_ACTIVITY_ERROR, &error);
    }
    if app.emit("window_context_changed", &context).is_err() {
        eprintln!("上下文变化通知发送失败，请刷新窗口");
    }
    Ok(session)
}

#[tauri::command]
pub async fn set_active_session(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    persona_id: String,
    session_id: String,
    context_revision: u64,
) -> AppResult<crate::persona::Context> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let worker = runtime.inner().clone();
    let requests = worker.requests.clone();
    let (context, changed, pet_error) = storage(worker.clone(), move |store, secrets| {
        worker.settle_pet(store, secrets)?;
        let before = validate_persona_context(store, persona_id, context_revision)?;
        let changed = crate::sessions::select(&mut store.connection, persona_id, session_id)?;
        if !changed {
            return Ok((before, false, None));
        }
        memory_management::discard(&worker);
        store.accessory_crafts.discard();
        if let Some(old_session) = before.session_id {
            requests.cancel_key(&format!("chat:{old_session}")); requests.cancel_key(&format!("proactive:{old_session}"));
        }
        store.context_revision += 1;
        store.pet.reset_context();
        let context = crate::persona::context(&store.connection, store.context_revision)?;
        let pet_error = worker.settle_pet(store, secrets).err();
        Ok((context, true, pet_error))
    })
    .await?;
    crate::accessories::images::cleanup_unused(runtime.inner(), &[]);
    crate::chat_images::cleanup_unused(runtime.inner());
    if let Some(error) = pet_error {
        let _ = app.emit(crate::events::PET_ACTIVITY_ERROR, &error);
    }
    if changed && app.emit("window_context_changed", &context).is_err() {
        eprintln!("上下文变化通知发送失败，请刷新窗口");
    }
    Ok(context)
}

#[tauri::command]
pub async fn rename_session(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    persona_id: String,
    session_id: String,
    context_revision: u64,
    title: String,
) -> AppResult<crate::models::Session> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let requests = runtime.requests.clone();
    let session = storage(runtime.inner().clone(), move |store, _| {
        validate_persona_context(store, persona_id, context_revision)?;
        let session = crate::sessions::rename(&mut store.connection, persona_id, session_id, &title)?;
        requests.cancel_key(&format!("session-title:{session_id}"));
        Ok(session)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(session)
}

#[tauri::command]
pub async fn set_session_pinned(
    window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>,
    persona_id: String, session_id: String, context_revision: u64, is_pinned: bool,
) -> AppResult<crate::models::Session> {
    authorize(&window)?;
    let (persona_id, session_id) = (parse_id(&persona_id)?, parse_id(&session_id)?);
    let session = storage(runtime.inner().clone(), move |store, _| {
        validate_persona_context(store, persona_id, context_revision)?;
        crate::sessions::set_pinned(&mut store.connection, persona_id, session_id, is_pinned)
    }).await?;
    let _ = app.emit("configuration_changed", ());
    Ok(session)
}

#[tauri::command]
pub async fn delete_session(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    persona_id: String,
    session_id: String,
    context_revision: u64,
) -> AppResult<crate::models::SessionDeleteResult> {
    authorize(&window)?;
    let (persona_id,session_id) = (parse_id(&persona_id)?,parse_id(&session_id)?);
    let worker = runtime.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move ||
        crate::sessions::delete_in_context(&worker,persona_id,session_id,context_revision)
    ).await.map_err(|_|AppError::storage())??;
    crate::accessories::images::cleanup_unused(runtime.inner(), &[]);
    crate::chat_images::cleanup_unused(runtime.inner());
    if let Some(error) = outcome.pet_error { let _ = app.emit(crate::events::PET_ACTIVITY_ERROR,&error); }
    // 仅删除当前项改变上下文，其余删除刷新列表，不打断当前窗口任务。
    if outcome.context_changed && app.emit("window_context_changed",&outcome.context).is_err() {
        eprintln!("上下文变化通知发送失败，请刷新窗口");
    }
    let _ = app.emit("configuration_changed",());
    let _ = app.emit(crate::events::MEMORY_GENERATION_CHANGED,());
    let _ = app.emit(crate::events::EMBEDDING_CHANGED,());
    Ok(outcome.result)
}

#[tauri::command]
pub async fn list_user_profiles(window: WebviewWindow, runtime: State<'_, Runtime>) -> AppResult<Vec<crate::models::UserProfile>> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |store, _| crate::user_profiles::list(&store.connection)).await
}

#[tauri::command]
pub async fn create_user_profile(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, name: String, content: String) -> AppResult<crate::models::UserProfile> {
    authorize(&window)?;
    let saved = storage(runtime.inner().clone(), move |store, _| {
        let id = crate::user_profiles::create(&store.connection, &name, &content)?;
        crate::user_profiles::get(&store.connection, id)
    }).await?;
    let _ = app.emit("configuration_changed", ());
    Ok(saved)
}

fn notify_user_profile_change<T>(app: &AppHandle, outcome: crate::user_profiles::Change<T>) -> AppResult<T> {
    for session in outcome.sessions {
        let _ = app.emit("chat-messages-changed", serde_json::json!({"sessionId":session.to_string()}));
    }
    let _ = app.emit("configuration_changed", ());
    outcome.result
}

#[tauri::command]
pub async fn update_user_profile(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, id: String, name: String, content: String) -> AppResult<crate::models::UserProfile> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let worker = runtime.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || crate::user_profiles::update_in_runtime(&worker, id, &name, &content))
        .await.map_err(|_| AppError::storage())??;
    runtime.memory.wake.notify_one();
    notify_user_profile_change(&app, outcome)
}

#[tauri::command]
pub async fn delete_user_profile(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, id: String) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let worker = runtime.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || crate::user_profiles::delete_in_runtime(&worker, id))
        .await.map_err(|_| AppError::storage())??;
    runtime.memory.wake.notify_one();
    notify_user_profile_change(&app, outcome)
}

#[tauri::command]
pub async fn set_session_user_profile(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, persona_id: String, session_id: String, context_revision: u64, user_profile_id: Option<String>) -> AppResult<crate::models::Session> {
    authorize(&window)?;
    let persona = parse_id(&persona_id)?;
    let session = parse_id(&session_id)?;
    let profile = parse_optional_id(user_profile_id, "userProfileId")?;
    let worker = runtime.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || crate::user_profiles::bind_in_runtime(&worker, persona, session, context_revision, profile))
        .await.map_err(|_| AppError::storage())??;
    runtime.memory.wake.notify_one();
    notify_user_profile_change(&app, outcome)
}

#[tauri::command]
pub async fn list_messages(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    persona_id: String,
    session_id: String,
    context_revision: u64,
    after_id: Option<String>,
    limit: Option<i64>,
) -> AppResult<crate::models::MessagePage> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let after_id = parse_optional_id(after_id, "afterId")?;
    storage(runtime.inner().clone(), move |store, _| {
        let context = validate_persona_context(store, persona_id, context_revision)?;
        if context.session_id.as_deref() != Some(&session_id.to_string()) {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前会话已经改变，请刷新后重试",
            ));
        }
        crate::sessions::messages(
            &store.connection,
            persona_id,
            session_id,
            after_id,
            limit.unwrap_or(100),
        )
    })
    .await
}

#[tauri::command]
pub async fn get_weather_settings(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<crate::config::WeatherSettings> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::config::get_weather(&s.connection)
    })
    .await
}
#[tauri::command]
pub async fn get_weather(window: WebviewWindow, runtime: State<'_, Runtime>) -> AppResult<crate::weather::WeatherView> {
    authorize(&window)?;
    let clock = runtime.clock.clone();
    storage(runtime.inner().clone(), move |s, _| Ok(s.weather.view(s.context_revision, clock.sample()?.utc))).await
}
#[tauri::command]
pub async fn refresh_weather(window: WebviewWindow, runtime: State<'_, Runtime>, context_revision: u64) -> AppResult<crate::weather::WeatherView> {
    authorize(&window)?;
    let source = crate::platform::windows::location::WindowsLocation(window);
    crate::weather::refresh(runtime.inner().clone(), context_revision, &source).await
}
#[tauri::command]
pub fn cancel_weather_refresh(window: WebviewWindow, runtime: State<'_, Runtime>) -> AppResult<()> {
    authorize(&window)?;
    runtime.requests.cancel_key("weather");
    Ok(())
}
#[tauri::command]
pub async fn save_weather_settings(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    input: crate::config::WeatherInput,
) -> AppResult<crate::config::WeatherSettings> {
    authorize(&window)?;
    let requests = runtime.requests.clone();
    storage(runtime.inner().clone(), move |s, p| {
        let settings = crate::config::save_weather(&mut s.connection, input, p)?;
        requests.cancel_key("weather");
        s.weather = Default::default();
        Ok(settings)
    })
    .await
}

#[tauri::command]
pub async fn test_provider_connection(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    check_provider(runtime.inner().clone(), id).await
}

fn authorize(window: &WebviewWindow) -> AppResult<()> {
    if !crate::desktop_windows::is_application_window(window.label()) {
        return Err(AppError::new("PERMISSION_DENIED", "此窗口无权管理应用配置"));
    }
    Ok(())
}
#[tauri::command]
pub async fn focus_or_create_window(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, page: crate::desktop_windows::Page,
    detail: Option<crate::desktop_windows::SettingsTarget>) -> AppResult<()> {
    authorize(&window)?;
    if let Some(target) = detail {
        if !matches!(page, crate::desktop_windows::Page::Settings) { return Err(AppError::validation("detail", "详情跳转只能打开设置页")); }
        let (table, id) = match &target {
            crate::desktop_windows::SettingsTarget::Persona { id } => ("personas", parse_id(id)?),
            crate::desktop_windows::SettingsTarget::UserProfile { id } => ("user_profiles", parse_id(id)?),
            _ => return Err(AppError::validation("detail", "请选择人设或用户设定详情")),
        };
        runtime.read(|store,_| {
            let exists: bool = store.connection.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"), [id], |row| row.get(0))?;
            if exists { Ok(()) } else { Err(AppError::not_found()) }
        })?;
        return crate::desktop_windows::open_settings_detail(app, target).await;
    }
    crate::desktop_windows::open(app, page).await
}
#[tauri::command]
pub async fn get_memory_generation_status(
    window: WebviewWindow, runtime: State<'_, Runtime>,
    persona_id: String, session_id: String, context_revision: u64,
) -> AppResult<crate::memory::GenerationView> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || crate::memory::current_status(&worker, persona_id, session_id, context_revision))
        .await.map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn retry_short_memories(
    window: WebviewWindow, runtime: State<'_, Runtime>, app: AppHandle,
    persona_id: String, session_id: String, context_revision: u64,
) -> AppResult<()> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || crate::memory::retry_current(&worker, persona_id, session_id, context_revision))
        .await.map_err(|_| AppError::storage())??;
    let _ = app.emit(crate::events::MEMORY_GENERATION_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub async fn get_reminder_occurrence(window: WebviewWindow, runtime: State<'_, Runtime>, id: String) -> AppResult<crate::models::ReminderOccurrence> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    storage(runtime.inner().clone(), move |store, _| crate::reminders::get_occurrence(&store.connection, id)).await
}
#[tauri::command]
pub fn take_reminder_navigation(window: WebviewWindow, navigation: State<'_, crate::desktop_windows::SettingsNavigation>) -> AppResult<bool> {
    if window.label() != "settings" {
        return Err(AppError::new("PERMISSION_DENIED", "仅设置窗口可读取设置导航"));
    }
    let mut pending = navigation.inner().0.lock().map_err(|_| AppError::storage())?;
    if matches!(*pending, Some(crate::desktop_windows::SettingsTarget::Reminders)) {
        *pending = None; Ok(true)
    } else { Ok(false) }
}
/// 固定设置目标可在页面加载后消费，避免快捷入口的一次性事件丢失。
#[tauri::command]
pub fn take_settings_navigation(window: WebviewWindow, navigation: State<'_, crate::desktop_windows::SettingsNavigation>) -> AppResult<Option<crate::desktop_windows::SettingsTarget>> {
    if window.label() != "settings" { return Err(AppError::new("PERMISSION_DENIED", "仅设置窗口可读取设置导航")); }
    let mut pending = navigation.inner().0.lock().map_err(|_| AppError::storage())?;
    Ok(pending.take())
}
async fn storage<T: Send + 'static>(
    runtime: Runtime,
    action: impl FnOnce(&mut Store, &dyn crate::platform::contracts::SecretProtector) -> AppResult<T>
        + Send
        + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(move || runtime.read(action))
        .await
        .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn get_configuration_status(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<()> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |_, _| Ok(())).await
}
#[tauri::command]
pub async fn get_settings(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<crate::config::Settings> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::config::get(&s.connection)
    })
    .await
}
#[tauri::command]
pub fn get_settings_defaults(window: WebviewWindow) -> AppResult<crate::config::Settings> {
    authorize(&window)?;
    Ok(crate::config::defaults())
}

#[tauri::command]
pub async fn save_settings(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    changes: crate::config::Settings,
) -> AppResult<crate::config::Settings> {
    authorize(&window)?;
    let worker = runtime.inner().clone();
    let settings = storage(runtime.inner().clone(), move |s, p| {
        worker.settle_pet(s, p)?;
        crate::config::save(&mut s.connection, changes)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(settings)
}

#[tauri::command]
pub async fn list_providers(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Vec<crate::providers::Provider>> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::providers::list(&s.connection)
    })
    .await
}
#[tauri::command]
pub async fn get_provider_roles(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<crate::provider_roles::Config> {
    authorize(&window)?;storage(runtime.inner().clone(),|s,_|crate::provider_roles::get(&s.connection)).await
}
#[tauri::command]
pub async fn save_provider_role(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,purpose:String,provider_id:Option<String>,model:String,enabled:bool,expected_revision:u64,max_output_tokens:Option<u32>,context_window_override:Option<u32>)->AppResult<crate::provider_roles::Config> {
    authorize_image_settings(&window)?;
    let requests=runtime.requests.clone();
    let (config,preview)=storage(runtime.inner().clone(),move|s,_| {
        if max_output_tokens==Some(0)||context_window_override==Some(0){return Err(AppError::validation("maxOutputTokens","高级容量必须为正整数"));}
        let result=crate::provider_roles::save(&mut s.connection,&purpose,provider_id.as_deref(),&model,enabled,expected_revision,max_output_tokens,context_window_override)?;
        if purpose=="chat" {
            requests.cancel_all();
        } else if purpose=="vision" {
            let context=crate::persona::context(&s.connection,s.context_revision)?;
            if let Some(session)=context.session_id{requests.cancel_key(&format!("chat:{session}"));}
        }
        Ok((result,if purpose=="chat"||purpose=="image"{crate::accessories::craft::configuration_changed(s)}else{None}))
    }).await?;
    notify_image_config(&app,preview);let _=app.emit("configuration_changed",());Ok(config)
}
#[tauri::command]
pub async fn create_provider(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    input: crate::providers::ProviderInput,
) -> AppResult<crate::providers::Provider> {
    authorize(&window)?;
    let result = storage(runtime.inner().clone(), move |s, p| {
        crate::providers::save(&mut s.connection, None, input, p)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(result)
}
#[tauri::command]
pub async fn update_provider(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
    input: crate::providers::ProviderInput,
) -> AppResult<crate::providers::Provider> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let requests = runtime.requests.clone();
    let (result,preview) = storage(runtime.inner().clone(), move |s, p| {
        let result = crate::providers::save(&mut s.connection, Some(id), input, p)?;
        if result.is_active || result.purposes.iter().any(|p|p=="vision") {
            requests.cancel_all();
        }
        let image=result.purposes.iter().any(|p|p=="image");
        let preview=if image{crate::accessories::craft::configuration_changed(s)}else{None};
        Ok((result,preview))
    })
    .await?;
    if result.purposes.iter().any(|p|p=="image"){notify_image_config(&app,preview);}
    let _ = app.emit("configuration_changed", ());
    Ok(result)
}

#[tauri::command]
pub async fn list_personas(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Vec<crate::persona::Persona>> {
    authorize(&window)?;
    storage(runtime.inner().clone(), |s, _| {
        crate::persona::list(&s.connection)
    })
    .await
}
#[tauri::command]
pub async fn create_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    input: crate::persona::PersonaInput,
) -> AppResult<crate::persona::Persona> {
    authorize(&window)?;
    let persona = storage(runtime.inner().clone(), move |s, _| {
        crate::persona::save(&mut s.connection, None, input)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    let id = parse_id(&persona.id)?;
    let available = storage(runtime.inner().clone(), |s, _| {
        Ok(s.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_effective WHERE is_active=1)",
            [],
            |r| r.get::<_, bool>(0),
        )?)
    })
    .await;
    let available = match available {
        Ok(value) => value,
        Err(error) => {
            let _ = app.emit(
                "expression_generation_failed",
                serde_json::json!({"personaId":id.to_string(),"error":error}),
            );
            false
        }
    };
    if available {
        let runtime = runtime.inner().clone();
        tauri::async_runtime::spawn(async move {
            match crate::expressions::generate_persona_expressions(runtime, id).await {
                Ok(_) => {
                    let _ = app.emit("configuration_changed", ());
                }
                Err(error) => {
                    let _ = app.emit(
                        "expression_generation_failed",
                        serde_json::json!({"personaId":id.to_string(),"error":error}),
                    );
                }
            }
        });
    }
    Ok(persona)
}

fn background_access(window: &WebviewWindow, revision: Option<u64>) -> AppResult<Option<u64>> {
    match window.label() {
        "settings" => Ok(None),
        "chat" => revision.map(Some).ok_or_else(|| AppError::new("CONTEXT_CHANGED", "请重新打开当前人设的背景设置")),
        _ => Err(AppError::new("PERMISSION_DENIED", "仅聊天和设置窗口可访问聊天背景")),
    }
}
#[tauri::command]
pub async fn get_chat_background(window: WebviewWindow, runtime: State<'_, Runtime>, id: String,
    context_revision: Option<u64>) -> AppResult<crate::persona::background::View> {
    let revision = background_access(&window, context_revision)?;
    let id = parse_id(&id)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || crate::persona::background::get(&worker, id, revision))
        .await.map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn save_chat_background(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, id: String,
    context_revision: Option<u64>, input: crate::persona::background::Input) -> AppResult<crate::persona::background::Background> {
    let revision = background_access(&window, context_revision)?;
    let id = parse_id(&id)?;
    let worker = runtime.inner().clone();
    let saved = tauri::async_runtime::spawn_blocking(move || crate::persona::background::save(&worker, id, revision, input))
        .await.map_err(|_| AppError::storage())??;
    let _ = app.emit("configuration_changed", ());
    Ok(saved)
}
#[tauri::command]
pub async fn update_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
    input: crate::persona::PersonaInput,
) -> AppResult<crate::persona::Persona> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let requests = runtime.requests.clone();
    let persona = storage(runtime.inner().clone(), move |s, _| {
        let result = crate::persona::save(&mut s.connection, Some(id), input)?;
        requests.cancel_persona(id);
        Ok(result)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(persona)
}

#[tauri::command]
pub async fn delete_provider(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let result = storage(runtime.inner().clone(), move |s, _| {
        crate::providers::delete(&mut s.connection, id)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(result)
}
#[tauri::command]
pub async fn set_active_chat_provider(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<()> {
    authorize(&window)?;
    let id = parse_id(&id)?;
    let requests = runtime.requests.clone();
    let result = storage(runtime.inner().clone(), move |s, _| {
        let same = s.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_effective WHERE id=?1 AND is_active=1)",
            [id],
            |r| r.get::<_, bool>(0),
        )?;
        crate::providers::select(&mut s.connection, Some(id), false)?;
        if !same {
            requests.cancel_all();
        }
        Ok(())
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(result)
}
#[tauri::command]
pub async fn set_memory_provider(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: Option<String>,
) -> AppResult<()> {
    authorize(&window)?;
    let id = id.as_deref().map(parse_id).transpose()?;
    let result = storage(runtime.inner().clone(), move |s, _| {
        crate::providers::select(&mut s.connection, id, true)
    })
    .await?;
    let _ = app.emit("configuration_changed", ());
    Ok(result)
}
async fn change_persona(
    app: AppHandle,
    runtime: Runtime,
    id: i64,
    delete: bool,
) -> AppResult<crate::persona::Context> {
    let requests = runtime.requests.clone();
    let worker = runtime.clone();
    let cleanup_runtime = runtime.clone();
    let (context, changed) = storage(runtime, move |s, p| {
        worker.settle_pet(s, p)?;
        let before = crate::persona::context(&s.connection, s.context_revision)?;
        if delete {
            crate::persona::delete(&mut s.connection, id)?;
            requests.cancel_persona(id);
        } else {
            crate::persona::select(&mut s.connection, id)?;
        }
        let mut after = crate::persona::context(&s.connection, s.context_revision)?;
        let changed =
            before.persona_id != after.persona_id || before.session_id != after.session_id;
        if changed {
            requests.cancel_persona(parse_id(&before.persona_id)?);
            memory_management::discard(&worker);
            s.accessory_crafts.discard();
            s.context_revision += 1;
            after.context_revision = s.context_revision;
            s.pet.reset_context();
            worker.settle_pet(s, p)?;
        }
        Ok((after, changed))
    })
    .await?;
    if delete || changed { crate::accessories::images::cleanup_unused(&cleanup_runtime, &[]);crate::chat_images::cleanup_unused(&cleanup_runtime); }
    if delete {
        let _ = app.emit("configuration_changed", ());
    }
    // 事务已提交；窗口可通过列表重新获取权威当前项。
    if changed && app.emit("window_context_changed", &context).is_err() {
        eprintln!("上下文变化通知发送失败，请刷新窗口");
    }
    Ok(context)
}
#[tauri::command]
pub async fn set_active_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<crate::persona::Context> {
    authorize(&window)?;
    change_persona(app, runtime.inner().clone(), parse_id(&id)?, false).await
}
#[tauri::command]
pub async fn delete_persona(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
) -> AppResult<crate::persona::Context> {
    authorize(&window)?;
    change_persona(app, runtime.inner().clone(), parse_id(&id)?, true).await
}

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, LogicalSize, Manager};

pub const STATE_EVENT: &str = "pet-window-state";
const PET_CHAT_EXTRA_HEIGHT: f64 = 120.0;

#[derive(Clone, Copy, Default, Serialize)]
pub struct Snapshot {
    pub hidden: bool,
    #[serde(rename = "chatOpen")]
    pub chat_open: bool,
    pub revision: u64,
}

#[derive(Default)]
pub struct PetState {
    snapshot: Snapshot,
    full_size: Option<LogicalSize<f64>>,
    region: Option<Vec<[i32; 4]>>,
}

impl PetState {
    fn target_size(&self, hidden: bool) -> LogicalSize<f64> {
        if hidden {
            LogicalSize::new(112.0, 44.0)
        } else {
            let base = self.full_size.unwrap_or(LogicalSize::new(320.0, 280.0));
            LogicalSize::new(base.width, base.height + if self.snapshot.chat_open { PET_CHAT_EXTRA_HEIGHT } else { 0.0 })
        }
    }

    fn commit_hidden(&mut self, hidden: bool, full_size: Option<LogicalSize<f64>>) {
        if self.snapshot.hidden != hidden {
            self.full_size = full_size;
            self.snapshot.hidden = hidden;
            self.snapshot.revision += 1;
            self.region = None;
        }
    }

    fn accepts_region(&self, revision: u64) -> bool {
        self.snapshot.revision == revision
    }
}

async fn on_main<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(AppHandle) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(operation(handle));
    })
    .map_err(|e| format!("调度宠窗操作失败：{e}"))?;
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv().map_err(|_| "宠窗操作已中断".to_string())
    })
    .await
    .map_err(|e| format!("等待宠窗操作失败：{e}"))??
}

// 调用方必须位于窗口主线程；托盘和 command 共用这一条恢复路径。
pub fn change_hidden(app: &AppHandle, hidden: bool) -> Result<Snapshot, String> {
    let window = app.get_webview_window("main").ok_or("找不到 main 宠窗")?;
    let managed = app.state::<Mutex<PetState>>();
    let mut state = managed.lock().map_err(|_| "宠窗状态不可用")?;
    if state.snapshot.hidden != hidden {
        let full_size = if hidden {
            let mut base: LogicalSize<f64> = window.inner_size()
                .map_err(|e| format!("读取宠窗尺寸失败：{e}"))?
                .to_logical(window.scale_factor().map_err(|e| format!("读取屏幕缩放失败：{e}"))?);
            if state.snapshot.chat_open { base.height = (base.height - PET_CHAT_EXTRA_HEIGHT).max(1.0); }
            Some(base)
        } else { state.full_size };
        // 先解除旧模式裁剪，确保恢复期间不会被旧区域永久遮挡。
        pet_region::apply(&window, None)?;
        state.region = None;
        window
            .set_size(state.target_size(hidden))
            .map_err(|e| format!("调整宠窗尺寸失败：{e}"))?;
        state.commit_hidden(hidden, full_size);
    } else if !hidden {
        pet_region::apply(&window, None)?;
        state.region = None;
        state.snapshot.revision += 1;
    }
    let snapshot = state.snapshot;
    drop(state);
    // 数据库故障不得让用户失去隐藏/恢复窗口的能力。
    if let Err(error) = app.state::<Runtime>().pet_visibility(Some(hidden), None) {
        let _ = app.emit(crate::events::PET_ACTIVITY_ERROR, &error);
    }
    window
        .emit(STATE_EVENT, snapshot)
        .map_err(|e| format!("同步宠窗状态失败：{e}"))?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn get_pet_window_state(app: AppHandle) -> Result<Snapshot, String> {
    on_main(app, |app| {
        Ok(app
            .state::<Mutex<PetState>>()
            .lock()
            .map_err(|_| "宠窗状态不可用")?
            .snapshot)
    })
    .await
}

#[tauri::command]
pub async fn set_pet_hidden(app: AppHandle, hidden: bool) -> Result<Snapshot, String> {
    on_main(app, move |app| change_hidden(&app, hidden)).await
}

/// 快捷输入只扩大透明可用区域，复用主线程尺寸与裁剪处理；不写入数据库。
#[tauri::command]
pub async fn set_pet_chat_open(window: WebviewWindow, app: AppHandle, open: bool) -> Result<Snapshot, String> {
    if window.label() != "main" { return Err("只有宠窗可以展开快捷输入".into()); }
    on_main(app, move |app| {
        let window=app.get_webview_window("main").ok_or("找不到 main 宠窗")?;
        let managed=app.state::<Mutex<PetState>>();
        let mut state=managed.lock().map_err(|_|"宠窗状态不可用")?;
        if state.snapshot.hidden { return Err("请先显示宠窗".into()); }
        if state.snapshot.chat_open != open {
            let mut base:LogicalSize<f64>=window.inner_size().map_err(|_|"读取宠窗尺寸失败")?
                .to_logical(window.scale_factor().map_err(|_|"读取屏幕缩放失败")?);
            if state.snapshot.chat_open {base.height=(base.height-PET_CHAT_EXTRA_HEIGHT).max(1.0);}
            let target=LogicalSize::new(base.width,base.height+if open {PET_CHAT_EXTRA_HEIGHT}else{0.0});
            pet_region::apply(&window,None)?;
            state.region=None;
            window.set_size(target).map_err(|_|"调整快捷输入区域失败")?;
            state.full_size=Some(base);
            state.snapshot.chat_open=open;
            state.snapshot.revision+=1;
        }
        let snapshot=state.snapshot;drop(state);
        // 沿用现有多屏可见性处理，靠近屏幕下缘时保证输入框可操作。
        let position=app.state::<crate::pet_position::WindowPosition>();
        if let Err(error)=position.ensure_visible(&window,true) {crate::pet_position::report(&app,&error);}
        window.emit(STATE_EVENT,snapshot).map_err(|_|"同步快捷输入状态失败")?;
        Ok(snapshot)
    }).await
}

#[tauri::command]
pub async fn update_pet_regions(
    app: AppHandle,
    revision: u64,
    scale: f64,
    rects: Vec<Rect>,
) -> Result<(), String> {
    on_main(app, move |app| {
        let managed = app.state::<Mutex<PetState>>();
        let mut state = managed.lock().map_err(|_| "宠窗状态不可用")?;
        if !state.accepts_region(revision) {
            return Ok(());
        }
        let window = app.get_webview_window("main").ok_or("找不到 main 宠窗")?;
        let region = pet_region::window_rects(&window, &rects, scale)?;
        if state.region.as_ref() == Some(&region) {
            return Ok(());
        }
        pet_region::apply(&window, Some(&region))?;
        state.region = Some(region);
        Ok(())
    })
    .await
}

#[cfg(test)]
#[path = "commands/recovery_tests.rs"]
mod recovery_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_hide_preserves_full_size_and_restore_revision() {
        let mut state = PetState::default();
        let full = LogicalSize::new(400.0, 300.0);
        state.commit_hidden(true, Some(full));
        state.commit_hidden(true, Some(LogicalSize::new(112.0, 44.0)));
        assert_eq!(state.target_size(false), full);
        assert_eq!(state.snapshot.revision, 1);
        state.commit_hidden(false, state.full_size);
        assert!(!state.snapshot.hidden);
        assert_eq!(state.snapshot.revision, 2);
        state.commit_hidden(false, state.full_size);
        assert_eq!(state.snapshot.revision, 2);
    }
    #[test]
    fn quick_chat_height_is_temporary_and_hidden_mode_keeps_its_small_size() {
        let mut state=PetState::default();
        state.snapshot.chat_open=true;
        assert_eq!(state.target_size(false),LogicalSize::new(320.0,400.0));
        assert_eq!(state.target_size(true),LogicalSize::new(112.0,44.0));
        state.commit_hidden(true,state.full_size);
        state.commit_hidden(false,state.full_size);
        assert_eq!(state.target_size(false),LogicalSize::new(320.0,400.0));
        state.snapshot.chat_open=false;
        assert_eq!(state.target_size(false),LogicalSize::new(320.0,280.0));
    }
    #[test]
    fn old_regions_are_rejected_after_mode_changes() {
        let mut state = PetState {
            region: Some(vec![[1, 2, 3, 4]]),
            ..Default::default()
        };
        state.commit_hidden(true, Some(LogicalSize::new(320.0, 240.0)));
        assert!(!state.accepts_region(0));
        assert!(state.accepts_region(1));
        assert!(state.region.is_none());
        state.commit_hidden(false, state.full_size);
        assert!(!state.accepts_region(1));
        assert!(state.accepts_region(2));
    }
}

fn letter_scope(persona:&str,session:&str,revision:u64)->AppResult<crate::letters::Scope> {
    Ok(crate::letters::Scope{persona:parse_id(persona)?,session:parse_id(session)?,revision})
}

#[tauri::command]
pub async fn list_travels(window: WebviewWindow, runtime: State<'_, Runtime>, persona_id: String, session_id: String, context_revision: u64, before_id: Option<String>, limit: i64) -> AppResult<Vec<crate::travel::Travel>> {
    authorize(&window)?;
    let scope = letter_scope(&persona_id, &session_id, context_revision)?;
    let before = before_id.as_deref().map(parse_id).transpose()?;
    let worker = runtime.inner().clone();
    storage(worker.clone(), move |s, secrets| {
        worker.settle_pet(s, secrets)?;
        crate::letters::require_current(&s.connection, s.context_revision, scope)?;
        crate::travel::queries::list(&s.connection, scope.session, before, limit)
    }).await
}
#[tauri::command]
pub async fn get_current_travel(window: WebviewWindow, runtime: State<'_, Runtime>, persona_id: String, session_id: String, context_revision: u64) -> AppResult<Option<crate::travel::Travel>> {
    authorize(&window)?;
    let scope = letter_scope(&persona_id, &session_id, context_revision)?;
    let worker = runtime.inner().clone();
    storage(worker.clone(), move |s, secrets| {
        worker.settle_pet(s, secrets)?;
        crate::letters::require_current(&s.connection, s.context_revision, scope)?;
        crate::travel::queries::current(&s.connection, scope.session)
    }).await
}
#[tauri::command]
pub async fn get_travel_generation_status(window: WebviewWindow, runtime: State<'_, Runtime>, persona_id: String, session_id: String, context_revision: u64) -> AppResult<crate::travel::FactsGenerationStatus> {
    authorize(&window)?;
    let scope = letter_scope(&persona_id, &session_id, context_revision)?;
    let worker = runtime.inner().clone();
    storage(worker.clone(), move |s, _| {
        crate::letters::require_current(&s.connection, s.context_revision, scope)?;
        worker.travel_facts.status(&s.connection, scope.session)
    }).await
}
#[tauri::command]
pub async fn retry_travel_generation(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, persona_id: String, session_id: String, context_revision: u64) -> AppResult<()> {
    authorize(&window)?;
    let scope = letter_scope(&persona_id, &session_id, context_revision)?;
    let worker = runtime.inner().clone();
    storage(worker.clone(), move |s, _| {
        crate::letters::require_current(&s.connection, s.context_revision, scope)?;
        worker.travel_facts.retry(&s.connection, scope.session)
    }).await?;
    let _ = app.emit(crate::events::TRAVEL_CHANGED, serde_json::json!({"sessionId":session_id}));
    Ok(())
}
#[tauri::command]
pub async fn open_letter_window(window: WebviewWindow, app: AppHandle, runtime: State<'_, Runtime>, persona_id: String, session_id: String, context_revision: u64, travel_id: String, reply_to_id: Option<String>) -> AppResult<()> {
    authorize(&window)?;
    let scope = letter_scope(&persona_id, &session_id, context_revision)?;
    let travel = parse_id(&travel_id)?;
    let reply = reply_to_id.as_deref().map(parse_id).transpose()?;
    let worker = runtime.inner().clone();
    storage(worker.clone(), move |s, secrets| {
        worker.settle_pet(s, secrets)?;
        crate::letters::require_current(&s.connection, s.context_revision, scope)?;
        crate::letters::validate_target(&s.connection, scope.session, travel, reply, worker.clock.sample()?.utc)
    }).await?;
    let target = crate::desktop_windows::LetterTarget { persona_id:scope.persona.to_string(),session_id:scope.session.to_string(),context_revision,travel_id:travel.to_string(),reply_to_id:reply.map(|id|id.to_string()) };
    *app.state::<crate::desktop_windows::LetterNavigation>().inner().0.lock().map_err(|_|AppError::storage())? = Some(target);
    crate::desktop_windows::open(app.clone(), crate::desktop_windows::Page::Letter).await?;
    if let Some(editor) = app.get_webview_window("letter") { let _ = editor.emit("letter-navigation", ()); }
    Ok(())
}
#[tauri::command]
pub fn take_letter_navigation(window: WebviewWindow, pending: State<'_, crate::desktop_windows::LetterNavigation>) -> AppResult<Option<crate::desktop_windows::LetterTarget>> {
    if window.label() != "letter" { return Err(AppError::new("PERMISSION_DENIED", "仅写信窗口可读取写信目标")); }
    Ok(pending.inner().0.lock().map_err(|_|AppError::storage())?.take())
}
#[tauri::command]
pub fn take_travel_navigation(window: WebviewWindow, pending: State<'_, crate::desktop_windows::TravelNavigation>) -> AppResult<Option<bool>> {
    if window.label() != "travel" { return Err(AppError::new("PERMISSION_DENIED", "仅出行窗口可读取出行导航")); }
    Ok(pending.inner().0.lock().map_err(|_|AppError::storage())?.take())
}
#[tauri::command]
pub async fn list_letters(window:WebviewWindow,runtime:State<'_,Runtime>,persona_id:String,session_id:String,context_revision:u64,before_id:Option<String>,limit:i64)->AppResult<Vec<crate::models::Letter>> {
    authorize(&window)?;let scope=letter_scope(&persona_id,&session_id,context_revision)?;let before=before_id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(),move|s,_|{crate::letters::require_current(&s.connection,s.context_revision,scope)?;crate::letters::list(&s.connection,scope.session,before,limit)}).await
}
#[tauri::command]
pub async fn get_letter(window:WebviewWindow,runtime:State<'_,Runtime>,persona_id:String,session_id:String,context_revision:u64,id:String)->AppResult<crate::models::Letter> {
    authorize(&window)?;let scope=letter_scope(&persona_id,&session_id,context_revision)?;let id=parse_id(&id)?;
    storage(runtime.inner().clone(),move|s,_|{crate::letters::require_current(&s.connection,s.context_revision,scope)?;crate::letters::get(&s.connection,scope.session,id)}).await
}
#[tauri::command]
pub async fn send_letter(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,session_id:String,context_revision:u64,input:crate::letters::SendInput)->AppResult<crate::models::Letter> {
    authorize(&window)?;let scope=letter_scope(&persona_id,&session_id,context_revision)?;let travel=parse_id(&input.travel_id)?;let reply=input.reply_to_id.as_deref().map(parse_id).transpose()?;
    let worker=runtime.inner().clone();
    let result=storage(worker.clone(),move|s,secrets|{
        worker.settle_pet(s,secrets)?;crate::letters::require_current(&s.connection,s.context_revision,scope)?;
        let mut random=worker.random.lock().map_err(|_|AppError::storage())?;
        crate::letters::send(&mut s.connection,s.context_revision,scope,travel,&input.content,reply,worker.clock.sample()?.utc,random.as_mut())
    }).await?;
    runtime.letters.wake.notify_one();let _=app.emit(crate::events::LETTERS_CHANGED,serde_json::json!({"sessionId":session_id}));Ok(result)
}
#[tauri::command]
pub async fn set_letter_flags(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,session_id:String,context_revision:u64,id:String,favorite:Option<bool>,read:Option<bool>)->AppResult<crate::models::Letter> {
    authorize(&window)?;if favorite.is_none()&&read.is_none(){return Err(AppError::validation("flags","请选择要修改的信件标记"))}
    let scope=letter_scope(&persona_id,&session_id,context_revision)?;let id=parse_id(&id)?;
    let letter=storage(runtime.inner().clone(),move|s,_|{crate::letters::require_current(&s.connection,s.context_revision,scope)?;crate::letters::flags(&s.connection,scope.session,id,favorite,read)}).await?;
    let _=app.emit(crate::events::LETTERS_CHANGED,serde_json::json!({"sessionId":session_id}));Ok(letter)
}
#[tauri::command]
pub async fn delete_letter(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,session_id:String,context_revision:u64,id:String)->AppResult<()> {
    authorize(&window)?;let scope=letter_scope(&persona_id,&session_id,context_revision)?;let id=parse_id(&id)?;
    storage(runtime.inner().clone(),move|s,_|{crate::letters::require_current(&s.connection,s.context_revision,scope)?;crate::letters::delete(&s.connection,scope.session,id)}).await?;
    runtime.requests.cancel_key(&format!("letter-reply:{id}"));runtime.letters.wake.notify_one();
    let _=app.emit(crate::events::LETTERS_CHANGED,serde_json::json!({"sessionId":session_id}));Ok(())
}
#[tauri::command]
pub async fn get_letter_generation_status(window:WebviewWindow,runtime:State<'_,Runtime>,persona_id:String,session_id:String,context_revision:u64)->AppResult<crate::letters::GenerationStatus> {
    authorize(&window)?;let scope=letter_scope(&persona_id,&session_id,context_revision)?;let worker=runtime.inner().clone();
    storage(worker.clone(),move|s,_|{crate::letters::require_current(&s.connection,s.context_revision,scope)?;worker.letters.status(scope.session)}).await
}
#[tauri::command]
pub async fn retry_letter_generation(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,session_id:String,context_revision:u64)->AppResult<()> {
    authorize(&window)?;let scope=letter_scope(&persona_id,&session_id,context_revision)?;let worker=runtime.inner().clone();
    storage(worker.clone(),move|s,_|{crate::letters::require_current(&s.connection,s.context_revision,scope)?;worker.letters.retry(scope.session)}).await?;
    let _=app.emit(crate::events::LETTERS_CHANGED,serde_json::json!({"sessionId":session_id}));Ok(())
}


// 饰品接口复用聊天的完整上下文输入；窗口白名单与插件命令门控保持不变。
fn accessory_target(context: &ChatContextInput) -> AppResult<(i64,i64,u64)> {
    Ok((parse_id(&context.persona_id)?,parse_id(&context.session_id)?,context.context_revision))
}
fn notify_accessories(app: &AppHandle, session: i64) {
    let _ = app.emit(crate::events::ACCESSORIES_CHANGED,serde_json::json!({"sessionId":session.to_string()}));
}
#[tauri::command]
pub async fn list_accessories(window:WebviewWindow,runtime:State<'_,Runtime>,context:ChatContextInput,before_id:Option<String>,limit:Option<i64>) -> AppResult<Vec<crate::models::Accessory>> {
    authorize(&window)?;
    let (persona,session,revision) = accessory_target(&context)?;
    let before = before_id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(),move|store,_| {
        crate::accessories::require_current(store,persona,session,revision)?;
        crate::accessories::list(&store.connection,session,before,limit.unwrap_or(30))
    }).await
}
#[tauri::command]
pub async fn get_accessory_image(window:WebviewWindow,runtime:State<'_,Runtime>,context:ChatContextInput,id:String) -> AppResult<crate::accessories::images::ImageView> {
    authorize(&window)?;
    let (persona,session,revision) = accessory_target(&context)?;
    let id = parse_id(&id)?;
    let worker = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::accessories::images::get(&worker,persona,session,revision,id))
        .await.map_err(|_|AppError::storage())?
}
async fn change_accessory_wear(app:AppHandle,runtime:Runtime,context:ChatContextInput,id:String,worn:bool) -> AppResult<crate::models::Accessory> {
    let (persona,session,revision) = accessory_target(&context)?;
    let id = parse_id(&id)?;
    let item = storage(runtime,move|store,_| {
        crate::accessories::require_current(store,persona,session,revision)?;
        crate::accessories::wear(&mut store.connection,session,id,worn)
    }).await?;
    notify_accessories(&app,session);
    Ok(item)
}
#[tauri::command]
pub async fn wear_accessory(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,id:String) -> AppResult<crate::models::Accessory> {
    authorize(&window)?;
    change_accessory_wear(app,runtime.inner().clone(),context,id,true).await
}
#[tauri::command]
pub async fn unwear_accessory(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,id:String) -> AppResult<crate::models::Accessory> {
    authorize(&window)?;
    change_accessory_wear(app,runtime.inner().clone(),context,id,false).await
}
#[tauri::command]
pub async fn sell_accessory(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,id:String) -> AppResult<crate::models::AccessorySellResult> {
    authorize(&window)?;
    let (persona,session,revision) = accessory_target(&context)?;
    let id = parse_id(&id)?;
    let worker = runtime.inner().clone();
    let clock = worker.clock.clone();
    let (result,file) = storage(worker.clone(),move|store,_| {
        crate::accessories::require_current(store,persona,session,revision)?;
        crate::accessories::sell(&mut store.connection,session,id,clock.sample()?.utc)
    }).await?;
    if let Some(file) = file { crate::accessories::images::cleanup_one(&worker,&file,&[]); }
    notify_accessories(&app,session);
    let _ = app.emit(crate::events::WALLET_CHANGED,());
    Ok(result)
}
#[tauri::command]
pub async fn delete_accessory(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,id:String) -> AppResult<crate::models::AccessoryDeleteResult> {
    authorize(&window)?;
    let (persona,session,revision) = accessory_target(&context)?;
    let id = parse_id(&id)?;
    let worker = runtime.inner().clone();
    let (result,file) = storage(worker.clone(),move|store,_| {
        crate::accessories::require_current(store,persona,session,revision)?;
        crate::accessories::delete(&mut store.connection,session,id)
    }).await?;
    if let Some(file) = file { crate::accessories::images::cleanup_one(&worker,&file,&[]); }
    notify_accessories(&app,session);
    Ok(result)
}


fn authorize_image_settings(window:&WebviewWindow)->AppResult<()> {
    authorize(window)?;
    if window.label()!="settings" {return Err(AppError::new("PERMISSION_DENIED","请在设置窗口管理生图提供商"));}Ok(())
}
fn notify_image_config(app:&AppHandle,preview:Option<(String,String)>) {
    let _=app.emit(crate::events::IMAGE_GENERATION_CONFIG_CHANGED,());
    if let Some((session,preview))=preview {
        let _=app.emit(crate::events::ACCESSORY_CRAFT_PREVIEW_CHANGED,serde_json::json!({"sessionId":session,"previewId":preview}));
    }
}
#[tauri::command]
pub async fn get_image_generation_config(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<crate::image_generation::Config> {
    authorize(&window)?;storage(runtime.inner().clone(),|store,_|crate::image_generation::get(&store.connection)).await
}
#[tauri::command]
pub async fn save_image_generation_provider(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:Option<String>,expected_revision:u64,input:crate::image_generation::Input)->AppResult<crate::image_generation::Config> {
    authorize_image_settings(&window)?;
    let (result,preview)=storage(runtime.inner().clone(),move|store,secrets| {
        let config=crate::image_generation::save(&mut store.connection,id,expected_revision,input,secrets)?;
        Ok((config,crate::accessories::craft::configuration_changed(store)))
    }).await?;notify_image_config(&app,preview);Ok(result)
}
#[tauri::command]
pub async fn select_image_generation_provider(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:Option<String>,expected_revision:u64)->AppResult<crate::image_generation::Config> {
    authorize_image_settings(&window)?;
    let (result,preview)=storage(runtime.inner().clone(),move|store,_| {
        let config=crate::image_generation::select(&mut store.connection,id,expected_revision)?;
        Ok((config,crate::accessories::craft::configuration_changed(store)))
    }).await?;notify_image_config(&app,preview);Ok(result)
}
#[tauri::command]
pub async fn set_image_generation_enabled(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,enabled:bool,expected_revision:u64)->AppResult<crate::image_generation::Config> {
    authorize_image_settings(&window)?;
    let (result,preview)=storage(runtime.inner().clone(),move|store,_| {
        let config=crate::image_generation::set_enabled(&mut store.connection,enabled,expected_revision)?;
        Ok((config,crate::accessories::craft::configuration_changed(store)))
    }).await?;notify_image_config(&app,preview);Ok(result)
}
#[tauri::command]
pub async fn delete_image_generation_provider(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,expected_revision:u64)->AppResult<crate::image_generation::Config> {
    authorize_image_settings(&window)?;
    let (result,preview)=storage(runtime.inner().clone(),move|store,_| {
        let config=crate::image_generation::delete(&mut store.connection,&id,expected_revision)?;
        Ok((config,crate::accessories::craft::configuration_changed(store)))
    }).await?;notify_image_config(&app,preview);Ok(result)
}
#[tauri::command]
pub async fn get_accessory_craft_preview(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context,preview_id:String)->AppResult<crate::accessories::craft::Preview> {
    authorize(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::accessories::craft::get(&worker,&context,&preview_id)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn get_current_accessory_craft_preview(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context)->AppResult<Option<crate::accessories::craft::Preview>> {
    authorize(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::accessories::craft::current(&worker,&context)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn confirm_accessory_craft(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context,preview_id:String)->AppResult<crate::models::Accessory> {
    authorize(&window)?;let worker=runtime.inner().clone();let session=parse_id(&context.session_id)?;let preview=preview_id.clone();
    let result=tauri::async_runtime::spawn_blocking(move||crate::accessories::craft::confirm(&worker,&context,&preview_id)).await.map_err(|_|AppError::storage())??;
    notify_accessories(&app,session);let _=app.emit(crate::events::WALLET_CHANGED,());
    let _=app.emit(crate::events::ACCESSORY_CRAFT_PREVIEW_CHANGED,serde_json::json!({"sessionId":session.to_string(),"previewId":preview}));Ok(result)
}
#[tauri::command]
pub async fn cancel_accessory_craft(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context,preview_id:String)->AppResult<()> {
    authorize(&window)?;let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::accessories::craft::cancel(&worker,&context,&preview_id)).await.map_err(|_|AppError::storage())?
}

/// 所有伴手礼操作限定当前会话，前端不能指定任意归属或发放状态。
fn souvenir_scope(store:&Store,context:&crate::memory::management::Context)->AppResult<i64> {
    let persona=parse_id(&context.persona_id)?;let session=parse_id(&context.session_id)?;
    crate::accessories::require_current(store,persona,session,context.context_revision)?;Ok(session)
}
#[tauri::command]
pub async fn list_souvenirs(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context,before_id:Option<String>,limit:i64)->AppResult<Vec<crate::models::Souvenir>> {
    authorize(&window)?;let before=before_id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(),move|store,_|{let session=souvenir_scope(store,&context)?;crate::souvenirs::list(&store.connection,session,before,limit)}).await
}
#[tauri::command]
pub async fn get_souvenir(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context,id:String)->AppResult<crate::models::Souvenir> {
    authorize(&window)?;let id=parse_id(&id)?;
    storage(runtime.inner().clone(),move|store,_|{let session=souvenir_scope(store,&context)?;crate::souvenirs::get(&store.connection,session,id)}).await
}
#[tauri::command]
pub async fn set_souvenir_favorite(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context,id:String,favorite:bool)->AppResult<crate::models::Souvenir> {
    authorize(&window)?;let id=parse_id(&id)?;let session=context.session_id.clone();
    let result=storage(runtime.inner().clone(),move|store,_|{let session=souvenir_scope(store,&context)?;crate::souvenirs::favorite(&store.connection,session,id,favorite)}).await?;
    let _=app.emit(crate::events::TRAVEL_CHANGED,serde_json::json!({"sessionId":session}));Ok(result)
}
#[tauri::command]
pub async fn delete_souvenir(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context,id:String)->AppResult<()> {
    authorize(&window)?;let id=parse_id(&id)?;let session=context.session_id.clone();
    storage(runtime.inner().clone(),move|store,_|{let session=souvenir_scope(store,&context)?;crate::souvenirs::delete(&store.connection,session,id)}).await?;
    let _=app.emit(crate::events::TRAVEL_CHANGED,serde_json::json!({"sessionId":session}));Ok(())
}
#[tauri::command]
pub async fn get_gift_generation_status(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context)->AppResult<crate::souvenirs::generation::Status> {
    authorize(&window)?;let worker=runtime.travel_gifts.clone();
    storage(runtime.inner().clone(),move|store,_|{let session=souvenir_scope(store,&context)?;worker.status(&store.connection,session)}).await
}
#[tauri::command]
pub async fn retry_gift_generation(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context)->AppResult<()> {
    authorize(&window)?;let worker=runtime.travel_gifts.clone();let session=context.session_id.clone();
    storage(runtime.inner().clone(),move|store,_|{let session=souvenir_scope(store,&context)?;worker.retry(&store.connection,session)}).await?;
    let _=app.emit(crate::events::TRAVEL_CHANGED,serde_json::json!({"sessionId":session}));Ok(())
}

/// 游记管理只能读取/改动当前会话中的既有记录，不开放AI重新生成或任意插入。
fn journal_scope(store:&Store,context:&crate::memory::management::Context)->AppResult<i64> {
    let persona=parse_id(&context.persona_id)?;let session=parse_id(&context.session_id)?;
    crate::accessories::require_current(store,persona,session,context.context_revision).map_err(|e|if e.code=="CONTEXT_CHANGED" {AppError::new("CONTEXT_CHANGED","当前人设或会话已变化，请重新读取游记")} else {e})?;Ok(session)
}
#[tauri::command]
pub async fn list_travel_journals(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context,before_id:Option<String>,limit:i64)->AppResult<Vec<crate::models::TravelJournalSummary>> {
    authorize(&window)?;let before=before_id.as_deref().map(parse_id).transpose()?;
    storage(runtime.inner().clone(),move|store,_|{let session=journal_scope(store,&context)?;crate::travel_journals::list(&store.connection,session,before,limit)}).await
}
#[tauri::command]
pub async fn get_travel_journal(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context,id:String)->AppResult<crate::models::TravelJournal> {
    authorize(&window)?;let id=parse_id(&id)?;
    storage(runtime.inner().clone(),move|store,_|{let session=journal_scope(store,&context)?;crate::travel_journals::get(&store.connection,session,id)}).await
}
#[tauri::command]
pub async fn update_travel_journal(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context,id:String,travel_id:String,expected:crate::models::TravelJournalExpected,title:String,content:String)->AppResult<crate::models::TravelJournal> {
    authorize(&window)?;let id=parse_id(&id)?;let travel=parse_id(&travel_id)?;let session=context.session_id.clone();let clock=runtime.clock.clone();
    let result=storage(runtime.inner().clone(),move|store,_|{let session=journal_scope(store,&context)?;crate::travel_journals::update(&mut store.connection,session,travel,id,expected,&title,&content,clock.sample()?.utc)}).await?;
    let _=app.emit(crate::events::TRAVEL_CHANGED,serde_json::json!({"sessionId":session}));Ok(result)
}
#[tauri::command]
pub async fn delete_travel_journal(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context,id:String,travel_id:String)->AppResult<crate::models::TravelJournalDeleted> {
    authorize(&window)?;let id=parse_id(&id)?;let travel=parse_id(&travel_id)?;let session=context.session_id.clone();
    let result=storage(runtime.inner().clone(),move|store,_|{let session=journal_scope(store,&context)?;crate::travel_journals::delete(&mut store.connection,session,travel,id)}).await?;
    runtime.requests.cancel_key(&format!("travel-journal:{travel}"));
    let _=app.emit(crate::events::TRAVEL_CHANGED,serde_json::json!({"sessionId":session}));Ok(result)
}
#[tauri::command]
pub async fn get_journal_generation_status(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context)->AppResult<crate::travel_journals::generation::Status> {
    authorize(&window)?;let worker=runtime.travel_journals.clone();
    storage(runtime.inner().clone(),move|store,_|{let session=journal_scope(store,&context)?;worker.status(&store.connection,session)}).await
}
#[tauri::command]
pub async fn retry_journal_generation(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context)->AppResult<()> {
    authorize(&window)?;let worker=runtime.travel_journals.clone();let session=context.session_id.clone();
    storage(runtime.inner().clone(),move|store,_|{let session=journal_scope(store,&context)?;worker.retry(&store.connection,session)}).await?;
    let _=app.emit(crate::events::TRAVEL_CHANGED,serde_json::json!({"sessionId":session}));Ok(())
}

/// 母表只读；全局日期管理复用已有可信窗口权限与记录快照校验。
#[tauri::command]
pub async fn browse_cards(window:WebviewWindow,runtime:State<'_,Runtime>,input:crate::card_browser::Query)->AppResult<crate::card_browser::Page> {
    authorize(&window)?;storage(runtime.inner().clone(),move|s,_|crate::card_browser::browse(s,input)).await
}
#[tauri::command]
pub async fn get_calendar_month(window:WebviewWindow,runtime:State<'_,Runtime>,year:i32,month:u32)->AppResult<crate::festivals::calendar::Month> {
    authorize(&window)?;
    storage(runtime.inner().clone(),move|store,_| {
        crate::festivals::calendar::month(&store.connection,year,month)
    }).await
}
#[tauri::command]
pub async fn list_festival_catalog(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<Vec<crate::festivals::custom::CatalogRecord>> {
    authorize(&window)?;let clock=runtime.clock.clone();
    storage(runtime.inner().clone(),move|store,_|{let today=clock.sample()?.utc.with_timezone(&chrono::Local).date_naive();crate::festivals::synchronize(&mut store.connection,today)?;crate::festivals::custom::catalog(&store.connection,today)}).await
}
#[tauri::command]
pub async fn list_custom_festivals(window:WebviewWindow,runtime:State<'_,Runtime>,before_id:Option<String>,limit:i64)->AppResult<Vec<crate::festivals::custom::Record>> {
    authorize(&window)?;let before=before_id.as_deref().map(parse_id).transpose()?;let clock=runtime.clock.clone();
    storage(runtime.inner().clone(),move|store,_|{let today=clock.sample()?.utc.with_timezone(&chrono::Local).date_naive();crate::festivals::synchronize(&mut store.connection,today)?;crate::festivals::custom::list(&store.connection,before,limit,today)}).await
}
#[tauri::command]
pub async fn get_custom_festival(window:WebviewWindow,runtime:State<'_,Runtime>,id:String)->AppResult<crate::festivals::custom::Record> {
    authorize(&window)?;let id=parse_id(&id)?;let clock=runtime.clock.clone();
    storage(runtime.inner().clone(),move|store,_|{let today=clock.sample()?.utc.with_timezone(&chrono::Local).date_naive();crate::festivals::synchronize(&mut store.connection,today)?;crate::festivals::custom::get(&store.connection,id,today)}).await
}
#[tauri::command]
pub async fn create_custom_festival(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,input:crate::festivals::custom::Input)->AppResult<crate::festivals::custom::Record> {
    authorize(&window)?;let clock=runtime.clock.clone();
    let result=storage(runtime.inner().clone(),move|store,_|{let now=clock.sample()?.utc;let today=now.with_timezone(&chrono::Local).date_naive();crate::festivals::synchronize(&mut store.connection,today)?;crate::festivals::custom::create(&mut store.connection,input,"user",today,now)}).await?;
    let _=app.emit(crate::events::FESTIVALS_CHANGED,serde_json::json!({"sessionId":null}));Ok(result)
}
#[tauri::command]
pub async fn update_custom_festival(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,expected:String,input:crate::festivals::custom::Input)->AppResult<crate::festivals::custom::Record> {
    authorize(&window)?;let id=parse_id(&id)?;let clock=runtime.clock.clone();
    let result=storage(runtime.inner().clone(),move|store,_|{let now=clock.sample()?.utc;let today=now.with_timezone(&chrono::Local).date_naive();crate::festivals::synchronize(&mut store.connection,today)?;crate::festivals::custom::update(&mut store.connection,id,&expected,input,today,now)}).await?;
    let _=app.emit(crate::events::FESTIVALS_CHANGED,serde_json::json!({"sessionId":null}));Ok(result)
}
#[tauri::command]
pub async fn delete_custom_festival(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,id:String,expected:String)->AppResult<()> {
    authorize(&window)?;let id=parse_id(&id)?;let clock=runtime.clock.clone();
    storage(runtime.inner().clone(),move|store,_|{let today=clock.sample()?.utc.with_timezone(&chrono::Local).date_naive();crate::festivals::synchronize(&mut store.connection,today)?;crate::festivals::custom::delete(&mut store.connection,id,&expected,today)}).await?;
    let _=app.emit(crate::events::FESTIVALS_CHANGED,serde_json::json!({"sessionId":null}));Ok(())
}

#[tauri::command]
pub async fn get_chat_image(window:WebviewWindow,runtime:State<'_,Runtime>,context:ChatContextInput,id:String)->AppResult<String> {
    authorize(&window)?;let worker=runtime.inner().clone();let id=parse_id(&id)?;
    tauri::async_runtime::spawn_blocking(move||{
        let image=worker.read(|store,_|{
            let persona=parse_id(&context.persona_id)?;let session=parse_id(&context.session_id)?;
            validate_persona_context(store,persona,context.context_revision)?;
            let current=crate::persona::context(&store.connection,store.context_revision)?;
            if current.session_id.as_deref()!=Some(&context.session_id){return Err(AppError::new("CONTEXT_CHANGED","当前会话已变化"));}
            let message:i64=store.connection.query_row("SELECT m.id FROM message_images i JOIN messages m ON m.id=i.message_id WHERE i.id=?1 AND m.session_id=?2",[id,session],|r|r.get(0)).optional()?.ok_or_else(AppError::not_found)?;
            crate::chat_images::list(&store.connection,message)?.into_iter().find(|image|image.id==id.to_string()).ok_or_else(AppError::not_found)
        })?;
        crate::chat_images::data_url(worker.paths.as_ref().map_err(Clone::clone)?,&image)
    }).await.map_err(|_|AppError::storage())?
}

/// SMTP管理只开放给可信设置窗口，不提供给小游戏或聊天工具。
#[tauri::command]
pub async fn get_reminder_email(window:WebviewWindow,runtime:State<'_,Runtime>)->AppResult<crate::reminder_email::View> {
    authorize_backup(&window)?;
    storage(runtime.inner().clone(),|s,_|crate::reminder_email::view(&s.connection)).await
}
#[tauri::command]
pub async fn save_reminder_email(window:WebviewWindow,runtime:State<'_,Runtime>,input:crate::reminder_email::Input)->AppResult<crate::reminder_email::View> {
    authorize_backup(&window)?;
    storage(runtime.inner().clone(),move|s,secrets|crate::reminder_email::save(&mut s.connection,secrets,input)).await
}
#[tauri::command]
pub async fn send_reminder_email_code(window:WebviewWindow,runtime:State<'_,Runtime>,input:crate::reminder_email::Input)->AppResult<crate::reminder_email::View> {
    authorize_backup(&window)?;
    crate::reminder_email::send_code(runtime.inner(),input).await
}
#[tauri::command]
pub async fn verify_reminder_email(window:WebviewWindow,runtime:State<'_,Runtime>,code:String,revision:u64)->AppResult<crate::reminder_email::View> {
    authorize_backup(&window)?;
    let worker=runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move||crate::reminder_email::verify(&worker,&code,revision)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn set_reminder_email_enabled(window:WebviewWindow,runtime:State<'_,Runtime>,enabled:bool,revision:u64)->AppResult<crate::reminder_email::View> {
    authorize_backup(&window)?;
    let worker=runtime.inner().clone();let clock=worker.clock.clone();
    storage(worker,move|s,_|crate::reminder_email::enable(&mut s.connection,enabled,revision,clock.sample()?.utc)).await
}
#[tauri::command]
pub async fn retry_reminder_email(window:WebviewWindow,runtime:State<'_,Runtime>,id:String,revision:u64,confirm_uncertain:bool)->AppResult<crate::reminder_email::View> {
    authorize_backup(&window)?;
    let id=parse_id(&id)?;
    storage(runtime.inner().clone(),move|s,_|crate::reminder_email::outbox_retry(&mut s.connection,id,revision,confirm_uncertain)).await
}

#[tauri::command]
pub async fn get_pet_creation(window:WebviewWindow,runtime:State<'_,Runtime>,persona_id:String)->AppResult<crate::pet_creation::Library>{authorize(&window)?;let id=parse_id(&persona_id)?;storage(runtime.inner().clone(),move|s,_|crate::pet_creation::library(&s.connection,id)).await}
#[tauri::command]
pub async fn save_pet_artwork(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,id:Option<String>,revision:Option<u64>,input:crate::pet_creation::Input)->AppResult<crate::pet_creation::Artwork>{authorize_backup(&window)?;let persona=parse_id(&persona_id)?;let id=id.as_deref().map(parse_id).transpose()?;let result=storage(runtime.inner().clone(),move|s,_|crate::pet_creation::save(&mut s.connection,persona,id,revision,input)).await?;let _=app.emit("pet_creation_changed",());Ok(result)}
#[tauri::command]
pub async fn set_pet_creation_enabled(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,enabled:bool,revision:u64)->AppResult<()>{authorize_backup(&window)?;let id=parse_id(&persona_id)?;storage(runtime.inner().clone(),move|s,_|crate::pet_creation::set_enabled(&s.connection,id,enabled,revision)).await?;let _=app.emit("pet_creation_changed",());Ok(())}
#[tauri::command]
pub async fn get_pet_artwork_image(window:WebviewWindow,runtime:State<'_,Runtime>,persona_id:String,id:String,preview:bool)->AppResult<String>{authorize(&window)?;let persona=parse_id(&persona_id)?;let id=parse_id(&id)?;let worker=runtime.inner().clone();tauri::async_runtime::spawn_blocking(move||crate::pet_creation::image(&worker,persona,id,preview)).await.map_err(|_|AppError::storage())?}
#[tauri::command]
pub async fn upload_pet_artwork(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,id:String,revision:u64,data:String)->AppResult<crate::pet_creation::Artwork>{authorize_backup(&window)?;use base64::Engine;let persona=parse_id(&persona_id)?;let id=parse_id(&id)?;if data.len()>crate::accessories::images::MAX_BYTES.div_ceil(3)*4+128{return Err(AppError::validation("image","图片超过16MB，请压缩图片后上传"));}let bytes=base64::engine::general_purpose::STANDARD.decode(data.split_once(',').map(|(_,s)|s).unwrap_or(&data)).map_err(|_|AppError::validation("image","图片编码无效"))?;let worker=runtime.inner().clone();let result=tauri::async_runtime::spawn_blocking(move||crate::pet_creation::preview(&worker,persona,id,revision,&bytes)).await.map_err(|_|AppError::storage())??;let _=app.emit("pet_creation_changed",());Ok(result)}
#[tauri::command]
pub async fn finish_pet_artwork(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,id:String,revision:u64,action:String)->AppResult<()>{authorize_backup(&window)?;let persona=parse_id(&persona_id)?;let id=parse_id(&id)?;let removed=storage(runtime.inner().clone(),move|s,_|crate::pet_creation::finish(&mut s.connection,persona,id,revision,&action)).await?;for file in removed{crate::accessories::images::cleanup_one(runtime.inner(),&file,&[]);}let _=app.emit("pet_creation_changed",());Ok(())}
#[tauri::command]
pub async fn generate_pet_artwork(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,persona_id:String,id:String,revision:u64)->AppResult<crate::pet_creation::Artwork>{authorize_backup(&window)?;let persona=parse_id(&persona_id)?;let id=parse_id(&id)?;let result=crate::pet_creation::generate(runtime.inner().clone(),persona,id,revision).await?;let _=app.emit("pet_creation_changed",());Ok(result)}
#[tauri::command]
pub async fn cancel_pet_artwork(window:WebviewWindow,runtime:State<'_,Runtime>,persona_id:String,id:String)->AppResult<()>{authorize_backup(&window)?;let persona=parse_id(&persona_id)?;let id=parse_id(&id)?;runtime.requests.cancel_key(&format!("pet-art:{persona}:{id}"));Ok(())}
#[tauri::command]
pub async fn save_accessory_placement(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:ChatContextInput,id:String,x:Option<i32>,y:Option<i32>,scale:i32)->AppResult<crate::models::Accessory>{authorize_backup(&window)?;let (persona,session,revision)=accessory_target(&context)?;let id=parse_id(&id)?;let item=storage(runtime.inner().clone(),move|s,_|{crate::accessories::require_current(s,persona,session,revision)?;crate::accessories::placement(&s.connection,session,id,x,y,scale)}).await?;notify_accessories(&app,session);Ok(item)}
