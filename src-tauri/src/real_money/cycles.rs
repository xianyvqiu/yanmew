use super::*;
use chrono::{Days, Months};
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub category_id: String,
    pub asset_id: String,
    pub description: String,
    pub amount_cents: String,
    pub anchor_local: String,
    pub unit: String,
    pub interval: i64,
    pub generation: i64,
    pub next_sequence: i64,
    pub status: String,
    pub review_required: bool,
    pub recorder: String,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}
const FIELDS:&str="id,title,kind,category_id,asset_id,description,amount_cents,anchor_local,unit,interval,generation,next_sequence,status,review_required,recorder,revision,created_at,updated_at";
fn plan(r: &rusqlite::Row<'_>) -> rusqlite::Result<Plan> {
    Ok(Plan {
        id: r.get::<_, i64>(0)?.to_string(),
        title: r.get(1)?,
        kind: r.get(2)?,
        category_id: r.get::<_, i64>(3)?.to_string(),
        asset_id: r.get::<_, i64>(4)?.to_string(),
        description: r.get(5)?,
        amount_cents: r.get::<_, i64>(6)?.to_string(),
        anchor_local: r.get(7)?,
        unit: r.get(8)?,
        interval: r.get(9)?,
        generation: r.get(10)?,
        next_sequence: r.get(11)?,
        status: r.get(12)?,
        review_required: r.get(13)?,
        recorder: r.get(14)?,
        revision: r.get(15)?,
        created_at: r.get(16)?,
        updated_at: r.get(17)?,
    })
}
impl Plan {
    fn rule(&self) -> Rule {
        Rule {
            unit: self.unit.clone(),
            interval: self.interval,
        }
    }
    fn entry(&self, at: NaiveDateTime) -> AppResult<EntryInput> {
        Ok(EntryInput {
            title: self.title.clone(),
            kind: self.kind.clone(),
            category_id: self.category_id.clone(),
            asset_id: self.asset_id.clone(),
            description: self.description.clone(),
            amount: records::yuan(self.amount_cents.parse().map_err(|_| AppError::storage())?),
            occurred_local: Some(at.format("%Y-%m-%dT%H:%M").to_string()),
            recorder: Some(format!("周期任务 · {}", self.recorder)),
        })
    }
}
pub fn check_rule(rule: &Rule) -> AppResult<()> {
    if rule.interval <= 0 || !["day", "week", "month", "year"].contains(&rule.unit.as_str()) {
        return Err(invalid("周期应为每N天、周、月或年，N必须为正整数"));
    }
    Ok(())
}
pub fn due(anchor: NaiveDateTime, rule: &Rule, sequence: i64) -> Option<NaiveDateTime> {
    let count = u64::try_from(rule.interval.checked_mul(sequence)?).ok()?;
    let day = match rule.unit.as_str() {
        "day" => anchor.date().checked_add_days(Days::new(count))?,
        "week" => anchor
            .date()
            .checked_add_days(Days::new(count.checked_mul(7)?))?,
        "month" | "year" => {
            let months = if rule.unit == "year" {
                count.checked_mul(12)?
            } else {
                count
            };
            let first = anchor
                .date()
                .with_day(1)?
                .checked_add_months(Months::new(u32::try_from(months).ok()?))?;
            let last = first.checked_add_months(Months::new(1))?.pred_opt()?.day();
            first.with_day(anchor.day().min(last))?
        }
        _ => return None,
    };
    if !(1..=9999).contains(&day.year()) {
        return None;
    }
    Some(day.and_time(anchor.time()))
}
fn sequence_near(anchor: NaiveDateTime, rule: &Rule, at: NaiveDateTime) -> i64 {
    let difference = match rule.unit.as_str() {
        "day" => (at.date() - anchor.date()).num_days(),
        "week" => (at.date() - anchor.date()).num_days() / 7,
        "month" => {
            i64::from(at.year() - anchor.year()) * 12 + i64::from(at.month())
                - i64::from(anchor.month())
        }
        "year" => i64::from(at.year() - anchor.year()),
        _ => 0,
    };
    (difference / rule.interval).saturating_sub(1).max(0)
}
pub fn create(
    c: &Connection,
    mut entry: EntryInput,
    rule: Rule,
    now: DateTime<Utc>,
    actor: &Actor,
) -> AppResult<Value> {
    check_rule(&rule)?;
    let (cat, asset, amount, at) = check_entry(c, &entry, now, false)?;
    let anchor = datetime(&at)?;
    let recorder = entry
        .recorder
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| actor.name.clone());
    entry.recorder = Some(recorder.clone());
    entry.occurred_local = Some(at.clone());
    let review_required = anchor <= now.with_timezone(&Local).naive_local();
    c.execute("INSERT INTO real_schedules(title,kind,category_id,asset_id,description,amount_cents,anchor_local,unit,interval,review_required,recorder,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?12,?10,?11,?11)",params![entry.title.trim(),entry.kind,cat,asset,entry.description,amount,at,rule.unit,rule.interval,recorder,stamp(now),review_required])?;
    let id = c.last_insert_rowid();
    let mut entry_id = None;
    if anchor <= now.with_timezone(&Local).naive_local() {
        let eid = insert_entry(c, &entry, now, actor, Some(id), false)?;
        c.execute("INSERT INTO real_occurrences(schedule_id,sequence,generation,scheduled_local,snapshot_json,status,entry_id,created_at,resolved_at) VALUES(?1,0,1,?2,?3,'recorded',?4,?5,?5)",params![id,at,serde_json::to_string(&entry).map_err(|_|AppError::storage())?,eid,stamp(now)])?;
        c.execute(
            "UPDATE real_schedules SET next_sequence=1 WHERE id=?1",
            [id],
        )?;
        entry_id = Some(eid.to_string());
    }
    Ok(json!({"id":entry_id,"scheduleId":id.to_string(),"scheduled":true}))
}
pub fn update(
    c: &Connection,
    id: i64,
    rev: i64,
    entry: EntryInput,
    rule: Rule,
    status: &str,
    now: DateTime<Utc>,
) -> AppResult<Value> {
    revision(c, "real_schedules", id, Some(rev))?;
    check_rule(&rule)?;
    if !["active", "paused", "ended"].contains(&status) {
        return Err(invalid("周期状态无效"));
    }
    let previous = c.query_row(
        &format!("SELECT {FIELDS} FROM real_schedules WHERE id=?1"),
        [id],
        plan,
    )?;
    if previous.status == "ended" && status != "ended" {
        return Err(invalid("已结束的任务不能重新启用，请新建周期"));
    }
    let (cat, asset, amount, at) = check_entry(c, &entry, now, status == "ended")?;
    let changed = at != previous.anchor_local
        || rule.unit != previous.unit
        || rule.interval != previous.interval;
    let mut generation = previous.generation;
    let mut sequence = previous.next_sequence;
    if changed {
        generation = generation.checked_add(1).ok_or_else(AppError::storage)?;
        let anchor = datetime(&at)?;
        let local = now.with_timezone(&Local).naive_local();
        sequence = sequence_near(anchor, &rule, local);
        while due(anchor, &rule, sequence).is_some_and(|at| at <= local) {
            sequence = sequence.checked_add(1).ok_or_else(AppError::storage)?;
        }
    }
    c.execute("UPDATE real_schedules SET title=?1,kind=?2,category_id=?3,asset_id=?4,description=?5,amount_cents=?6,anchor_local=?7,unit=?8,interval=?9,generation=?10,next_sequence=?11,status=?12,review_required=CASE WHEN status='paused' AND ?12='active' THEN 1 ELSE review_required END,revision=revision+1,updated_at=?13 WHERE id=?14",params![entry.title.trim(),entry.kind,cat,asset,entry.description,amount,at,rule.unit,rule.interval,generation,sequence,status,stamp(now),id])?;
    Ok(json!({"id":id.to_string()}))
}
#[derive(Default)]
pub struct Worker {
    started: bool,
    last_scan: Option<i64>,
}
impl Worker {
    pub fn tick(&mut self, c: &mut Connection, now: DateTime<Utc>) -> AppResult<bool> {
        if self.last_scan == Some(now.timestamp() / 15) {
            return Ok(false);
        }
        let tx = c.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !self.started {
            tx.execute(
                "UPDATE real_schedules SET review_required=1 WHERE status='active'",
                [],
            )?;
        }
        let plans = tx
            .prepare(&format!(
                "SELECT {FIELDS} FROM real_schedules WHERE status='active'"
            ))?
            .query_map([], plan)?
            .collect::<Result<Vec<_>, _>>()?;
        let local = now.with_timezone(&Local).naive_local();
        let mut changed = false;
        for p in plans {
            let id = parse_id(&p.id)?;
            let anchor = datetime(&p.anchor_local)?;
            let rule = p.rule();
            let mut sequence = p.next_sequence;
            for _ in 0..128 {
                let Some(at) = due(anchor, &rule, sequence) else {
                    tx.execute("UPDATE real_schedules SET status='ended',revision=revision+1,updated_at=?1 WHERE id=?2",params![stamp(now),id])?;
                    changed = true;
                    break;
                };
                if at > local {
                    if p.review_required {
                        tx.execute(
                            "UPDATE real_schedules SET review_required=0 WHERE id=?1",
                            [id],
                        )?;
                    }
                    break;
                }
                let entry = p.entry(at)?;
                let pending = p.review_required || !self.started;
                let inserted=tx.execute("INSERT OR IGNORE INTO real_occurrences(schedule_id,sequence,generation,scheduled_local,snapshot_json,status,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,sequence,p.generation,at.format("%Y-%m-%dT%H:%M").to_string(),serde_json::to_string(&entry).map_err(|_|AppError::storage())?,if pending{"pending"}else{"recorded"},stamp(now)])?;
                if inserted > 0 {
                    let occurrence = tx.last_insert_rowid();
                    if !pending {
                        let eid = insert_entry(
                            &tx,
                            &entry,
                            now,
                            &Actor {
                                source: "cycle",
                                name: p.recorder.clone(),
                            },
                            Some(id),
                            true,
                        )?;
                        tx.execute(
                            "UPDATE real_occurrences SET entry_id=?1,resolved_at=?2 WHERE id=?3",
                            params![eid, stamp(now), occurrence],
                        )?;
                    }
                    changed = true;
                }
                sequence = sequence.checked_add(1).ok_or_else(AppError::storage)?;
                // 剩余漏期继续分批发现，但绝不在下一批自动记入。
                tx.execute("UPDATE real_schedules SET next_sequence=?1,review_required=?2,updated_at=?3 WHERE id=?4",params![sequence,pending,stamp(now),id])?;
            }
        }
        tx.commit()?;
        self.started = true;
        self.last_scan = Some(now.timestamp() / 15);
        Ok(changed)
    }
}
pub fn resolve(
    c: &Connection,
    ids: &[String],
    through: Option<&str>,
    record: bool,
    now: DateTime<Utc>,
) -> AppResult<Value> {
    let mut chosen = std::collections::BTreeSet::new();
    if let Some(through) = through {
        let through = parse_id(through)?;
        let mut s = c.prepare(
            "SELECT id FROM real_occurrences WHERE status='pending' AND id<=?1 ORDER BY id",
        )?;
        for id in s.query_map([through], |r| r.get::<_, i64>(0))? {
            chosen.insert(id?);
        }
    } else {
        for id in ids {
            chosen.insert(parse_id(id)?);
        }
    }
    let mut count = 0;
    for id in chosen {
        let value = c
            .query_row(
                "SELECT schedule_id,snapshot_json,status FROM real_occurrences WHERE id=?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(AppError::not_found)?;
        if value.2 != "pending" {
            continue;
        }
        let entry: EntryInput = serde_json::from_str(&value.1).map_err(|_| AppError::storage())?;
        let eid = if record {
            Some(insert_entry(
                c,
                &entry,
                now,
                &Actor {
                    source: "cycle",
                    name: "周期任务".into(),
                },
                Some(value.0),
                true,
            )?)
        } else {
            None
        };
        c.execute("UPDATE real_occurrences SET status=?1,entry_id=?2,resolved_at=?3 WHERE id=?4 AND status='pending'",params![if record{"recorded"}else{"skipped"},eid,stamp(now),id])?;
        count += 1;
    }
    Ok(json!({"changed":count,"recorded":record}))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListQuery {
    pub kind: String,
    pub search: String,
    pub status: String,
    pub ascending: bool,
    pub page: u64,
}
pub fn list(c: &Connection, q: ListQuery) -> AppResult<Value> {
    if q.page == 0 {
        return Err(invalid("页码无效"));
    }
    let order = if q.ascending { "ASC" } else { "DESC" };
    let offset = i64::try_from((q.page - 1).saturating_mul(12)).map_err(|_| invalid("页码无效"))?;
    let (count, items) = if q.kind == "schedules" {
        if !["", "active", "paused", "ended"].contains(&q.status.as_str()) {
            return Err(invalid("状态无效"));
        }
        let filter="WHERE instr(lower(title||' '||description||' '||recorder),lower(?1))>0 AND (?2='' OR status=?2)";
        let count = c.query_row(
            &format!("SELECT count(*) FROM real_schedules {filter}"),
            params![q.search, q.status],
            |r| r.get::<_, u64>(0),
        )?;
        let mut s=c.prepare(&format!("SELECT {FIELDS} FROM real_schedules {filter} ORDER BY updated_at {order},id {order} LIMIT 12 OFFSET ?3"))?;
        let plans = s
            .query_map(params![q.search, q.status, offset], plan)?
            .collect::<Result<Vec<_>, _>>()?;
        let mut items = Vec::new();
        for p in plans {
            let next = due(datetime(&p.anchor_local)?, &p.rule(), p.next_sequence)
                .map(|d| d.format("%Y-%m-%dT%H:%M").to_string());
            let mut v = serde_json::to_value(&p).map_err(|_| AppError::storage())?;
            v["nextLocal"] = json!(next);
            items.push(v);
        }
        (count, items)
    } else if q.kind == "pending" {
        let filter = "WHERE status='pending' AND instr(lower(snapshot_json),lower(?1))>0";
        let count = c.query_row(
            &format!("SELECT count(*) FROM real_occurrences {filter}"),
            [&q.search],
            |r| r.get::<_, u64>(0),
        )?;
        let mut s=c.prepare(&format!("SELECT id,schedule_id,scheduled_local,snapshot_json FROM real_occurrences {filter} ORDER BY scheduled_local {order},id {order} LIMIT 12 OFFSET ?2"))?;
        let mut rows = s.query(params![q.search, offset])?;
        let mut items = Vec::new();
        while let Some(r) = rows.next()? {
            let entry: Value =
                serde_json::from_str(&r.get::<_, String>(3)?).map_err(|_| AppError::storage())?;
            items.push(json!({"id":r.get::<_,i64>(0)?.to_string(),"scheduleId":r.get::<_,i64>(1)?.to_string(),"scheduledLocal":r.get::<_,String>(2)?,"entry":entry}));
        }
        (count, items)
    } else {
        return Err(invalid("任务类型无效"));
    };
    Ok(json!({"items":items,"total":count,"pages":count.div_ceil(12).max(1),"page":q.page}))
}
pub fn calendar(c: &Connection, start: NaiveDate, end: NaiveDate) -> AppResult<Value> {
    if start > end {
        return Err(invalid("日期范围无效"));
    }
    let mut days: BTreeMap<String, Value> = BTreeMap::new();
    let mut day = start;
    while day <= end {
        let sum = records::sum(c, day, day)?;
        days.insert(day.to_string(),json!({"date":day.to_string(),"income":sum.income.to_string(),"expense":sum.expense.to_string(),"planned":0,"pending":0}));
        match day.succ_opt() {
            Some(next) => day = next,
            None => break,
        }
    }
    let plans = c
        .prepare(&format!(
            "SELECT {FIELDS} FROM real_schedules WHERE status='active'"
        ))?
        .query_map([], plan)?
        .collect::<Result<Vec<_>, _>>()?;
    for p in plans {
        let anchor = datetime(&p.anchor_local)?;
        let rule = p.rule();
        let mut sequence = p.next_sequence.max(sequence_near(
            anchor,
            &rule,
            start.and_hms_opt(0, 0, 0).unwrap(),
        ));
        while let Some(at) = due(anchor, &rule, sequence) {
            if at.date() > end {
                break;
            }
            if at.date() >= start && at > Local::now().naive_local() {
                if let Some(day) = days.get_mut(&at.date().to_string()) {
                    day["planned"] = json!(day["planned"].as_u64().unwrap_or(0) + 1);
                }
            }
            sequence = sequence.checked_add(1).ok_or_else(AppError::storage)?;
        }
    }
    let mut s=c.prepare("SELECT substr(scheduled_local,1,10),count(*) FROM real_occurrences WHERE status='pending' AND scheduled_local>=?1 AND scheduled_local<=?2 GROUP BY 1")?;
    let mut rows = s.query(params![format!("{start}T00:00"), format!("{end}T23:59")])?;
    while let Some(r) = rows.next()? {
        if let Some(day) = days.get_mut(&r.get::<_, String>(0)?) {
            day["pending"] = json!(r.get::<_, u64>(1)?);
        }
    }
    Ok(json!({"days":days.into_values().collect::<Vec<_>>()}))
}
