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

const COLUMNS: &str = "id,session_id,name,display_text,image_file,description,cost,source,travel_id,is_worn,created_at,position_x,position_y,scale,render_layer,layer_order";
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
        position_x: row.get(11)?, position_y: row.get(12)?, scale: row.get(13)?,render_layer:row.get(14)?,layer_order:row.get(15)?,
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

/// 显式保存相对桌宠的千分比坐标，不改变佩戴、币或制作资格。
pub(crate) fn placement(c:&Connection,session:i64,id:i64,x:Option<i32>,y:Option<i32>,scale:i32)->AppResult<Accessory>{get(c,session,id)?;if x.is_some()!=y.is_some()||x.is_some_and(|n|!(-1500..=1500).contains(&n))||y.is_some_and(|n|!(-1500..=1500).contains(&n))||!(200..=3000).contains(&scale){return Err(AppError::validation("position","位置须在相对范围内，大小为20%–300%"));}c.execute("UPDATE accessories SET position_x=?1,position_y=?2,scale=?3 WHERE id=?4 AND session_id=?5",params![x,y,scale,id,session])?;get(c,session,id)}

/// 图层顺序仅作用当前会话，先验证归属，同事务重排并保持其他平面的顺序。
pub(crate) fn layer(c:&mut Connection,session:i64,id:i64,plane:&str,action:&str)->AppResult<Accessory>{
    if !matches!(plane,"front"|"back")||!matches!(action,"keep"|"up"|"down"|"top"|"bottom"){return Err(AppError::validation("layer","无效图层操作"));}
    let tx=c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let old=get(&tx,session,id)?;
    if old.render_layer!=plane{let top:i64=tx.query_row("SELECT COALESCE(MAX(layer_order),0)+1 FROM accessories WHERE session_id=?1 AND render_layer=?2",params![session,plane],|r|r.get(0))?;tx.execute("UPDATE accessories SET render_layer=?1,layer_order=?2 WHERE id=?3",params![plane,top,id])?;}
    if action!="keep"{
      let mut ids={let mut q=tx.prepare("SELECT id FROM accessories WHERE session_id=?1 AND render_layer=?2 AND (is_worn=1 OR id=?3) ORDER BY layer_order,id")?;let v=q.query_map(params![session,plane,id],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;v};
      let i=ids.iter().position(|v|*v==id).ok_or_else(AppError::not_found)?;
      let target=match action{"up"=>(i+1).min(ids.len()-1),"down"=>i.saturating_sub(1),"top"=>ids.len()-1,_=>0};
      ids.remove(i);ids.insert(target,id);for (order,key) in ids.iter().enumerate(){tx.execute("UPDATE accessories SET layer_order=?1 WHERE id=?2",params![order as i64,key])?;}
    }
    let result=get(&tx,session,id)?;tx.commit()?;Ok(result)
}
