//! 会话自己的年度日期：父表不复制，新建不接受母表关联；修改使用旧快照校验。
use super::*;
use crate::error::AppError;
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub kind: String,
    pub name: String,
    pub calendar_type: String,
    pub month: u32,
    pub day: u32,
    pub is_leap_month: bool,
    pub blessing: String,
    pub note: Option<String>,
}
impl Input {
    fn validate(&self) -> AppResult<()> {
        if !["festival", "birthday", "anniversary", "custom"].contains(&self.kind.as_str())
            || self.name.trim().is_empty()
            || self.blessing.trim().is_empty()
        {
            return Err(AppError::validation(
                "festival",
                "名称和祝福词必填，类型须为festival、birthday、anniversary或custom",
            ));
        }
        super::dates::validate(
            &self.calendar_type,
            self.month,
            self.day,
            self.is_leap_month,
        )
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub id: String,
    pub session_id: String,
    pub festival_id: Option<String>,
    pub kind: String,
    pub name: String,
    pub year: i32,
    pub calendar_type: String,
    pub solar_month: Option<u32>,
    pub solar_day: Option<u32>,
    pub lunar_month: Option<u32>,
    pub lunar_day: Option<u32>,
    pub is_leap_month: bool,
    pub blessing: String,
    pub note: Option<String>,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_memorial: bool,
    pub revision_token: String,
    /// 派生查询结果，不增加数据库字段；最多两次，农历日不存在则为空。
    pub occurrences: Vec<NaiveDate>,
}
const SELECT: &str="SELECT f.id,f.session_id,f.festival_id,f.kind,f.name,f.year,f.calendar_type,f.solar_month,f.solar_day,f.lunar_month,f.lunar_day,f.is_leap_month,f.blessing,f.note,f.source,f.created_at,f.updated_at,COALESCE(p.is_memorial,0) FROM session_festivals f LEFT JOIN festival_catalog p ON p.id=f.festival_id";
fn row(r: &Row<'_>) -> rusqlite::Result<Record> {
    Ok(Record {
        id: r.get::<_, i64>(0)?.to_string(),
        session_id: r.get::<_, i64>(1)?.to_string(),
        festival_id: r.get::<_, Option<i64>>(2)?.map(|id| id.to_string()),
        kind: r.get(3)?,
        name: r.get(4)?,
        year: r.get(5)?,
        calendar_type: r.get(6)?,
        solar_month: r.get(7)?,
        solar_day: r.get(8)?,
        lunar_month: r.get(9)?,
        lunar_day: r.get(10)?,
        is_leap_month: r.get(11)?,
        blessing: r.get(12)?,
        note: r.get(13)?,
        source: r.get(14)?,
        created_at: r.get(15)?,
        updated_at: r.get(16)?,
        is_memorial: r.get(17)?,
        revision_token: String::new(),
        occurrences: vec![],
    })
}
impl Record {
    fn dates(&self, year: i32) -> Vec<NaiveDate> {
        let pair = if self.calendar_type == "solar" {
            self.solar_month.zip(self.solar_day)
        } else {
            self.lunar_month.zip(self.lunar_day)
        };
        pair.map(|(month, day)| {
            super::dates::occurrences(year, &self.calendar_type, month, day, self.is_leap_month)
        })
        .unwrap_or_default()
    }
    fn view(mut self, today: NaiveDate) -> AppResult<Self> {
        self.revision_token = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&self).map_err(|_| AppError::storage())?)
        );
        self.occurrences = self.dates(today.year());
        Ok(self)
    }
}
pub fn get(c: &Connection, session: i64, id: i64, today: NaiveDate) -> AppResult<Record> {
    c.query_row(
        &format!("{SELECT} WHERE f.session_id=?1 AND f.id=?2"),
        params![session, id],
        row,
    )
    .optional()?
    .ok_or_else(|| AppError::new("NOT_FOUND", "当前会话没有这条日期记录"))?
    .view(today)
}
pub fn list(
    c: &Connection,
    session: i64,
    before: Option<i64>,
    limit: i64,
    today: NaiveDate,
) -> AppResult<Vec<Record>> {
    if !(1..=50).contains(&limit) {
        return Err(AppError::validation(
            "limit",
            "每页读取1–50条，不限制保存总数",
        ));
    }
    let mut q = c.prepare(&format!(
        "{SELECT} WHERE f.session_id=?1 AND (?2 IS NULL OR f.id<?2) ORDER BY f.id DESC LIMIT ?3"
    ))?;
    let records = q
        .query_map(params![session, before, limit], row)?
        .collect::<Result<Vec<_>, _>>()?;
    records.into_iter().map(|r| r.view(today)).collect()
}
fn require_session(c: &Connection, session: i64) -> AppResult<()> {
    if !c.query_row(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
        [session],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(AppError::new("NOT_FOUND", "会话不存在"));
    }
    Ok(())
}
pub fn create(
    c: &mut Connection,
    session: i64,
    input: Input,
    source: &str,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> AppResult<Record> {
    input.validate()?;
    if !["ai", "user"].contains(&source) {
        return Err(AppError::validation("source", "日期来源无效"));
    }
    let dates = super::dates::occurrences(
        today.year(),
        &input.calendar_type,
        input.month,
        input.day,
        input.is_leap_month,
    );
    let (sm, sd, lm, ld) = if input.calendar_type == "solar" {
        (Some(input.month), Some(input.day), None, None)
    } else {
        (
            dates.first().map(|d| d.month()),
            dates.first().map(|d| d.day()),
            Some(input.month),
            Some(input.day),
        )
    };
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    require_session(&tx, session)?;
    let stamp = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    tx.execute("INSERT INTO session_festivals(session_id,festival_id,kind,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,blessing,note,source,created_at,updated_at) VALUES(?1,NULL,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)",params![session,input.kind,input.name.trim(),today.year(),input.calendar_type,sm,sd,lm,ld,input.is_leap_month,input.blessing.trim(),input.note,source,stamp])?;
    let saved = get(&tx, session, tx.last_insert_rowid(), today)?;
    tx.commit()?;
    Ok(saved)
}
pub fn update(
    c: &mut Connection,
    session: i64,
    id: i64,
    expected: &str,
    input: Input,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> AppResult<Record> {
    input.validate()?;
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let old = get(&tx, session, id, today)?;
    if old.revision_token != expected {
        return Err(AppError::new(
            "CONFLICT",
            "日期记录已变化，请重新读取后修改",
        ));
    }
    let dates = super::dates::occurrences(
        today.year(),
        &input.calendar_type,
        input.month,
        input.day,
        input.is_leap_month,
    );
    let (sm, sd, lm, ld) = if input.calendar_type == "solar" {
        (Some(input.month), Some(input.day), None, None)
    } else {
        (
            dates.first().map(|d| d.month()),
            dates.first().map(|d| d.day()),
            Some(input.month),
            Some(input.day),
        )
    };
    tx.execute("UPDATE session_festivals SET kind=?1,name=?2,year=?3,calendar_type=?4,solar_month=?5,solar_day=?6,lunar_month=?7,lunar_day=?8,is_leap_month=?9,blessing=?10,note=?11,updated_at=?12 WHERE id=?13 AND session_id=?14",params![input.kind,input.name.trim(),today.year(),input.calendar_type,sm,sd,lm,ld,input.is_leap_month,input.blessing.trim(),input.note,now.to_rfc3339_opts(chrono::SecondsFormat::Millis,true),id,session])?;
    let saved = get(&tx, session, id, today)?;
    tx.commit()?;
    Ok(saved)
}
pub fn delete(
    c: &mut Connection,
    session: i64,
    id: i64,
    expected: &str,
    today: NaiveDate,
) -> AppResult<()> {
    let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if get(&tx, session, id, today)?.revision_token != expected {
        return Err(AppError::new(
            "CONFLICT",
            "日期记录已变化，请重新读取后删除",
        ));
    }
    tx.execute(
        "DELETE FROM session_festivals WHERE id=?1 AND session_id=?2",
        params![id, session],
    )?;
    tx.commit()?;
    Ok(())
}
pub(super) fn refresh_in_transaction(tx: &Transaction<'_>, today: NaiveDate) -> AppResult<usize> {
    let mut q = tx.prepare(&format!("{SELECT} WHERE f.year<>?1"))?;
    let records = q
        .query_map([today.year()], row)?
        .collect::<Result<Vec<_>, _>>()?;
    drop(q);
    let mut count = 0;
    for r in records {
        count += refresh_record_in_transaction(tx, today, &r)?;
    }
    Ok(count)
}

fn refresh_record_in_transaction(tx: &Transaction<'_>, today: NaiveDate, r: &Record) -> AppResult<usize> {
    if r.calendar_type == "solar" {
        Ok(tx.execute(
            "UPDATE session_festivals SET year=?1 WHERE id=?2",
            params![today.year(), r.id],
        )?)
    } else {
        let dates = r.dates(today.year());
        Ok(tx.execute(
            "UPDATE session_festivals SET year=?1,solar_month=?2,solar_day=?3 WHERE id=?4",
            params![today.year(), dates.first().map(|d| d.month()), dates.first().map(|d| d.day()), r.id],
        )?)
    }
}

/// 导入仅重算本次新增行；即使来源year等于本年，也不沿用来源的派生日期缓存。
pub(crate) fn refresh_imported_in_transaction(tx: &Transaction<'_>, today: NaiveDate, id: i64) -> AppResult<()> {
    let record = tx.query_row(&format!("{SELECT} WHERE f.id=?1"), [id], row)?;
    refresh_record_in_transaction(tx, today, &record)?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRecord {
    pub id: String,
    pub code: String,
    pub name: String,
    pub year: i32,
    pub calendar_type: String,
    pub solar_month: Option<u32>,
    pub solar_day: Option<u32>,
    pub lunar_month: Option<u32>,
    pub lunar_day: Option<u32>,
    pub is_leap_month: bool,
    pub solar_term: Option<String>,
    pub blessing: String,
    pub is_memorial: bool,
    pub note: Option<String>,
}
pub fn catalog(c: &Connection, today: NaiveDate) -> AppResult<Vec<CatalogRecord>> {
    let mut q=c.prepare("SELECT id,code,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,solar_term,blessing,is_memorial,note FROM festival_catalog ORDER BY id")?;
    let mut records = q
        .query_map([], |r| {
            Ok(CatalogRecord {
                id: r.get::<_, i64>(0)?.to_string(),
                code: r.get(1)?,
                name: r.get(2)?,
                year: r.get(3)?,
                calendar_type: r.get(4)?,
                solar_month: r.get(5)?,
                solar_day: r.get(6)?,
                lunar_month: r.get(7)?,
                lunar_day: r.get(8)?,
                is_leap_month: r.get(9)?,
                solar_term: r.get(10)?,
                blessing: r.get(11)?,
                is_memorial: r.get(12)?,
                note: r.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    for r in &mut records {
        if r.year != today.year() {
            if r.calendar_type != "solar" {
                let (date, day) = super::mapping(
                    &r.code,
                    &r.calendar_type,
                    today.year(),
                    r.lunar_month,
                    r.lunar_day,
                    r.is_leap_month,
                    r.solar_term.as_deref(),
                );
                r.solar_month = date.map(|d| d.month());
                r.solar_day = date.map(|d| d.day());
                r.lunar_day = day;
            }
            r.year = today.year();
        }
    }
    Ok(records)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Today {
    pub id: String,
    pub scope: &'static str,
    pub name: String,
    pub blessing: String,
    pub is_memorial: bool,
}
pub fn today(c: &Connection, date: NaiveDate, session: Option<i64>) -> AppResult<Vec<Today>> {
    let mut matches = catalog(c, date)?
        .into_iter()
        .filter(|r| r.solar_month == Some(date.month()) && r.solar_day == Some(date.day()))
        .map(|r| Today {
            id: r.id,
            scope: "catalog",
            name: r.name,
            blessing: r.blessing,
            is_memorial: r.is_memorial,
        })
        .collect::<Vec<_>>();
    if let Some(session) = session {
        let mut q = c.prepare(&format!("{SELECT} WHERE f.session_id=?1 ORDER BY f.id"))?;
        for r in q.query_map([session], row)? {
            let r = r?;
            if r.dates(date.year()).contains(&date) {
                matches.push(Today {
                    id: r.id,
                    scope: "session",
                    name: r.name,
                    blessing: r.blessing,
                    is_memorial: r.is_memorial,
                });
            }
        }
    }
    Ok(matches)
}
