use super::*;
use chrono::Days;
#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Query {
    pub start: Option<String>,
    pub end: Option<String>,
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub asset_id: String,
    #[serde(default)]
    pub search: String,
    #[serde(default)]
    pub ascending: bool,
    #[serde(default = "first_page")]
    pub page: u64,
}
fn first_page() -> u64 {
    1
}
#[derive(Default, Clone)]
pub struct Sum {
    pub income: i128,
    pub expense: i128,
    pub count: u64,
}
impl Sum {
    pub fn add(&mut self, kind: &str, amount: i64) {
        if kind == "income" {
            self.income += i128::from(amount);
        }
        if kind == "expense" {
            self.expense += i128::from(amount);
        }
        self.count += 1;
    }
    pub fn json(&self) -> Value {
        json!({"income":self.income.to_string(),"expense":self.expense.to_string(),"net":(self.income-self.expense).to_string(),"count":self.count})
    }
}
pub fn query(c: &Connection, q: Query, now: DateTime<Utc>) -> AppResult<Value> {
    let start = q.start.as_deref().map(date).transpose()?;
    let end = q.end.as_deref().map(date).transpose()?;
    if start.zip(end).is_some_and(|(s, e)| s > e) {
        return Err(invalid("开始日期不能晚于结束日期"));
    }
    if !["", "all", "income", "expense", "adjustment"].contains(&q.direction.as_str())
        || q.page == 0
    {
        return Err(invalid("账单筛选无效"));
    }
    let category = if q.category_id.is_empty() {
        None
    } else {
        Some(parse_id(&q.category_id)?)
    };
    let asset = if q.asset_id.is_empty() {
        None
    } else {
        Some(parse_id(&q.asset_id)?)
    };
    let lower = start.map(|d| format!("{d}T00:00")).unwrap_or_default();
    let upper = end
        .map(|d| format!("{d}T23:59"))
        .unwrap_or_else(|| "9999-12-31T23:59".into());
    let order = if q.ascending { "ASC" } else { "DESC" };
    let mut stmt=c.prepare(&format!("SELECT e.id,e.title,e.kind,e.category_id,COALESCE(cat.name,'余额校准'),e.asset_id,a.name,e.description,e.amount_cents,e.occurred_local,e.source,e.recorder,e.schedule_id,e.revision,e.created_at,e.updated_at FROM real_entries e LEFT JOIN real_categories cat ON cat.id=e.category_id JOIN real_assets a ON a.id=e.asset_id WHERE e.occurred_local>=?1 AND e.occurred_local<=?2 AND (?3='' OR ?3='all' OR e.kind=?3) AND (?4 IS NULL OR e.category_id=?4) AND (?5 IS NULL OR e.asset_id=?5) ORDER BY e.occurred_local {order},e.id {order}"))?;
    let mut rows = stmt.query(params![lower, upper, q.direction, category, asset])?;
    let word = q.search.trim().to_lowercase();
    let offset = (q.page - 1).saturating_mul(12);
    let mut entries = Vec::new();
    let mut total = Sum::default();
    let mut categories: BTreeMap<String, (String, Sum)> = BTreeMap::new();
    let mut assets: BTreeMap<String, (String, Sum)> = BTreeMap::new();
    let mut daily: BTreeMap<NaiveDate, Sum> = BTreeMap::new();
    while let Some(row) = rows.next()? {
        let title: String = row.get(1)?;
        let kind: String = row.get(2)?;
        let category_id: Option<i64> = row.get(3)?;
        let category_name: String = row.get(4)?;
        let asset_id: i64 = row.get(5)?;
        let asset_name: String = row.get(6)?;
        let description: String = row.get(7)?;
        let amount: i64 = row.get(8)?;
        let at: String = row.get(9)?;
        let source: String = row.get(10)?;
        let recorder: String = row.get(11)?;
        let searchable = format!(
            "{title} {category_name} {asset_name} {description} {recorder} {}",
            yuan(i128::from(amount))
        )
        .to_lowercase();
        if !word.is_empty() && !searchable.contains(&word) {
            continue;
        }
        let day = datetime(&at)?.date();
        if total.count >= offset && entries.len() < 12 {
            entries.push(json!({"id":row.get::<_,i64>(0)?.to_string(),"title":title,"kind":kind,"categoryId":category_id.map(|v|v.to_string()),"categoryName":category_name,"assetId":asset_id.to_string(),"assetName":asset_name,"description":description,"amountCents":amount.to_string(),"occurredLocal":at,"source":source,"recorder":recorder,"scheduleId":row.get::<_,Option<i64>>(12)?.map(|v|v.to_string()),"revision":row.get::<_,i64>(13)?,"createdAt":row.get::<_,String>(14)?,"updatedAt":row.get::<_,String>(15)?}));
        }
        if kind != "adjustment" {
            let cat = categories
                .entry(category_id.unwrap().to_string())
                .or_insert_with(|| (category_name, Sum::default()));
            cat.1.add(&kind, amount);
            assets
                .entry(asset_id.to_string())
                .or_insert_with(|| (asset_name, Sum::default()))
                .1
                .add(&kind, amount);
        }
        total.add(&kind, amount);
        daily.entry(day).or_default().add(&kind, amount);
    }
    let today = now.with_timezone(&Local).date_naive();
    let from = start
        .or_else(|| daily.keys().next().copied())
        .or(end)
        .unwrap_or(today);
    let to = end
        .or_else(|| daily.keys().next_back().copied())
        .unwrap_or(from)
        .max(from);
    let days = (to - from).num_days() + 1;
    let bucket = if days <= 31 {
        "day"
    } else if days <= 180 {
        "week"
    } else if days <= 730 {
        "month"
    } else {
        "year"
    };
    fn bin(day: NaiveDate, bucket: &str) -> NaiveDate {
        match bucket {
            "year" => NaiveDate::from_ymd_opt(day.year(), 1, 1).unwrap(),
            "month" => day.with_day(1).unwrap(),
            "week" => day
                .checked_sub_days(Days::new(day.weekday().num_days_from_monday() as u64))
                .unwrap_or(day),
            _ => day,
        }
    }
    let mut bins: BTreeMap<NaiveDate, Sum> = BTreeMap::new();
    let mut cursor = bin(from, bucket);
    while cursor <= to {
        bins.insert(cursor, Sum::default());
        let next = match bucket {
            "year" => cursor.with_year(cursor.year() + 1),
            "month" => cursor.checked_add_months(chrono::Months::new(1)),
            "week" => cursor.checked_add_days(Days::new(7)),
            _ => cursor.succ_opt(),
        };
        match next {
            Some(d) if d > cursor => cursor = d,
            _ => break,
        }
    }
    for (day, sum) in daily {
        let target = bins.entry(bin(day, bucket)).or_default();
        target.income += sum.income;
        target.expense += sum.expense;
        target.count += sum.count;
    }
    let groups = |map: BTreeMap<String, (String, Sum)>| {
        map.into_iter()
            .map(|(key, (name, sum))| json!({"key":key,"name":name,"summary":sum.json()}))
            .collect::<Vec<_>>()
    };
    let balance: i128 = balances(c)?.values().sum();
    Ok(
        json!({"entries":entries,"summary":total.json(),"balance":balance.to_string(),"page":q.page,"totalPages":total.count.div_ceil(12).max(1),"categories":groups(categories),"assets":groups(assets),"timeline":bins.into_iter().map(|(d,s)|json!({"key":d.to_string(),"name":d.to_string(),"summary":s.json()})).collect::<Vec<_>>(),"bucket":bucket,"rangeStart":from.to_string(),"rangeEnd":to.to_string()}),
    )
}
pub fn yuan(value: i128) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let v = value.abs();
    format!("{sign}{}.{:02}", v / 100, v % 100)
}
pub fn sum(c: &Connection, start: NaiveDate, end: NaiveDate) -> AppResult<Sum> {
    let mut s=c.prepare("SELECT kind,amount_cents FROM real_entries WHERE occurred_local>=?1 AND occurred_local<=?2 AND kind<>'adjustment'")?;
    let mut rows = s.query(params![format!("{start}T00:00"), format!("{end}T23:59")])?;
    let mut out = Sum::default();
    while let Some(r) = rows.next()? {
        out.add(&r.get::<_, String>(0)?, r.get(1)?);
    }
    Ok(out)
}
pub fn overview(c: &Connection, now: DateTime<Utc>) -> AppResult<Value> {
    let balance = balances(c)?;
    let mut assets = Vec::new();
    let mut stmt=c.prepare("SELECT id,name,note,is_default,archived,revision,created_at,updated_at FROM real_assets ORDER BY is_default DESC,updated_at DESC,id DESC")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        assets.push(json!({"id":id.to_string(),"name":r.get::<_,String>(1)?,"note":r.get::<_,String>(2)?,"isDefault":r.get::<_,bool>(3)?,"archived":r.get::<_,bool>(4)?,"revision":r.get::<_,i64>(5)?,"createdAt":r.get::<_,String>(6)?,"updatedAt":r.get::<_,String>(7)?,"balanceCents":balance.get(&id).copied().unwrap_or(0).to_string()}));
    }
    let mut categories = Vec::new();
    let mut stmt=c.prepare("SELECT id,name,icon,direction,archived,revision,created_at,updated_at FROM real_categories ORDER BY updated_at DESC,id DESC")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        categories.push(json!({"id":r.get::<_,i64>(0)?.to_string(),"name":r.get::<_,String>(1)?,"icon":r.get::<_,String>(2)?,"direction":r.get::<_,String>(3)?,"archived":r.get::<_,bool>(4)?,"revision":r.get::<_,i64>(5)?,"createdAt":r.get::<_,String>(6)?,"updatedAt":r.get::<_,String>(7)?}));
    }
    let today = now.with_timezone(&Local).date_naive();
    let month = today.with_day(1).unwrap();
    let year = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
    let mut budgets = Vec::new();
    for (period, start, next) in [
        (
            "day",
            today,
            today.succ_opt().ok_or_else(|| invalid("日期超出范围"))?,
        ),
        (
            "month",
            month,
            month
                .checked_add_months(chrono::Months::new(1))
                .ok_or_else(|| invalid("日期超出范围"))?,
        ),
        (
            "year",
            year,
            year.with_year(year.year() + 1)
                .ok_or_else(|| invalid("日期超出范围"))?,
        ),
    ] {
        let budget = c
            .query_row(
                "SELECT amount_cents FROM real_budgets WHERE period=?1",
                [period],
                |r| r.get::<_, i64>(0),
            )
            .optional()?;
        let spending = sum(c, start, today)?.expense;
        let elapsed = (today - start).num_days() + 1;
        let remaining_days = (next - today).num_days();
        let remaining = budget.map(|b| i128::from(b) - spending);
        budgets.push(json!({"period":period,"amountCents":budget.map(|b|b.to_string()),"spentCents":spending.to_string(),"remainingCents":remaining.map(|v|v.to_string()),"averageDailyCents":(spending/i128::from(elapsed)).to_string(),"remainingDailyCents":remaining.map(|v|(v.max(0)/i128::from(remaining_days)).to_string()),"elapsedDays":elapsed,"remainingDays":remaining_days}));
    }
    let mut pending_income = 0i128;
    let mut pending_expense = 0i128;
    let mut pending_count = 0u64;
    let mut through = 0i64;
    let mut stmt =
        c.prepare("SELECT id,snapshot_json FROM real_occurrences WHERE status='pending'")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        through = through.max(r.get(0)?);
        pending_count += 1;
        let entry: EntryInput =
            serde_json::from_str(&r.get::<_, String>(1)?).map_err(|_| AppError::storage())?;
        let amount = i128::from(cents(&entry.amount, false)?);
        if entry.kind == "income" {
            pending_income += amount;
        } else {
            pending_expense += amount;
        }
    }
    Ok(
        json!({"assets":assets,"categories":categories,"totalAssetsCents":balance.values().sum::<i128>().to_string(),"today":today.to_string(),"todaySummary":sum(c,today,today)?.json(),"monthSummary":sum(c,month,today)?.json(),"budgets":budgets,"pending":{"count":pending_count,"throughId":if through>0{Some(through.to_string())}else{None},"incomeCents":pending_income.to_string(),"expenseCents":pending_expense.to_string()},"activeSchedules":c.query_row("SELECT count(*) FROM real_schedules WHERE status='active'",[],|r|r.get::<_,i64>(0))?}),
    )
}
