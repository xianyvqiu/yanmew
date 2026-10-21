//! 全局账单只读查询：流式扫描候选流水，仅保留当前页和统计桶，不修改钱包。
use crate::error::{AppError, AppResult};
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Query {
    pub start: Option<String>,
    pub end: Option<String>,
    pub direction: String,
    pub reason: String,
    pub plugin_id: String,
    pub search: String,
    pub ascending: bool,
    pub page: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub delta: String,
    pub reason: String,
    pub plugin_id: Option<String>,
    pub plugin_name: Option<String>,
    pub plugin_version: Option<String>,
    pub description: Option<String>,
    pub ref_id: Option<String>,
    pub created_at: String,
}
#[derive(Default)]
struct Totals {
    income: i128,
    expense: i128,
    count: u64,
}
impl Totals {
    fn add(&mut self, delta: i64) {
        if delta > 0 {
            self.income += i128::from(delta);
        }
        if delta < 0 {
            self.expense -= i128::from(delta);
        }
        self.count += 1;
    }
    fn view(&self) -> Summary {
        Summary {
            income: self.income.to_string(),
            expense: self.expense.to_string(),
            net: (self.income - self.expense).to_string(),
            count: self.count,
        }
    }
}
#[derive(Serialize)]
pub struct Summary {
    pub income: String,
    pub expense: String,
    pub net: String,
    pub count: u64,
}
#[derive(Serialize)]
pub struct Group {
    pub key: String,
    pub name: String,
    pub summary: Summary,
}
#[derive(Serialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub entries: Vec<Entry>,
    pub summary: Summary,
    pub balance: String,
    pub total_pages: u64,
    pub page: u64,
    pub categories: Vec<Group>,
    pub games: Vec<Group>,
    pub plugins: Vec<Plugin>,
    pub timeline: Vec<Group>,
    pub bucket: String,
    pub range_start: String,
    pub range_end: String,
}
pub fn label(reason: &str) -> &str {
    match reason {
        "daily_sign_in" => "签到",
        "sign_in_festival" => "节日奖励",
        "accessory_craft" => "饰品制作",
        "accessory_sell" => "饰品卖出",
        "food_purchase" => "食物购买",
        "plugin_game" => "小游戏",
        _ => "其他",
    }
}
fn date(value: &Option<String>) -> AppResult<Option<NaiveDate>> {
    value
        .as_ref()
        .map(|s| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .filter(|d| {
                    d.format("%Y-%m-%d").to_string() == *s && (1..=9999).contains(&d.year())
                })
                .ok_or_else(|| AppError::validation("date", "请选择有效日期"))
        })
        .transpose()
}
fn bucket_start(day: NaiveDate, bucket: &str) -> NaiveDate {
    match bucket {
        "year" => day.with_month(1).unwrap().with_day(1).unwrap(),
        "month" => day.with_day(1).unwrap(),
        "week" => day
            .checked_sub_days(Days::new(day.weekday().num_days_from_monday() as u64))
            .unwrap_or(day),
        _ => day,
    }
}
pub fn read(connection: &Connection, input: Query) -> AppResult<Report> {
    read_with_date(
        connection,
        input,
        |instant| instant.with_timezone(&Local).date_naive(),
        Local::now().date_naive(),
    )
}
fn read_with_date(
    connection: &Connection,
    input: Query,
    local_date: impl Fn(DateTime<Utc>) -> NaiveDate,
    today: NaiveDate,
) -> AppResult<Report> {
    let start = date(&input.start)?;
    let end = date(&input.end)?;
    if start.zip(end).is_some_and(|(s, e)| s > e) {
        return Err(AppError::validation("date", "开始日期不能晚于结束日期"));
    }
    if !["all", "income", "expense"].contains(&input.direction.as_str())
        || (!input.reason.is_empty() && label(&input.reason) == "其他")
        || input.page == 0
    {
        return Err(AppError::validation("filter", "账单筛选无效"));
    }
    // UTC候选放宽一天，再按当前系统时区的本地日期判断；兼容夏令时和历史时区变化。
    let lower = start
        .and_then(|d| d.pred_opt())
        .map(|d| format!("{d}T00:00:00Z"))
        .unwrap_or_default();
    let upper = end
        .and_then(|d| d.checked_add_days(Days::new(2)))
        .map(|d| format!("{d}T00:00:00Z"))
        .unwrap_or_else(|| "9999-12-31T23:59:59.999999999Z".into());
    let order = if input.ascending { "ASC" } else { "DESC" };
    let sql = format!("SELECT l.id,l.delta,l.reason,l.plugin_id,r.plugin_name,l.plugin_version,l.plugin_display_reason,l.ref_id,l.created_at FROM currency_ledger l LEFT JOIN game_runs r ON r.id=l.game_run_id WHERE l.created_at>=?1 AND l.created_at<?2 AND (?3='' OR l.reason=?3) AND (?4='' OR l.plugin_id=?4) AND (?5='all' OR (?5='income' AND l.delta>0) OR (?5='expense' AND l.delta<0)) ORDER BY l.created_at {order},l.id {order}");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params![lower, upper, input.reason, input.plugin_id, input.direction],
        |r| {
            Ok(Entry {
                id: r.get::<_, i64>(0)?.to_string(),
                delta: r.get::<_, i64>(1)?.to_string(),
                reason: r.get(2)?,
                plugin_id: r.get(3)?,
                plugin_name: r.get(4)?,
                plugin_version: r.get(5)?,
                description: r.get(6)?,
                ref_id: r.get::<_, Option<i64>>(7)?.map(|v| v.to_string()),
                created_at: r.get(8)?,
            })
        },
    )?;
    let search = input.search.trim().to_lowercase();
    let offset = (input.page - 1).saturating_mul(12);
    let mut entries = Vec::new();
    let mut totals = Totals::default();
    let mut categories: BTreeMap<String, Totals> = BTreeMap::new();
    let mut games: BTreeMap<String, (String, Totals)> = BTreeMap::new();
    let mut days: BTreeMap<NaiveDate, Totals> = BTreeMap::new();
    for row in rows {
        let entry = row?;
        let instant = DateTime::parse_from_rfc3339(&entry.created_at)
            .map_err(|_| AppError::storage())?
            .to_utc();
        let day = local_date(instant);
        if start.is_some_and(|s| day < s) || end.is_some_and(|e| day > e) {
            continue;
        }
        let text = format!(
            "{} {} {} {} {}",
            label(&entry.reason),
            entry.plugin_name.as_deref().unwrap_or(""),
            entry.plugin_id.as_deref().unwrap_or(""),
            entry.description.as_deref().unwrap_or(""),
            entry.delta
        )
        .to_lowercase();
        if !search.is_empty() && !text.contains(&search) {
            continue;
        }
        let delta = entry
            .delta
            .parse::<i64>()
            .map_err(|_| AppError::storage())?;
        categories
            .entry(entry.reason.clone())
            .or_default()
            .add(delta);
        if let Some(id) = &entry.plugin_id {
            games
                .entry(id.clone())
                .or_insert_with(|| {
                    (
                        entry.plugin_name.clone().unwrap_or_else(|| id.clone()),
                        Totals::default(),
                    )
                })
                .1
                .add(delta);
        }
        if totals.count >= offset && entries.len() < 12 {
            entries.push(entry);
        }
        totals.add(delta);
        days.entry(day).or_default().add(delta);
    }
    let range_start = start
        .or_else(|| days.keys().next().copied())
        .or(end)
        .unwrap_or(today);
    let range_end = end
        .or_else(|| days.keys().next_back().copied())
        .unwrap_or(range_start)
        .max(range_start);
    let span = (range_end - range_start).num_days() + 1;
    let bucket = if span <= 31 {
        "day"
    } else if span <= 180 {
        "week"
    } else if span <= 730 {
        "month"
    } else {
        "year"
    };
    let mut bins: BTreeMap<NaiveDate, Totals> = BTreeMap::new();
    let mut cursor = bucket_start(range_start, bucket);
    while cursor <= range_end {
        bins.insert(cursor, Totals::default());
        let next = match bucket {
            "year" => cursor.with_year(cursor.year() + 1),
            "month" => cursor.checked_add_months(chrono::Months::new(1)),
            "week" => cursor.checked_add_days(Days::new(7)),
            _ => cursor.succ_opt(),
        };
        match next {
            Some(next) if next > cursor => cursor = next,
            _ => break,
        }
    }
    for (day, value) in days {
        let target = bins.entry(bucket_start(day, bucket)).or_default();
        target.income += value.income;
        target.expense += value.expense;
        target.count += value.count;
    }
    let mut plugin_query = connection.prepare("SELECT l.plugin_id,COALESCE(MAX(r.plugin_name),l.plugin_id) FROM currency_ledger l LEFT JOIN game_runs r ON r.id=l.game_run_id WHERE l.plugin_id IS NOT NULL GROUP BY l.plugin_id ORDER BY 2")?;
    let plugins = plugin_query
        .query_map([], |r| {
            Ok(Plugin {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Report {
        entries,
        total_pages: totals.count.div_ceil(12).max(1),
        page: input.page,
        summary: totals.view(),
        balance: super::get_wallet(connection)?.coins,
        plugins,
        categories: categories
            .into_iter()
            .map(|(key, v)| Group {
                name: label(&key).into(),
                key,
                summary: v.view(),
            })
            .collect(),
        games: games
            .into_iter()
            .map(|(key, (name, v))| Group {
                key,
                name,
                summary: v.view(),
            })
            .collect(),
        timeline: bins
            .into_iter()
            .map(|(day, v)| Group {
                key: day.to_string(),
                name: day.to_string(),
                summary: v.view(),
            })
            .collect(),
        bucket: bucket.into(),
        range_start: range_start.to_string(),
        range_end: range_end.to_string(),
    })
}

#[cfg(test)]
mod tests;
