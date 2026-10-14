//! 应用全局签到：用既有账单去重，余额和签到流水在同一事务提交。
use super::{apply_delta_in_tx, count_today_reason, get_wallet, CoinChange};
use crate::{
    config,
    error::{AppError, AppResult},
    models::{CurrencyReason, DailySignInResult, SignInStatus},
    pet::RandomSource,
};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, TransactionBehavior};

fn signed_in_today<T: TimeZone>(
    connection: &Connection,
    now: DateTime<Utc>,
    zone: &T,
) -> AppResult<bool> {
    // 节日签到接入后也只能领一次；不能分别领取普通和节日两份奖励。
    Ok(
        count_today_reason(connection, CurrencyReason::DailySignIn, now, zone)? > 0
            || count_today_reason(connection, CurrencyReason::SignInFestival, now, zone)? > 0,
    )
}

pub fn get_sign_in_status<T: TimeZone>(
    connection: &Connection,
    now: DateTime<Utc>,
    zone: &T,
) -> AppResult<SignInStatus> {
    let values = config::get(connection)?;
    let number = |key: &str| -> AppResult<i64> {
        values
            .get(key)
            .and_then(|value| value.parse().ok())
            .ok_or_else(AppError::storage)
    };
    let context = crate::persona::context(connection, 0)?;
    let session = context.session_id.as_deref().map(str::parse::<i64>)
        .transpose().map_err(|_| AppError::storage())?;
    let festivals = crate::festivals::session::today(
        connection, now.with_timezone(zone).date_naive(), session,
    )?;
    Ok(SignInStatus {
        wallet: get_wallet(connection)?,
        local_date: now.with_timezone(zone).date_naive().to_string(),
        signed_in: signed_in_today(connection, now, zone)?,
        reward_min: number("sign_in_min_coins")?.to_string(),
        reward_max: number("sign_in_max_coins")?.to_string(),
        intimacy_thresholds: [
            number("intimacy_threshold_1")?,
            number("intimacy_threshold_2")?,
            number("intimacy_threshold_3")?,
            number("intimacy_threshold_4")?,
        ],
        festivals,
    })
}

/// 后端提供时刻、当前时区和随机源，不接收前端的日期或奖励数量。
pub fn daily_sign_in<T: TimeZone>(
    connection: &mut Connection,
    now: DateTime<Utc>,
    zone: &T,
    random: &mut dyn RandomSource,
) -> AppResult<DailySignInResult> {
    // IMMEDIATE 先取得写锁，另一连接不能在“检查未领”和写账单之间重复领取。
    let mut transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut status = get_sign_in_status(&transaction, now, zone)?;
    if status.signed_in {
        transaction.commit()?;
        return Ok(DailySignInResult {
            status,
            granted_coins: None,
        });
    }
    let min: i64 = status.reward_min.parse().map_err(|_| AppError::storage())?;
    let max: i64 = status.reward_max.parse().map_err(|_| AppError::storage())?;
    // 全局配置已校验为非负 i32；包含两个端点，固定奖励无需消耗随机数。
    let is_festival = !status.festivals.is_empty();
    let span = if is_festival { 1 } else { (max - min + 1) as u64 };
    let offset = if span == 1 { 0 } else { random.below(span)? };
    if offset >= span {
        return Err(AppError::new(
            "RANDOM_SOURCE_ERROR",
            "签到奖励生成失败，请重试",
        ));
    }
    let reward = if is_festival { 10 } else { min + offset as i64 };
    apply_delta_in_tx(
        &mut transaction,
        &CoinChange {
            delta: reward,
            reason: if is_festival { CurrencyReason::SignInFestival } else { CurrencyReason::DailySignIn },
            ref_id: None,
            plugin: None,
        },
        now,
    )?;
    status.wallet = get_wallet(&transaction)?;
    status.signed_in = true;
    transaction.commit()?;
    Ok(DailySignInResult {
        status,
        granted_coins: Some(reward.to_string()),
    })
}

#[cfg(test)]
#[path = "sign_in_tests.rs"]
mod tests;
