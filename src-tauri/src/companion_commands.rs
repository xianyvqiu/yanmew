//! 食品与Skill可信宿主命令；不向小游戏开放。
use crate::{
    error::{parse_id, AppError, AppResult},
    foods,
    runtime::Runtime,
    skills,
};
use tauri::{AppHandle, Emitter, State, WebviewWindow};
fn allow(w: &WebviewWindow, main: bool) -> AppResult<()> {
    if w.label() == "settings" || (main && w.label() == "main") {
        Ok(())
    } else {
        Err(AppError::new("PERMISSION_DENIED", "此窗口不能执行该操作"))
    }
}
#[tauri::command]
pub async fn query_currency_ledger(window: WebviewWindow, runtime: State<'_, Runtime>, input: crate::currency::ledger::Query) -> AppResult<crate::currency::ledger::Report> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rt.read(|s, _| crate::currency::ledger::read(&s.connection, input)))
        .await.map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn list_foods(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Vec<foods::Food>> {
    allow(&window, true)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rt.read(|s, _| foods::list(&s.connection)))
        .await
        .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn get_food_preferences(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    persona_id: String,
) -> AppResult<foods::Preferences> {
    allow(&window, true)?;
    runtime.read(|s, _| foods::preferences(&s.connection, parse_id(&persona_id)?))
}
#[tauri::command]
pub async fn save_food_preferences(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    persona_id: String,
    default_food_id: String,
    favorite_food_id: Option<String>,
    field: String,
) -> AppResult<()> {
    allow(&window, true)?;
    runtime.read(|s, _| {
        let id = parse_id(&persona_id)?;
        let mut value = foods::preferences(&s.connection, id)?;
        match field.as_str() {
            "default" => value.default_food_id = default_food_id,
            "favorite" => {
                if window.label() != "settings" {
                    return Err(AppError::new("PERMISSION_DENIED", "请在设置中修改喜好"));
                }
                value.favorite_food_id = favorite_food_id
            }
            _ => return Err(AppError::validation("field", "无效配置字段")),
        };
        foods::set_preferences(&s.connection, id, value)
    })
}
#[tauri::command]
pub async fn buy_food(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    action_id: String,
    food_id: String,
    quantity: i64,
    quote: i64,
) -> AppResult<foods::Receipt> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| foods::buy(&mut s.connection, &action_id, &food_id, quantity, quote))
    })
    .await
    .map_err(|_| AppError::storage())??;
    let _ = app.emit(crate::events::WALLET_CHANGED, ());
    let _ = app.emit("foods-changed", ());
    Ok(r)
}
#[tauri::command]
pub async fn get_food_action(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    action_id: String,
) -> AppResult<Option<foods::Receipt>> {
    allow(&window, true)?;
    runtime.read(|s, _| foods::receipt(&s.connection, &action_id))
}
#[tauri::command]
pub async fn feed_food(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    action_id: String,
    food_id: String,
    persona_id: String,
    session_id: String,
    context_revision: u64,
) -> AppResult<foods::Receipt> {
    if window.label() != "main" {
        return Err(AppError::new("PERMISSION_DENIED", "请在宠窗完成投喂"));
    }
    let rt = runtime.inner().clone();
    let r = tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, p| {
            let view = rt.settle_pet(s, p)?;
            if view.persona_id != persona_id
                || view.session_id.as_deref() != Some(&session_id)
                || view.context_revision != context_revision
            {
                return Err(AppError::new("CONTEXT_CHANGED", "当前会话已变化"));
            }
            if !view.can_interact {
                return Err(AppError::new("PET_UNAVAILABLE", "桌宠当前不能投喂"));
            }
            let sid = parse_id(&session_id)?;
            let snapshot = crate::pet_store::read(&s.connection, sid)?;
            let sample = rt.clock.sample()?;
            let mut random = rt.random.lock().map_err(|_| AppError::storage())?;
            let r = foods::feed(
                &mut s.connection,
                &action_id,
                &food_id,
                context_revision,
                crate::pet_store::PetWriteScope {
                    persona_id: parse_id(&persona_id)?,
                    session_id: sid,
                    context_revision,
                    state_revision: snapshot.state_revision,
                },
                random.as_mut(),
            )?;
            if !r.replayed {
                s.pet.food_feedback(
                    &s.connection,
                    parse_id(&persona_id)?,
                    foods::preferences(&s.connection, parse_id(&persona_id)?)?
                        .favorite_food_id
                        .as_deref()
                        == Some(&food_id),
                    sample,
                    random.as_mut(),
                )?;
                rt.requests.cancel_key(&format!("proactive:{session_id}"));
            }
            Ok(r)
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    let _ = app.emit("foods-changed", ());
    if let Ok(v) = runtime.pet_view() {
        let _ = app.emit(crate::events::PET_ACTIVITY_CHANGED, v);
    }
    Ok(r)
}
#[tauri::command]
pub async fn list_skills(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Vec<skills::Skill>> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rt.read(|s, _| skills::list(&s.connection)))
        .await
        .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn save_skill(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    input: skills::Input,
) -> AppResult<()> {
    allow(&window, false)?;
    runtime.read(|s, _| skills::save(&mut s.connection, input))?;
    let _=app.emit("skills-changed",());Ok(())
}
#[tauri::command]
pub async fn delete_skill(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    id: String,
    revision: i64,
) -> AppResult<()> {
    allow(&window, false)?;
    runtime.read(|s, _| {
        if s.connection.execute(
            "DELETE FROM skills WHERE id=?1 AND revision=?2",
            rusqlite::params![parse_id(&id)?, revision],
        )? != 1
        {
            return Err(AppError::new("SKILL_CONFLICT", "Skill已变化，请重新读取"));
        }
        Ok(())
    })?;
    let _=app.emit("skills-changed",());Ok(())
}
#[tauri::command]
pub async fn bind_skill(
    window: WebviewWindow,
    app: AppHandle,
    runtime: State<'_, Runtime>,
    skill_id: String,
    persona_id: String,
    enabled: bool,
) -> AppResult<()> {
    allow(&window, false)?;
    runtime.read(|s, _| {
        skills::bind(
            &s.connection,
            parse_id(&skill_id)?,
            parse_id(&persona_id)?,
            enabled,
        )
    })?;
    let _=app.emit("skills-changed",());Ok(())
}

#[tauri::command]
pub async fn get_skill_previews(window:WebviewWindow,runtime:State<'_,Runtime>,context:crate::memory::management::Context)->AppResult<Vec<skills::tools::SharedPreview>> {
    if !matches!(window.label(),"main"|"chat"){return Err(AppError::new("PERMISSION_DENIED","只有聊天入口能读取Skill确认"));}
    let rt=runtime.inner().clone();tauri::async_runtime::spawn_blocking(move||skills::tools::pending(&rt,context)).await.map_err(|_|AppError::storage())?
}
#[tauri::command]
pub async fn resolve_skill_preview(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,context:crate::memory::management::Context,id:String,accept:bool)->AppResult<()> {
    if !matches!(window.label(),"main"|"chat"){return Err(AppError::new("PERMISSION_DENIED","只有聊天入口能确认Skill修改"));}
    let rt=runtime.inner().clone();tauri::async_runtime::spawn_blocking(move||skills::tools::resolve(&rt,context,&id,accept)).await.map_err(|_|AppError::storage())??;
    let _=app.emit("skills-changed",());Ok(())
}

#[tauri::command]
pub async fn get_food_expression(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    persona_id: String,
    favorite: bool,
) -> AppResult<String> {
    allow(&window, true)?;
    runtime.read(|s, _| {
        let mut random = runtime.random.lock().map_err(|_| AppError::storage())?;
        foods::expression(
            &s.connection,
            parse_id(&persona_id)?,
            if favorite {
                "craving_favorite"
            } else {
                "craving"
            },
            random.as_mut(),
        )
    })
}
