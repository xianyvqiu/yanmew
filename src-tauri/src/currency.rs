//! 全局钱包与账单的唯一变更服务。前端开放读取与签到，其余写入由可信业务入口调用。
use crate::{
    error::{AppError, AppResult},
    models::{CurrencyReason, SessionIntimacy, Wallet},
};
use chrono::{DateTime, Days, Duration, SecondsFormat, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

mod sign_in;
pub mod ledger;
pub use sign_in::{daily_sign_in, get_sign_in_status};

/// 宿主校验身份、权限、游戏局状态和幂等请求后传入的历史快照。
/// 本结构不是授权凭据，也不接受来自插件的未验证身份。
pub struct PluginLedgerContext {
    pub game_run_id: i64,
    pub plugin_id: String,
    pub plugin_version: String,
    pub display_reason: String,
    pub request_id: String,
}

/// 一次已由业务确定的变动；制作/卖出使用 ref_id，插件使用独立快照。
pub struct CoinChange {
    pub delta: i64,
    pub reason: CurrencyReason,
    pub ref_id: Option<i64>,
    pub plugin: Option<PluginLedgerContext>,
}

/// 同事务已应用的结果；只有最外层事务成功提交后才可通知用户。
#[derive(Debug, PartialEq, Eq)]
pub struct AppliedDelta {
    pub ledger_id: i64,
    pub balance: i64,
}

fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn get_wallet(connection: &Connection) -> AppResult<Wallet> {
    let (coins, updated_at): (i64, String) = connection
        .query_row(
            "SELECT coins,updated_at FROM user_state WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::new("WALLET_UNAVAILABLE", "全局钱包尚未初始化，请重新启动应用"))?;
    if coins < 0 {
        return Err(AppError::storage());
    }
    Ok(Wallet {
        coins: coins.to_string(),
        updated_at,
    })
}

/// 业务计算使用 i64；不得将余额字符串转成浮点数进行记账。
pub fn get_balance(connection: &Connection) -> AppResult<i64> {
    get_wallet(connection)?
        .coins
        .parse()
        .map_err(|_| AppError::storage())
}

fn validate(change: &CoinChange) -> AppResult<()> {
    use CurrencyReason::*;
    let valid = match change.reason {
        DailySignIn => change.delta >= 0 && change.ref_id.is_none() && change.plugin.is_none(),
        SignInFestival => change.delta == 10 && change.ref_id.is_none() && change.plugin.is_none(),
        AccessoryCraft => {
            change.delta <= 0 && change.ref_id.is_some_and(|id| id > 0) && change.plugin.is_none()
        }
        AccessorySell => {
            change.delta >= 0 && change.ref_id.is_some_and(|id| id > 0) && change.plugin.is_none()
        }
        FoodPurchase => change.delta < 0 && change.ref_id.is_none() && change.plugin.is_none(),
        PluginGame => {
            change.ref_id.is_none()
                && change.plugin.as_ref().is_some_and(|plugin| {
                    plugin.game_run_id > 0
                        && !plugin.plugin_id.trim().is_empty()
                        && !plugin.plugin_version.trim().is_empty()
                        && !plugin.display_reason.trim().is_empty()
                        && !plugin.request_id.trim().is_empty()
                })
        }
    };
    if !valid {
        return Err(AppError::validation(
            "change",
            "喵币变动数量、原因或来源不匹配",
        ));
    }
    Ok(())
}

/// 单独变更入口；签到、饰品和插件等联合事务必须改用 apply_delta_in_tx。
pub fn apply_delta(
    connection: &mut Connection,
    change: &CoinChange,
    now: DateTime<Utc>,
) -> AppResult<AppliedDelta> {
    let mut transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let applied = apply_delta_in_tx(&mut transaction, change, now)?;
    transaction.commit()?;
    Ok(applied)
}

/// 不提交外层事务；保存点保证写账单失败时余额也回滚，即使调用方处理了错误。
/// 本函数只负责记账，不能代替签到去重、饰品资格或插件授权/幂等校验。
pub fn apply_delta_in_tx(
    transaction: &mut Transaction<'_>,
    change: &CoinChange,
    now: DateTime<Utc>,
) -> AppResult<AppliedDelta> {
    validate(change)?;
    let savepoint = transaction.savepoint()?;
    let before = get_balance(&savepoint)?;
    let balance = before
        .checked_add(change.delta)
        .ok_or_else(|| AppError::new("COIN_OVERFLOW", "喵币数量超出可保存范围，本次变动未执行"))?;
    if balance < 0 {
        return Err(AppError::new(
            "INSUFFICIENT_COINS",
            "喵币余额不足，本次扣除未执行",
        ));
    }
    let created_at = stamp(now);
    if savepoint.execute(
        "UPDATE user_state SET coins=?1,updated_at=?2 WHERE id=1",
        params![balance, created_at],
    )? != 1
    {
        return Err(AppError::storage());
    }
    let plugin = change.plugin.as_ref();
    savepoint.execute(
        "INSERT INTO currency_ledger(delta,reason,ref_id,game_run_id,plugin_id,plugin_version,plugin_display_reason,wallet_request_id,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![change.delta, change.reason.as_str(), change.ref_id, plugin.map(|p| p.game_run_id),
            plugin.map(|p| p.plugin_id.as_str()), plugin.map(|p| p.plugin_version.as_str()),
            plugin.map(|p| p.display_reason.as_str()), plugin.map(|p| p.request_id.as_str()), created_at],
    )?;
    let ledger_id = savepoint.last_insert_rowid();
    savepoint.commit()?;
    Ok(AppliedDelta { ledger_id, balance })
}

/// 按电脑当前时区统计某原因在本地今日的流水；测试可注入时区和时刻。
/// 先用 created_at 索引查覆盖所有合法时区偏移的 UTC 候选，再逐条投影到本地日期。
/// 不按固定 24 小时计算本地一天，也不调用 SQLite 的系统 localtime。
pub fn count_today_reason<T: TimeZone>(
    connection: &Connection,
    reason: CurrencyReason,
    now: DateTime<Utc>,
    zone: &T,
) -> AppResult<i64> {
    let date = now.with_timezone(zone).date_naive();
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(AppError::storage)?
        .and_utc();
    let next_midnight = date
        .checked_add_days(Days::new(1))
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .ok_or_else(AppError::storage)?
        .and_utc();
    let start = midnight
        .checked_sub_signed(Duration::hours(24))
        .ok_or_else(AppError::storage)?;
    let end = next_midnight
        .checked_add_signed(Duration::hours(24))
        .ok_or_else(AppError::storage)?;
    let mut statement = connection.prepare(
        "SELECT created_at FROM currency_ledger WHERE reason=?1 AND created_at>=?2 AND created_at<?3")?;
    let rows = statement.query_map(params![reason.as_str(), stamp(start), stamp(end)], |row| {
        row.get::<_, String>(0)
    })?;
    let mut count = 0;
    for row in rows {
        let at = DateTime::parse_from_rfc3339(&row?).map_err(|_| AppError::storage())?;
        if at.with_timezone(zone).date_naive() == date {
            count += 1;
        }
    }
    Ok(count)
}

/// 复用会话归属读取；当前选择与版本由命令层在同一 Runtime 锁内校验。
pub fn get_session_intimacy(
    connection: &Connection,
    persona_id: i64,
    session_id: i64,
    context_revision: u64,
) -> AppResult<SessionIntimacy> {
    let session = crate::sessions::get(connection, persona_id, session_id)?;
    if !(0..=crate::pet::MAX_INTIMACY).contains(&session.intimacy) {
        return Err(AppError::storage());
    }
    Ok(SessionIntimacy {
        persona_id: session.persona_id,
        session_id: session.id,
        context_revision,
        intimacy: session.intimacy,
        maximum: crate::pet::MAX_INTIMACY,
    })
}

#[cfg(test)]
#[path = "currency/tests.rs"]
mod tests;
