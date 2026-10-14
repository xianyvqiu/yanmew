//! 会话级饰品管理。制作/旅行发放在后续阶段接入，不开放前端任意新增物品。
use crate::{
    currency::{self, CoinChange},
    error::{AppError, AppResult},
    models::{
        Accessory, AccessoryDeleteResult, AccessorySellResult, AccessorySource, CurrencyReason,
    },
    runtime::Store,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

pub(crate) mod images;
pub(crate) mod craft;
#[cfg(test)]
mod tests;

/// 校验实际活动上下文与会话归属；不能仅用客户端会话ID筛选数据库。
pub(crate) fn require_current(
    store: &Store,
    persona: i64,
    session: i64,
    revision: u64,
) -> AppResult<()> {
    let context = crate::persona::context(&store.connection, store.context_revision)?;
    if context.persona_id != persona.to_string()
        || context.session_id != Some(session.to_string())
        || context.context_revision != revision
    {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设或会话已变化，请刷新饰品后重试",
        ));
    }
    crate::sessions::get(&store.connection, persona, session)?;
    Ok(())
}

const COLUMNS: &str = "id,session_id,name,display_text,image_file,description,cost,source,travel_id,is_worn,created_at";
fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Accessory> {
    Ok(Accessory {
        id: row.get::<_, i64>(0)?.to_string(),
        session_id: row.get::<_, i64>(1)?.to_string(),
        name: row.get(2)?,
        display_text: row.get(3)?,
        image_file: row.get(4)?,
        description: row.get(5)?,
        cost: row.get::<_, i64>(6)?.to_string(),
        source: AccessorySource::parse(&row.get::<_, String>(7)?)
            .ok_or(rusqlite::Error::InvalidQuery)?,
        travel_id: row.get::<_, Option<i64>>(8)?.map(|id| id.to_string()),
        is_worn: row.get(9)?,
        created_at: row.get(10)?,
    })
}
pub(crate) fn get(connection: &Connection, session: i64, id: i64) -> AppResult<Accessory> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM accessories WHERE session_id=?1 AND id=?2"),
            params![session, id],
            row,
        )
        .optional()?
        .ok_or_else(AppError::not_found)
}
pub(crate) fn list(
    connection: &Connection,
    session: i64,
    before: Option<i64>,
    limit: i64,
) -> AppResult<Vec<Accessory>> {
    if !(1..=200).contains(&limit) || before.is_some_and(|id| id <= 0) {
        return Err(AppError::validation(
            "limit",
            "饰品每页须为1–200条，分页ID须为正整数",
        ));
    }
    let mut statement = connection.prepare(&format!(
        "SELECT {COLUMNS} FROM accessories WHERE session_id=?1 AND (?2 IS NULL OR id<?2) ORDER BY id DESC LIMIT ?3"))?;
    let items = statement
        .query_map(params![session, before, limit], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}
/// 先查归属再卸下旧件；新件更新失败时旧佩戴状态也回滚。
pub(crate) fn wear(
    connection: &mut Connection,
    session: i64,
    id: i64,
    worn: bool,
) -> AppResult<Accessory> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    get(&transaction, session, id)?;
    if worn {
        transaction.execute(
            "UPDATE accessories SET is_worn=0 WHERE session_id=?1 AND is_worn=1",
            [session],
        )?;
    }
    transaction.execute(
        "UPDATE accessories SET is_worn=?1 WHERE session_id=?2 AND id=?3",
        params![worn, session, id],
    )?;
    let result = get(&transaction, session, id)?;
    transaction.commit()?;
    Ok(result)
}
fn require_unworn(accessory: &Accessory) -> AppResult<()> {
    if accessory.is_worn {
        return Err(AppError::new(
            "ACCESSORY_WORN",
            "请先卸下饰品，再卖出或删除",
        ));
    }
    Ok(())
}
/// 删除提交后返回待清理文件；调用方确认无其他引用后清理，不影响历史流水/档位。
pub(crate) fn delete(
    connection: &mut Connection,
    session: i64,
    id: i64,
) -> AppResult<(AccessoryDeleteResult, Option<String>)> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let accessory = get(&transaction, session, id)?;
    require_unworn(&accessory)?;
    transaction.execute(
        "DELETE FROM accessories WHERE session_id=?1 AND id=?2",
        params![session, id],
    )?;
    transaction.commit()?;
    Ok((
        AccessoryDeleteResult {
            deleted_id: id.to_string(),
        },
        accessory.image_file,
    ))
}
/// 固定十币，钱包/账单/饰品删除同事务；重复提交找不到原件，不重复记账。
pub(crate) fn sell(
    connection: &mut Connection,
    session: i64,
    id: i64,
    now: DateTime<Utc>,
) -> AppResult<(AccessorySellResult, Option<String>)> {
    let mut transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let accessory = get(&transaction, session, id)?;
    require_unworn(&accessory)?;
    currency::apply_delta_in_tx(
        &mut transaction,
        &CoinChange {
            delta: 10,
            reason: CurrencyReason::AccessorySell,
            ref_id: Some(id),
            plugin: None,
        },
        now,
    )?;
    transaction.execute(
        "DELETE FROM accessories WHERE session_id=?1 AND id=?2",
        params![session, id],
    )?;
    let wallet = currency::get_wallet(&transaction)?;
    transaction.commit()?;
    Ok((
        AccessorySellResult {
            deleted_id: id.to_string(),
            wallet,
        },
        accessory.image_file,
    ))
}
