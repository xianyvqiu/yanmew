use crate::pet_region::{self, Rect};
use rusqlite::OptionalExtension;

use crate::{
    error::{parse_id, AppError, AppResult},
    runtime::{Runtime, Store},
};
use tauri::{State, WebviewWindow};
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
    persona_id: String, session_id: String, context_revision: u64, input: String,
) -> AppResult<crate::chat::RequestSnapshot> {
    authorize(&window)?;
    let persona_id = parse_id(&persona_id)?;
    let session_id = parse_id(&session_id)?;
    let runtime = runtime.inner().clone();
    let client = runtime.ai.clone()?;
    let title_runtime = runtime.clone();
    let (mut prepared, receipt) = tauri::async_runtime::spawn_blocking(move ||
        crate::chat::submit(&runtime, persona_id, session_id, context_revision, input)
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
            .query_row("SELECT id FROM providers WHERE is_active=1", [], |r| {
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
                .query_row("SELECT is_active FROM providers WHERE id=?1", [id], |r| {
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
    }).await;
    if result.is_ok() {client.discover_capacity(&config).await;}
    let health_result = result.clone();
    let worker = runtime.clone();
    storage(runtime, move |s, p| {
        s.pet.health(&s.connection, p)?;
        if let Some(token) = token {
            if !s.pet.complete_health(&token, &health_result) {
                return Err(AppError::new("CONTEXT_CHANGED", "提供商配置或恢复状态已改变，旧连接测试结果已忽略"));
            }
        }
        // 只对实际记忆提供商的成功测试解除失败等待，不重复请求不相关模型。
        if health_result.is_ok() {
            let selected: Option<i64> = s.connection.query_row(
                "SELECT id FROM providers WHERE memory_is_active=1 OR is_active=1 ORDER BY memory_is_active DESC LIMIT 1",
                [], |row| row.get(0),
            ).optional()?;
            if selected == Some(id) { worker.memory.retry(None)?; }
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
pub async fn focus_or_create_window(window: WebviewWindow, app: AppHandle, page: crate::desktop_windows::Page) -> AppResult<()> {
    authorize(&window)?;
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
    Ok(std::mem::take(&mut *pending))
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
    let result = storage(runtime.inner().clone(), move |s, p| {
        let result = crate::providers::save(&mut s.connection, Some(id), input, p)?;
        if result.is_active {
            requests.cancel_all();
        }
        Ok(result)
    })
    .await?;
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
            "SELECT EXISTS(SELECT 1 FROM providers WHERE is_active=1)",
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
            "SELECT EXISTS(SELECT 1 FROM providers WHERE id=?1 AND is_active=1)",
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
            s.context_revision += 1;
            after.context_revision = s.context_revision;
            s.pet.reset_context();
            worker.settle_pet(s, p)?;
        }
        Ok((after, changed))
    })
    .await?;
    if delete { let _ = app.emit("configuration_changed", ()); }
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

#[derive(Clone, Copy, Default, Serialize)]
pub struct Snapshot {
    pub hidden: bool,
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
            self.full_size.unwrap_or(LogicalSize::new(320.0, 240.0))
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
            Some(
                window
                    .inner_size()
                    .map_err(|e| format!("读取宠窗尺寸失败：{e}"))?
                    .to_logical(
                        window
                            .scale_factor()
                            .map_err(|e| format!("读取屏幕缩放失败：{e}"))?,
                    ),
            )
        } else {
            state.full_size
        };
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
