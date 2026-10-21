//! 食品库存与操作回执，复用唯一钱包和会话互动事务。
use crate::{
    currency::{self, CoinChange},
    error::{AppError, AppResult},
    models::CurrencyReason,
    pet::{RandomSource, MAX_INTIMACY},
    pet_store::{self, Interaction, PetWriteScope},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Food {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub unlimited: bool,
    pub price: i64,
    pub quantity: i64,
    pub revision: i64,
    pub updated_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub default_food_id: String,
    pub favorite_food_id: Option<String>,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub action_id: String,
    pub kind: String,
    pub food_id: String,
    pub quantity: i64,
    pub persona_id: Option<String>,
    pub session_id: Option<String>,
    pub favorite_bonus: bool,
    pub intimacy_delta: i64,
    pub coin_delta: i64,
    pub replayed: bool,
}
pub fn seed(c: &Connection) -> AppResult<()> {
    for (id, name) in [
        ("token", "Token小点心"),
        ("cloud_cake", "云朵奶油小蛋糕"),
        ("black_tea", "红茶"),
        ("cat_cookie", "猫猫头饼干"),
        ("rice", "白饭"),
        ("dried_fish", "小鱼干"),
    ] {
        c.execute("INSERT OR IGNORE INTO food_catalog(id,name,asset_key,is_unlimited,price,created_at,updated_at) VALUES(?1,?2,?1,?3,?4,?5,?5)",params![id,name,id=="token",if id=="token"{0}else{5},chrono::Utc::now().to_rfc3339()])?;
    }
    Ok(())
}
pub fn list(c: &Connection) -> AppResult<Vec<Food>> {
    let mut q=c.prepare("SELECT f.id,f.name,f.is_unlimited,f.price,COALESCE(i.quantity,0),f.revision,COALESCE(i.updated_at,f.created_at),f.created_at FROM food_catalog f LEFT JOIN food_inventory i ON i.food_id=f.id ORDER BY f.is_unlimited DESC,f.id")?;
    let rows = q
        .query_map([], |r| {
            Ok(Food {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(7)?,
                unlimited: r.get(2)?,
                price: r.get(3)?,
                quantity: r.get(4)?,
                revision: r.get(5)?,
                updated_at: r.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
pub fn preferences(c: &Connection, persona: i64) -> AppResult<Preferences> {
    if !c.query_row(
        "SELECT EXISTS(SELECT 1 FROM personas WHERE id=?1)",
        [persona],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(AppError::not_found());
    }
    Ok(c.query_row(
        "SELECT default_food_id,favorite_food_id FROM persona_food_settings WHERE persona_id=?1",
        [persona],
        |r| {
            Ok(Preferences {
                default_food_id: r.get(0)?,
                favorite_food_id: r.get(1)?,
            })
        },
    )
    .optional()?
    .unwrap_or(Preferences {
        default_food_id: "token".into(),
        favorite_food_id: None,
    }))
}
pub fn set_preferences(c: &Connection, persona: i64, value: Preferences) -> AppResult<()> {
    preferences(c, persona)?;
    if !c.query_row(
        "SELECT EXISTS(SELECT 1 FROM food_catalog WHERE id=?1)",
        [&value.default_food_id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(AppError::validation(
            "defaultFoodId",
            "食物不存在，请重新选择",
        ));
    }
    if let Some(favorite) = value.favorite_food_id.as_deref() {
        if !c.query_row(
            "SELECT EXISTS(SELECT 1 FROM food_catalog WHERE id=?1 AND is_unlimited=0)",
            [favorite],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::validation(
                "favoriteFoodId",
                "请选择商城中的喜好食物",
            ));
        }
    }
    c.execute("INSERT INTO persona_food_settings(persona_id,default_food_id,favorite_food_id,updated_at) VALUES(?1,?2,?3,?4) ON CONFLICT(persona_id) DO UPDATE SET default_food_id=excluded.default_food_id,favorite_food_id=excluded.favorite_food_id,updated_at=excluded.updated_at",params![persona,value.default_food_id,value.favorite_food_id,chrono::Utc::now().to_rfc3339()])?;
    Ok(())
}
pub fn receipt(c: &Connection, id: &str) -> AppResult<Option<Receipt>> {
    Ok(c.query_row("SELECT action_id,kind,food_id,quantity,persona_id,session_id,favorite_bonus,intimacy_delta,unit_price FROM food_actions WHERE action_id=?1",[id],|r|Ok(Receipt{action_id:r.get(0)?,kind:r.get(1)?,food_id:r.get(2)?,quantity:r.get(3)?,persona_id:r.get::<_,Option<i64>>(4)?.map(|v|v.to_string()),session_id:r.get::<_,Option<i64>>(5)?.map(|v|v.to_string()),favorite_bonus:r.get(6)?,intimacy_delta:r.get(7)?,coin_delta:-r.get::<_,i64>(8)?*r.get::<_,i64>(3)?,replayed:true})).optional()?)
}
fn validate_action(id: &str) -> AppResult<()> {
    if id.len() < 8
        || id.len() > 160
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:".contains(&b))
    {
        return Err(AppError::validation("actionId", "操作编号无效"));
    }
    Ok(())
}
pub fn buy(
    c: &mut Connection,
    id: &str,
    food: &str,
    quantity: i64,
    quote: i64,
) -> AppResult<Receipt> {
    validate_action(id)?;
    let mut tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if let Some(r) = receipt(&tx, id)? {
        if r.kind != "purchase" || r.food_id != food || r.quantity != quantity {
            return Err(AppError::validation("actionId", "操作编号已用于其他操作"));
        }
        return Ok(r);
    }
    if quantity <= 0 {
        return Err(AppError::validation("quantity", "购买数量必须为正整数"));
    }
    let f = list(&tx)?
        .into_iter()
        .find(|f| f.id == food)
        .ok_or_else(AppError::not_found)?;
    if f.unlimited {
        return Err(AppError::validation("foodId", "基础食物免费无限，无需购买"));
    }
    if f.revision != quote {
        return Err(AppError::new(
            "FOOD_QUOTE_CHANGED",
            "报价已变化，请重新确认",
        ));
    }
    let total = f
        .price
        .checked_mul(quantity)
        .ok_or_else(AppError::storage)?;
    let previous: i64 = tx.query_row(
        "SELECT COALESCE((SELECT quantity FROM food_inventory WHERE food_id=?1),0)",
        [food],
        |r| r.get(0),
    )?;
    let next = previous
        .checked_add(quantity)
        .ok_or_else(AppError::storage)?;
    let now = chrono::Utc::now();
    let applied = currency::apply_delta_in_tx(
        &mut tx,
        &CoinChange {
            delta: -total,
            reason: CurrencyReason::FoodPurchase,
            ref_id: None,
            plugin: None,
        },
        now,
    )?;
    tx.execute("INSERT INTO food_inventory(food_id,quantity,updated_at) VALUES(?1,?2,?3) ON CONFLICT(food_id) DO UPDATE SET quantity=excluded.quantity,updated_at=excluded.updated_at",params![food,next,now.to_rfc3339()])?;
    tx.execute("INSERT INTO food_actions(action_id,kind,food_id,quantity,food_name,unit_price,ledger_id,created_at) VALUES(?1,'purchase',?2,?3,?4,?5,?6,?7)",params![id,food,quantity,f.name,f.price,applied.ledger_id,now.to_rfc3339()])?;
    let mut r = receipt(&tx, id)?.ok_or_else(AppError::storage)?;
    r.replayed = false;
    tx.commit()?;
    Ok(r)
}
pub fn feed(
    c: &mut Connection,
    id: &str,
    food: &str,
    revision: u64,
    scope: PetWriteScope,
    random: &mut dyn RandomSource,
) -> AppResult<Receipt> {
    validate_action(id)?;
    let tx = c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if let Some(r) = receipt(&tx, id)? {
        if r.kind != "feed"
            || r.food_id != food
            || r.persona_id.as_deref() != Some(&scope.persona_id.to_string())
            || r.session_id.as_deref() != Some(&scope.session_id.to_string())
        {
            return Err(AppError::validation("actionId", "操作编号已用于其他操作"));
        }
        return Ok(r);
    }
    let f = list(&tx)?
        .into_iter()
        .find(|f| f.id == food)
        .ok_or_else(AppError::not_found)?;
    let now = chrono::Utc::now();
    pet_store::interact_in_transaction(&tx, revision, scope, Interaction::Feed, now)?;
    if !f.unlimited && tx.execute("UPDATE food_inventory SET quantity=quantity-1,updated_at=?1 WHERE food_id=?2 AND quantity>0",params![now.to_rfc3339(),food])?!=1{return Err(AppError::new("FOOD_EMPTY","这份食物已用完，请重新选择"))}
    let favorite = preferences(&tx, scope.persona_id)?
        .favorite_food_id
        .as_deref()
        == Some(food);
    let bonus = favorite && random.below(100)? < 10;
    let (before,name):(i64,String)=tx.query_row("SELECT s.intimacy,p.name FROM sessions s JOIN personas p ON p.id=s.persona_id WHERE s.id=?1",[scope.session_id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    let after = (before + 1 + i64::from(bonus)).min(MAX_INTIMACY);
    let delta = after - before;
    tx.execute(
        "UPDATE sessions SET intimacy=?1,updated_at=?2 WHERE id=?3",
        params![after, now.to_rfc3339(), scope.session_id],
    )?;
    tx.execute("INSERT INTO food_actions(action_id,kind,food_id,quantity,persona_id,session_id,food_name,target_name,unit_price,favorite_bonus,intimacy_delta,created_at) VALUES(?1,'feed',?2,1,?3,?4,?5,?6,0,?7,?8,?9)",params![id,food,scope.persona_id,scope.session_id,f.name,name,bonus,delta,now.to_rfc3339()])?;
    let mut r = receipt(&tx, id)?.ok_or_else(AppError::storage)?;
    r.replayed = false;
    tx.commit()?;
    Ok(r)
}

/// 不发起模型请求，读取当前图片库或已保存颜文字。
pub fn expression(
    c: &Connection,
    persona: i64,
    category: &str,
    random: &mut dyn RandomSource,
) -> AppResult<String> {
    if let Some(image) = crate::pet_creation::select(c, persona, category, None, random)? {
        return Ok(image);
    }
    let raw: Option<String> = c.query_row(
        "SELECT expressions FROM personas WHERE id=?1",
        [persona],
        |r| r.get(0),
    )?;
    if let Some(raw) = raw {
        let lib: serde_json::Value = serde_json::from_str(&raw).map_err(|_| AppError::storage())?;
        let fallback = category.trim_end_matches("_favorite");
        if let Some(values) = lib
            .get(category)
            .or_else(|| lib.get(fallback))
            .and_then(|v| v.as_array())
            .filter(|v| !v.is_empty())
        {
            if let Some(v) = values[random.below(values.len() as u64)? as usize].as_str() {
                return Ok(v.into());
            }
        }
    }
    Ok(crate::expressions::builtin(category).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixed(u64);
    impl RandomSource for Fixed {
        fn below(&mut self, n: u64) -> AppResult<u64> {
            assert!(self.0 < n);
            Ok(self.0)
        }
    }
    fn db() -> Connection {
        let c = crate::db::test_database();
        c.execute("UPDATE user_state SET coins=50", []).unwrap();
        c.execute(
            "INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(1,'测试',?1,?1)",
            [chrono::Utc::now().to_rfc3339()],
        )
        .unwrap();
        c.execute(
            "INSERT OR REPLACE INTO settings VALUES('current_session_id','1')",
            [],
        )
        .unwrap();
        c
    }
    fn scope(c: &Connection) -> PetWriteScope {
        PetWriteScope {
            persona_id: 1,
            session_id: 1,
            context_revision: 0,
            state_revision: pet_store::read(c, 1).unwrap().state_revision,
        }
    }
    #[test]
    fn buy_is_atomic_idempotent_and_rejects_invalid() {
        let mut c = db();
        let r = buy(&mut c, "purchase-1", "rice", 3, 0).unwrap();
        assert_eq!(r.coin_delta, -15);
        assert!(buy(&mut c, "purchase-1", "rice", 3, 0).unwrap().replayed);
        assert_eq!(currency::get_balance(&c).unwrap(), 35);
        assert_eq!(
            list(&c)
                .unwrap()
                .into_iter()
                .find(|f| f.id == "rice")
                .unwrap()
                .quantity,
            3
        );
        assert!(buy(&mut c, "purchase-1", "rice", 4, 0).is_err());
        assert!(buy(&mut c, "purchase-2", "rice", 100, 0).is_err());
        assert_eq!(currency::get_balance(&c).unwrap(), 35);
        assert!(buy(&mut c, "purchase-3", "token", 1, 0).is_err());
    }
    #[test]
    fn feed_bonus_boundaries_cap_and_no_duplicate_consumption() {
        for (draw, bonus) in [(0, true), (9, true), (10, false), (99, false)] {
            let mut c = db();
            buy(&mut c, "purchase-1", "rice", 2, 0).unwrap();
            set_preferences(
                &c,
                1,
                Preferences {
                    default_food_id: "rice".into(),
                    favorite_food_id: Some("rice".into()),
                },
            )
            .unwrap();
            let target = scope(&c);
            let r = feed(&mut c, "feed-once", "rice", 0, target, &mut Fixed(draw)).unwrap();
            assert_eq!(r.favorite_bonus, bonus);
            assert_eq!(r.intimacy_delta, if bonus { 2 } else { 1 });
            assert!(
                feed(&mut c, "feed-once", "rice", 0, target, &mut Fixed(99))
                    .unwrap()
                    .replayed
            );
            assert_eq!(
                list(&c)
                    .unwrap()
                    .into_iter()
                    .find(|f| f.id == "rice")
                    .unwrap()
                    .quantity,
                1
            );
            c.execute("UPDATE sessions SET intimacy=999", []).unwrap();
            let target = scope(&c);
            assert_eq!(
                feed(&mut c, "feed-next", "rice", 0, target, &mut Fixed(0))
                    .unwrap()
                    .intimacy_delta,
                0
            );
        }
    }
    #[test]
    fn empty_food_rolls_back_interaction_and_token_has_no_inventory() {
        let mut c = db();
        let target = scope(&c);
        let rev = target.state_revision;
        assert!(feed(&mut c, "feed-none", "rice", 0, target, &mut Fixed(0)).is_err());
        assert_eq!(scope(&c).state_revision, rev);
        let target = scope(&c);
        assert_eq!(
            feed(&mut c, "feed-free", "token", 0, target, &mut Fixed(0))
                .unwrap()
                .intimacy_delta,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM food_inventory", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
