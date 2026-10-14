use super::*;
use crate::{
    currency::{self, CoinChange, PluginLedgerContext},
    error::parse_id,
    models::CurrencyReason,
};
use rusqlite::TransactionBehavior;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub plugin_id: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub wallet_permission_granted: bool,
    pub status: String,
    pub outcome: Option<String>,
    pub details: Option<Value>,
    pub coin_delta: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
    let raw: Option<String> = r.get(7)?;
    Ok(Run {
        id: r.get::<_, i64>(0)?.to_string(),
        plugin_id: r.get(1)?,
        plugin_name: r.get(2)?,
        plugin_version: r.get(3)?,
        wallet_permission_granted: r.get(4)?,
        status: r.get(5)?,
        outcome: r.get(6)?,
        details: raw
            .map(|s| serde_json::from_str(&s))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        coin_delta: r.get::<_, i64>(8)?.to_string(),
        started_at: r.get(9)?,
        ended_at: r.get(10)?,
    })
}
pub fn get_run(c: &Connection, id: &str) -> AppResult<Run> {
    c.query_row("SELECT id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,outcome,details_json,coin_delta,started_at,ended_at FROM game_runs WHERE id=?1",[parse_id(id)?],row).optional()?.ok_or_else(AppError::not_found)
}
pub fn history(
    c: &Connection,
    id: &str,
    before: Option<String>,
    limit: usize,
) -> AppResult<Vec<Run>> {
    let before = before
        .map(|v| parse_id(&v))
        .transpose()?
        .unwrap_or(i64::MAX);
    let mut s=c.prepare("SELECT id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,outcome,details_json,coin_delta,started_at,ended_at FROM game_runs WHERE plugin_id=?1 AND id<?2 ORDER BY id DESC LIMIT ?3")?;
    let rows = s.query_map(params![id, before, limit.clamp(1, 100)], row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}
pub fn start(c: &Connection, id: &str, version: &str, now: DateTime<Utc>) -> AppResult<Run> {
    let p = authorize(c, id, version)?;
    let wallet = allowed(c, id, version, "wallet.modify", "")?;
    c.execute("INSERT INTO game_runs(plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,started_at) VALUES(?1,?2,?3,?4,'running',?5)",params![id,p.name,version,wallet,stamp(now)])?;
    get_run(c, &c.last_insert_rowid().to_string())
}
pub fn complete(
    c: &Connection,
    id: &str,
    version: &str,
    run_id: &str,
    outcome: &str,
    details: Option<Value>,
    now: DateTime<Utc>,
) -> AppResult<Run> {
    authorize(c, id, version)?;
    if outcome.trim().is_empty() || outcome.len() > 256 {
        return Err(AppError::validation(
            "outcome",
            "结果不能为空或超过 256 字节",
        ));
    }
    let raw = details
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| AppError::storage())?;
    if raw.as_ref().is_some_and(|s| s.len() > 64 * 1024) {
        return Err(AppError::validation("details", "运行明细超过 64 KiB"));
    }
    let run = get_run(c, run_id)?;
    if run.plugin_id != id || run.plugin_version != version {
        return Err(denied());
    }
    if run.status != "running" {
        if run.status == "completed"
            && run.outcome.as_deref() == Some(outcome)
            && run.details == details
        {
            return Ok(run);
        }
        return Err(AppError::new(
            "PLUGIN_RUN_FINISHED",
            "运行记录已结束，不能覆盖结果",
        ));
    }
    c.execute("UPDATE game_runs SET status='completed',outcome=?2,details_json=?3,ended_at=?4 WHERE id=?1 AND status='running'",params![parse_id(run_id)?,outcome,raw,stamp(now)])?;
    get_run(c, run_id)
}
pub fn stop_plugin(c: &Connection, id: &str, status: &str, now: DateTime<Utc>) -> AppResult<()> {
    if !["cancelled", "error"].contains(&status) {
        return Err(AppError::validation("status", "结束状态无效"));
    }
    c.execute(
        "UPDATE game_runs SET status=?2,ended_at=?3 WHERE plugin_id=?1 AND status='running'",
        params![id, status, stamp(now)],
    )?;
    Ok(())
}
pub fn recover(c: &Connection, now: DateTime<Utc>) -> AppResult<()> {
    c.execute(
        "UPDATE game_runs SET status='error',ended_at=?1 WHERE status='running'",
        [stamp(now)],
    )?;
    Ok(())
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub id: String,
    pub game_run_id: String,
    pub plugin_id: String,
    pub plugin_name: String,
    pub plugin_version: String,
    pub delta: String,
    pub reason: String,
    pub request_id: String,
    pub created_at: String,
}
pub fn ledger(
    c: &Connection,
    id: &str,
    before: Option<String>,
    limit: usize,
) -> AppResult<Vec<LedgerEntry>> {
    let before = before
        .map(|id| parse_id(&id))
        .transpose()?
        .unwrap_or(i64::MAX);
    let mut q=c.prepare("SELECT l.id,l.game_run_id,l.plugin_id,r.plugin_name,l.plugin_version,l.delta,l.plugin_display_reason,l.wallet_request_id,l.created_at FROM currency_ledger l JOIN game_runs r ON r.id=l.game_run_id WHERE l.reason='plugin_game' AND l.plugin_id=?1 AND l.id<?2 ORDER BY l.id DESC LIMIT ?3")?;
    let rows = q.query_map(params![id, before, limit.clamp(1, 100)], |r| {
        Ok(LedgerEntry {
            id: r.get::<_, i64>(0)?.to_string(),
            game_run_id: r.get::<_, i64>(1)?.to_string(),
            plugin_id: r.get(2)?,
            plugin_name: r.get(3)?,
            plugin_version: r.get(4)?,
            delta: r.get::<_, i64>(5)?.to_string(),
            reason: r.get(6)?,
            request_id: r.get(7)?,
            created_at: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRequest {
    pub request_id: String,
    pub game_run_id: String,
    pub delta: i64,
    pub reason: String,
    pub details: Option<Value>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletResult {
    pub ledger_id: String,
    pub applied_delta: String,
    pub duplicate: bool,
    pub balance: String,
}
/// id/version/current_run 只能来自可信窗口实例，不能取自插件参数。
pub fn change_coins(
    c: &mut Connection,
    id: &str,
    version: &str,
    current_run: &str,
    request: &WalletRequest,
    now: DateTime<Utc>,
) -> AppResult<WalletResult> {
    if request.request_id.trim().is_empty()
        || request.request_id.len() > 128
        || request.reason.trim().is_empty()
        || request.reason.chars().count() > 200
        || request.delta.unsigned_abs() > 9_007_199_254_740_991
    {
        return Err(AppError::validation(
            "request",
            "钱包请求 ID、原因或整数变化无效",
        ));
    }
    let run_id = parse_id(&request.game_run_id)?;
    let mut tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let old:Option<(i64,i64,i64,String)>=tx.query_row("SELECT id,game_run_id,delta,plugin_display_reason FROM currency_ledger WHERE plugin_id=?1 AND wallet_request_id=?2",params![id,request.request_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    if let Some((ledger, old_run, delta, reason)) = old {
        if old_run != run_id || delta != request.delta || reason != request.reason {
            return Err(AppError::new(
                "PLUGIN_REQUEST_CONFLICT",
                "相同钱包请求 ID 携带了不同内容",
            ));
        }
        let balance = currency::get_balance(&tx)?;
        tx.commit()?;
        return Ok(WalletResult {
            ledger_id: ledger.to_string(),
            applied_delta: delta.to_string(),
            duplicate: true,
            balance: balance.to_string(),
        });
    }
    authorize(&tx, id, version)?;
    // 备份不一定包含旧运行；未结算的旧请求统一拒绝，SDK可保留旧进度并取消该操作。
    // 已结算请求仍由上面的唯一请求核对返回，不重放历史改币。
    if request.game_run_id != current_run {return Err(denied());}
    let run = get_run(&tx, &request.game_run_id)?;
    if run.plugin_id != id
        || run.plugin_version != version
        || run.status != "running"
        || !run.wallet_permission_granted
        || !allowed(&tx, id, version, "wallet.modify", "")?
    {
        return Err(denied());
    }
    let total = run
        .coin_delta
        .parse::<i64>()
        .map_err(|_| AppError::storage())?
        .checked_add(request.delta)
        .ok_or_else(|| AppError::new("COIN_OVERFLOW", "本次运行累计喵币变化超出范围"))?;
    let raw = request
        .details
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|_| AppError::storage())?;
    if raw.as_ref().is_some_and(|s| s.len() > 64 * 1024) {
        return Err(AppError::validation("details", "运行明细超过 64 KiB"));
    }
    let applied = currency::apply_delta_in_tx(
        &mut tx,
        &CoinChange {
            delta: request.delta,
            reason: CurrencyReason::PluginGame,
            ref_id: None,
            plugin: Some(PluginLedgerContext {
                game_run_id: run_id,
                plugin_id: id.into(),
                plugin_version: version.into(),
                display_reason: request.reason.clone(),
                request_id: request.request_id.clone(),
            }),
        },
        now,
    )?;
    tx.execute(
        "UPDATE game_runs SET coin_delta=?2,details_json=COALESCE(?3,details_json) WHERE id=?1",
        params![run_id, total, raw],
    )?;
    tx.commit()?;
    Ok(WalletResult {
        ledger_id: applied.ledger_id.to_string(),
        applied_delta: request.delta.to_string(),
        duplicate: false,
        balance: applied.balance.to_string(),
    })
}
