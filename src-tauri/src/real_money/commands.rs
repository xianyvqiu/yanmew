use super::*;
use crate::runtime::Runtime;
use tauri::{Emitter, State, WebviewWindow};
fn allow(w: &WebviewWindow, notice: bool) -> AppResult<()> {
    if w.label() == "settings" || (notice && w.label() == "main") {
        Ok(())
    } else {
        Err(AppError::new("PERMISSION_DENIED", "请在现实记账页面操作"))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mutation {
    pub request_id: String,
    pub action: Action,
}
#[tauri::command]
pub async fn get_real_money_operation(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    request_id: String,
) -> AppResult<Option<Value>> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| {
            s.connection
                .query_row(
                    "SELECT result_json FROM real_operations WHERE request_id=?1",
                    [request_id],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .map(|text| serde_json::from_str(&text).map_err(|_| AppError::storage()))
                .transpose()
        })
    })
    .await
    .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn get_real_money_overview(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Value> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| records::overview(&s.connection, rt.clock.sample()?.utc))
    })
    .await
    .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn query_real_money(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    input: records::Query,
) -> AppResult<Value> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| records::query(&s.connection, input, rt.clock.sample()?.utc))
    })
    .await
    .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn mutate_real_money(
    window: WebviewWindow,
    app: tauri::AppHandle,
    runtime: State<'_, Runtime>,
    input: Mutation,
) -> AppResult<Value> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    let value = tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| {
            mutate(
                &mut s.connection,
                &input.request_id,
                input.action,
                rt.clock.sample()?.utc,
                Actor {
                    source: "manual",
                    name: "用户".into(),
                },
            )
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    let _ = app.emit("real-money-changed", ());
    Ok(value)
}
#[tauri::command]
pub async fn list_real_money_tasks(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    input: cycles::ListQuery,
) -> AppResult<Value> {
    allow(&window, false)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rt.read(|s, _| cycles::list(&s.connection, input)))
        .await
        .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn get_real_money_calendar(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
    year: i32,
    month: u32,
) -> AppResult<Value> {
    allow(&window, false)?;
    let start = NaiveDate::from_ymd_opt(year, month, 1)
        .filter(|_| (1950..=2100).contains(&year))
        .ok_or_else(|| invalid("日历年份范围为1950至2100"))?;
    let end = start
        .checked_add_months(chrono::Months::new(1))
        .and_then(|d| d.pred_opt())
        .ok_or_else(|| invalid("月份无效"))?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| cycles::calendar(&s.connection, start, end))
    })
    .await
    .map_err(|_| AppError::storage())?
}
#[tauri::command]
pub async fn get_real_money_pending(
    window: WebviewWindow,
    runtime: State<'_, Runtime>,
) -> AppResult<Value> {
    allow(&window, true)?;
    let rt = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        rt.read(|s, _| {
            let (count, id): (i64, Option<i64>) = s.connection.query_row(
                "SELECT count(*),max(id) FROM real_occurrences WHERE status='pending'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            Ok(json!({"count":count,"throughId":id.map(|v|v.to_string())}))
        })
    })
    .await
    .map_err(|_| AppError::storage())?
}
