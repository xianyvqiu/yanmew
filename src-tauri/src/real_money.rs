//! 独立人民币账本；所有金额以分保存，余额由账单派生，不接触喵币。
use crate::error::{parse_id, AppError, AppResult};
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub mod commands;
pub mod cycles;
pub mod records;
pub mod tools;

pub const TABLES: &[(&str,&str)] = &[
 ("real_assets","id,name,note,is_default,archived,revision,created_at,updated_at"),
 ("real_categories","id,name,icon,direction,archived,revision,created_at,updated_at"),
 ("real_schedules","id,title,kind,category_id,asset_id,description,amount_cents,anchor_local,unit,interval,generation,next_sequence,status,review_required,recorder,revision,created_at,updated_at"),
 ("real_entries","id,title,kind,category_id,asset_id,description,amount_cents,occurred_local,source,recorder,schedule_id,revision,created_at,updated_at"),
 ("real_occurrences","id,schedule_id,sequence,generation,scheduled_local,snapshot_json,status,entry_id,created_at,resolved_at"),
 ("real_budgets","period,amount_cents,updated_at"),
 ("real_operations","request_id,fingerprint,result_json,created_at"),
];
pub fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Millis, true)
}
pub fn local_stamp(now: DateTime<Utc>) -> String {
    now.with_timezone(&Local)
        .format("%Y-%m-%dT%H:%M")
        .to_string()
}
pub fn invalid(text: &str) -> AppError {
    AppError::validation("realMoney", text)
}
pub fn stale() -> AppError {
    AppError::new("REAL_MONEY_CHANGED", "这条资料已改变，请刷新后再操作")
}
pub fn date(text: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .filter(|v| v.to_string() == text && (1..=9999).contains(&v.year()))
        .ok_or_else(|| invalid("日期格式应为YYYY-MM-DD"))
}
pub fn datetime(text: &str) -> AppResult<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M")
        .ok()
        .filter(|v| {
            v.format("%Y-%m-%dT%H:%M").to_string() == text && (1..=9999).contains(&v.year())
        })
        .ok_or_else(|| invalid("记账时间应为本地日期和时间，精确到分钟"))
}
/// 不经过f64；拒绝指数和超过两位的小数。
pub fn cents(text: &str, signed: bool) -> AppResult<i64> {
    let text = text.trim();
    let negative = text.starts_with('-');
    if negative && !signed {
        return Err(invalid("收入或支出的金额请填正数"));
    }
    let number = if negative { &text[1..] } else { text };
    let mut split = number.split('.');
    let whole = split.next().unwrap_or("");
    let fraction = split.next().unwrap_or("");
    if whole.is_empty()
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 2
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || split.next().is_some()
    {
        return Err(invalid("金额请填写人民币数字，最多两位小数"));
    }
    let whole = whole
        .parse::<i128>()
        .map_err(|_| invalid("金额超出可保存范围"))?;
    let minor = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i128>().unwrap() * if fraction.len() == 1 { 10 } else { 1 }
    };
    let value = whole
        .checked_mul(100)
        .and_then(|v| v.checked_add(minor))
        .and_then(|v| if negative { v.checked_neg() } else { Some(v) })
        .ok_or_else(|| invalid("金额超出可保存范围"))?;
    i64::try_from(value).map_err(|_| invalid("金额超出可保存范围"))
}
pub fn seed(c: &Connection) -> AppResult<()> {
    if c.query_row("SELECT count(*) FROM real_assets", [], |r| {
        r.get::<_, i64>(0)
    })? != 0
    {
        return Ok(());
    }
    let now = stamp(Utc::now());
    for (name, default) in [("默认资产", 1), ("储蓄资产", 0), ("旅游资产", 0)] {
        c.execute(
            "INSERT INTO real_assets(name,is_default,created_at,updated_at) VALUES(?1,?2,?3,?3)",
            params![name, default, now],
        )?;
    }
    for (name, icon, direction) in [
        ("食物", "🍜", "expense"),
        ("宠物", "🐾", "expense"),
        ("交通", "🚇", "expense"),
        ("购物", "🛍️", "expense"),
        ("社交", "🤝", "expense"),
        ("学习", "📚", "expense"),
        ("娱乐", "🎮", "expense"),
        ("日常（水电网费）", "💡", "expense"),
        ("房租", "🏠", "expense"),
        ("生活费", "🌱", "both"),
        ("医药", "💊", "expense"),
        ("订阅会员服务", "🎟️", "expense"),
        ("旅游", "🧳", "expense"),
        ("工资", "💼", "income"),
        ("奖金", "🏅", "income"),
        ("保险", "☂️", "both"),
        ("外快", "🪙", "income"),
        ("投资", "📈", "both"),
        ("其他", "🧾", "both"),
    ] {
        c.execute("INSERT INTO real_categories(name,icon,direction,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",params![name,icon,direction,now])?;
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntryInput {
    pub title: String,
    pub kind: String,
    pub category_id: String,
    pub asset_id: String,
    pub description: String,
    pub amount: String,
    pub occurred_local: Option<String>,
    pub recorder: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rule {
    pub unit: String,
    pub interval: i64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    SaveAsset {
        id: Option<String>,
        revision: Option<i64>,
        name: String,
        note: String,
        balance: Option<String>,
        is_default: bool,
    },
    DeleteAsset {
        id: String,
        revision: i64,
    },
    SetTotal {
        amount: String,
    },
    SaveCategory {
        id: Option<String>,
        revision: Option<i64>,
        name: String,
        icon: String,
        direction: String,
    },
    DeleteCategory {
        id: String,
        revision: i64,
    },
    SaveEntry {
        id: Option<String>,
        revision: Option<i64>,
        entry: EntryInput,
        recurrence: Option<Rule>,
    },
    DeleteEntry {
        id: String,
        revision: i64,
    },
    SaveBudgets {
        day: Option<String>,
        month: Option<String>,
        year: Option<String>,
    },
    SaveSchedule {
        id: String,
        revision: i64,
        entry: EntryInput,
        rule: Rule,
        status: String,
    },
    ResolveMissed {
        ids: Vec<String>,
        through_id: Option<String>,
        record: bool,
    },
}
pub struct Actor {
    pub source: &'static str,
    pub name: String,
}
pub fn revision(c: &Connection, table: &str, id: i64, expected: Option<i64>) -> AppResult<()> {
    let current = c
        .query_row(
            &format!("SELECT revision FROM {table} WHERE id=?1"),
            [id],
            |r| r.get::<_, i64>(0),
        )
        .optional()?;
    if current.is_none() {
        return Err(AppError::not_found());
    }
    if current != expected {
        return Err(stale());
    }
    Ok(())
}
pub fn balances(c: &Connection) -> AppResult<BTreeMap<i64, i128>> {
    let mut out = BTreeMap::new();
    let mut stmt = c.prepare("SELECT asset_id,kind,amount_cents FROM real_entries")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let kind: String = row.get(1)?;
        let amount = i128::from(row.get::<_, i64>(2)?);
        *out.entry(id).or_default() += if kind == "expense" { -amount } else { amount };
    }
    Ok(out)
}
pub fn check_entry(
    c: &Connection,
    input: &EntryInput,
    now: DateTime<Utc>,
    allow_archived: bool,
) -> AppResult<(i64, i64, i64, String)> {
    if input.title.trim().is_empty() || !["income", "expense"].contains(&input.kind.as_str()) {
        return Err(invalid("请填写标题并选择收入或支出"));
    }
    let amount = cents(&input.amount, false)?;
    if amount <= 0 {
        return Err(invalid("记账金额必须大于0"));
    }
    let category = parse_id(&input.category_id)?;
    let asset = parse_id(&input.asset_id)?;
    let available=c.query_row("SELECT EXISTS(SELECT 1 FROM real_assets a,real_categories c WHERE a.id=?1 AND c.id=?2 AND (?3 OR (a.archived=0 AND c.archived=0)) AND (?3 OR c.direction='both' OR c.direction=?4))",params![asset,category,allow_archived,input.kind],|r|r.get::<_,bool>(0))?;
    if !available {
        return Err(invalid("所选资产、分类不存在、已删除或不适用于这类收支"));
    }
    let at = input
        .occurred_local
        .clone()
        .unwrap_or_else(|| local_stamp(now));
    datetime(&at)?;
    Ok((category, asset, amount, at))
}
pub fn insert_entry(
    c: &Connection,
    input: &EntryInput,
    now: DateTime<Utc>,
    actor: &Actor,
    schedule: Option<i64>,
    allow_archived: bool,
) -> AppResult<i64> {
    let (category, asset, amount, at) = check_entry(c, input, now, allow_archived)?;
    let recorder = input
        .recorder
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&actor.name);
    c.execute("INSERT INTO real_entries(title,kind,category_id,asset_id,description,amount_cents,occurred_local,source,recorder,schedule_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",params![input.title.trim(),input.kind,category,asset,input.description,amount,at,actor.source,recorder,schedule,stamp(now)])?;
    Ok(c.last_insert_rowid())
}
fn adjust(
    c: &Connection,
    asset: i64,
    delta: i128,
    now: DateTime<Utc>,
    actor: &Actor,
    title: &str,
) -> AppResult<()> {
    if delta == 0 {
        return Ok(());
    }
    let amount = i64::try_from(delta).map_err(|_| invalid("本次校准差额超出单条金额范围"))?;
    c.execute("INSERT INTO real_entries(title,kind,asset_id,amount_cents,occurred_local,source,recorder,created_at,updated_at) VALUES(?1,'adjustment',?2,?3,?4,'adjustment',?5,?6,?6)",params![title,asset,amount,local_stamp(now),actor.name,stamp(now)])?;
    Ok(())
}
pub fn mutate(
    c: &mut Connection,
    request: &str,
    action: Action,
    now: DateTime<Utc>,
    actor: Actor,
) -> AppResult<Value> {
    if request.trim().is_empty() {
        return Err(invalid("缺少记账操作编号"));
    }
    let fingerprint =
        serde_json::to_string(&json!({"action":action,"source":actor.source,"name":actor.name}))
            .map_err(|_| AppError::storage())?;
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if let Some((old, result)) = tx
        .query_row(
            "SELECT fingerprint,result_json FROM real_operations WHERE request_id=?1",
            [request],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
    {
        if old != fingerprint {
            return Err(AppError::new(
                "REAL_MONEY_REQUEST_CONFLICT",
                "此操作编号已用于不同内容，未重复执行",
            ));
        }
        let mut result: Value = serde_json::from_str(&result).map_err(|_| AppError::storage())?;
        result["alreadyApplied"] = json!(true);
        return Ok(result);
    }
    let mut result = apply(&tx, action, now, &actor)?;
    result["alreadyApplied"] = json!(false);
    tx.execute(
        "INSERT INTO real_operations VALUES(?1,?2,?3,?4)",
        params![
            request,
            fingerprint,
            serde_json::to_string(&result).map_err(|_| AppError::storage())?,
            stamp(now)
        ],
    )?;
    tx.commit()?;
    Ok(result)
}
fn apply(c: &Connection, action: Action, now: DateTime<Utc>, actor: &Actor) -> AppResult<Value> {
    let time = stamp(now);
    match action {
        Action::SaveAsset {
            id,
            revision: rev,
            name,
            note,
            balance,
            is_default,
        } => {
            if name.trim().is_empty() {
                return Err(invalid("资产名称不能为空"));
            }
            let id = id.as_deref().map(parse_id).transpose()?;
            if let Some(id) = id {
                revision(c, "real_assets", id, rev)?;
                if !is_default
                    && c.query_row(
                        "SELECT is_default FROM real_assets WHERE id=?1",
                        [id],
                        |r| r.get::<_, bool>(0),
                    )?
                {
                    return Err(invalid("请先将另一份资产设为默认，再取消当前默认资产"));
                }
            }
            if is_default {
                c.execute("UPDATE real_assets SET is_default=0,revision=revision+1,updated_at=?1 WHERE is_default=1 AND id<>?2",params![time,id.unwrap_or(-1)])?;
            }
            let id = if let Some(id) = id {
                c.execute("UPDATE real_assets SET name=?1,note=?2,is_default=?3,archived=0,revision=revision+1,updated_at=?4 WHERE id=?5",params![name.trim(),note,is_default,time,id])?;
                id
            } else {
                c.execute("INSERT INTO real_assets(name,note,is_default,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",params![name.trim(),note,is_default,time])?;
                c.last_insert_rowid()
            };
            if let Some(balance) = balance {
                let value = cents(&balance, true)?;
                let current = balances(c)?.get(&id).copied().unwrap_or(0);
                adjust(
                    c,
                    id,
                    i128::from(value) - current,
                    now,
                    actor,
                    "资产余额校准",
                )?;
            }
            Ok(json!({"id":id.to_string()}))
        }
        Action::SetTotal { amount } => {
            let desired = cents(&amount, true)?;
            let total: i128 = balances(c)?.values().sum();
            let asset = c.query_row("SELECT id FROM real_assets WHERE is_default=1", [], |r| {
                r.get(0)
            })?;
            adjust(
                c,
                asset,
                i128::from(desired) - total,
                now,
                actor,
                "总资产校准（默认资产差额）",
            )?;
            Ok(json!({"id":asset.to_string()}))
        }
        Action::DeleteAsset { id, revision: rev } => {
            let id = parse_id(&id)?;
            revision(c, "real_assets", id, Some(rev))?;
            if c.query_row(
                "SELECT is_default FROM real_assets WHERE id=?1",
                [id],
                |r| r.get::<_, bool>(0),
            )? {
                return Err(invalid("请先将其他资产设为默认"));
            }
            if balances(c)?.get(&id).copied().unwrap_or(0) != 0 {
                return Err(invalid("这份资产还有余额，请先校准余额为0后再删除"));
            }
            if c.query_row("SELECT EXISTS(SELECT 1 FROM real_occurrences WHERE status='pending' AND json_extract(snapshot_json,'$.assetId')=?1)",[id.to_string()],|r|r.get::<_,bool>(0))?{return Err(invalid("这份资产还有待确认漏期，请先补记或跳过这些项目"));}
            if c.query_row(
                "SELECT EXISTS(SELECT 1 FROM real_schedules WHERE asset_id=?1 AND status<>'ended')",
                [id],
                |r| r.get::<_, bool>(0),
            )? {
                return Err(invalid("这份资产仍被周期任务使用，请先修改或结束相关任务"));
            }
            c.execute(
                "UPDATE real_assets SET archived=1,revision=revision+1,updated_at=?1 WHERE id=?2",
                params![time, id],
            )?;
            Ok(json!({"id":id.to_string()}))
        }
        Action::SaveCategory {
            id,
            revision: rev,
            name,
            icon,
            direction,
        } => {
            if name.trim().is_empty()
                || !["income", "expense", "both"].contains(&direction.as_str())
            {
                return Err(invalid("请填写分类名称并选择用途"));
            }
            let id = id.as_deref().map(parse_id).transpose()?;
            if c.query_row("SELECT EXISTS(SELECT 1 FROM real_categories WHERE name=?1 COLLATE NOCASE AND archived=0 AND id<>?2)",params![name.trim(),id.unwrap_or(-1)],|r|r.get::<_,bool>(0))?{return Err(invalid("已有同名分类，请直接使用或修改已有项"));}
            let id = if let Some(id) = id {
                revision(c, "real_categories", id, rev)?;
                if direction!="both"&&c.query_row("SELECT EXISTS(SELECT 1 FROM real_schedules WHERE category_id=?1 AND kind<>?2 AND status<>'ended')",params![id,direction],|r|r.get::<_,bool>(0))?{return Err(invalid("此用途与现有周期不一致，请先修改相关任务"));}
                c.execute("UPDATE real_categories SET name=?1,icon=?2,direction=?3,archived=0,revision=revision+1,updated_at=?4 WHERE id=?5",params![name.trim(),icon,direction,time,id])?;
                id
            } else {
                c.execute("INSERT INTO real_categories(name,icon,direction,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",params![name.trim(),icon,direction,time])?;
                c.last_insert_rowid()
            };
            Ok(json!({"id":id.to_string()}))
        }
        Action::DeleteCategory { id, revision: rev } => {
            let id = parse_id(&id)?;
            revision(c, "real_categories", id, Some(rev))?;
            if c.query_row("SELECT EXISTS(SELECT 1 FROM real_schedules WHERE category_id=?1 AND status<>'ended')",[id],|r|r.get::<_,bool>(0))?{return Err(invalid("此分类仍被周期使用，请先修改或结束相关任务"));}
            c.execute("UPDATE real_categories SET archived=1,revision=revision+1,updated_at=?1 WHERE id=?2",params![time,id])?;
            Ok(json!({"id":id.to_string()}))
        }
        Action::SaveEntry {
            id,
            revision: rev,
            entry,
            recurrence,
        } => {
            if let Some(id) = id {
                if recurrence.is_some() {
                    return Err(invalid(
                        "修改单条账单不会建立或改变周期，请到周期任务页面操作",
                    ));
                }
                let id = parse_id(&id)?;
                revision(c, "real_entries", id, rev)?;
                let (cat, asset, amount, at) = check_entry(c, &entry, now, true)?;
                if c.query_row(
                    "SELECT kind='adjustment' FROM real_entries WHERE id=?1",
                    [id],
                    |r| r.get::<_, bool>(0),
                )? {
                    return Err(invalid("余额校准请通过资产页面操作"));
                }
                c.execute("UPDATE real_entries SET title=?1,kind=?2,category_id=?3,asset_id=?4,description=?5,amount_cents=?6,occurred_local=?7,recorder=COALESCE(?8,recorder),revision=revision+1,updated_at=?9 WHERE id=?10",params![entry.title.trim(),entry.kind,cat,asset,entry.description,amount,at,entry.recorder,time,id])?;
                Ok(json!({"id":id.to_string()}))
            } else if let Some(rule) = recurrence {
                cycles::create(c, entry, rule, now, actor)
            } else {
                Ok(json!({"id":insert_entry(c,&entry,now,actor,None,false)?.to_string()}))
            }
        }
        Action::DeleteEntry { id, revision: rev } => {
            let id = parse_id(&id)?;
            revision(c, "real_entries", id, Some(rev))?;
            c.execute("DELETE FROM real_entries WHERE id=?1", [id])?;
            Ok(json!({"id":id.to_string()}))
        }
        Action::SaveBudgets { day, month, year } => {
            for (period, amount) in [("day", day), ("month", month), ("year", year)] {
                if let Some(amount) = amount {
                    let amount = cents(&amount, false)?;
                    c.execute("INSERT INTO real_budgets VALUES(?1,?2,?3) ON CONFLICT(period) DO UPDATE SET amount_cents=excluded.amount_cents,updated_at=excluded.updated_at",params![period,amount,time])?;
                } else {
                    c.execute("DELETE FROM real_budgets WHERE period=?1", [period])?;
                }
            }
            Ok(json!({"saved":true}))
        }
        Action::SaveSchedule {
            id,
            revision: rev,
            entry,
            rule,
            status,
        } => cycles::update(c, parse_id(&id)?, rev, entry, rule, &status, now),
        Action::ResolveMissed {
            ids,
            through_id,
            record,
        } => cycles::resolve(c, &ids, through_id.as_deref(), record, now),
    }
}
pub fn validate(c: &Connection) -> AppResult<()> {
    if c.query_row(
        "SELECT count(*) FROM real_assets WHERE is_default=1 AND archived=0",
        [],
        |r| r.get::<_, i64>(0),
    )? != 1
    {
        return Err(invalid("现实记账缺少唯一默认资产"));
    }
    let mut s=c.prepare("SELECT occurred_local FROM real_entries UNION ALL SELECT anchor_local FROM real_schedules UNION ALL SELECT scheduled_local FROM real_occurrences")?;
    for at in s.query_map([], |r| r.get::<_, String>(0))? {
        datetime(&at?)?;
    }
    let mut s = c.prepare("SELECT snapshot_json,scheduled_local FROM real_occurrences")?;
    let mut rows = s.query([])?;
    while let Some(row) = rows.next()? {
        let entry: EntryInput = serde_json::from_str(&row.get::<_, String>(0)?)
            .map_err(|_| invalid("周期快照格式无效"))?;
        if entry.occurred_local.as_deref() != Some(row.get::<_, String>(1)?.as_str()) {
            return Err(invalid("周期快照日期与期次不一致"));
        }
        check_entry(c, &entry, Utc::now(), true)?;
    }
    let mut s = c.prepare("SELECT result_json FROM real_operations")?;
    for text in s.query_map([], |r| r.get::<_, String>(0))? {
        serde_json::from_str::<Value>(&text?).map_err(|_| invalid("记账回执格式无效"))?;
    }
    Ok(())
}
#[cfg(test)]
mod tests;
