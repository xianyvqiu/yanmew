//! 月历为只读派生视图；浏览月份不改变节日年度缓存或签到状态。
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;
use crate::{error::{AppError, AppResult}, lunar};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub date: NaiveDate,
    pub lunar: String,
    pub events: Vec<super::custom::Today>,
    pub solar_term: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Month { pub year: i32, pub month: u32, pub days: Vec<Day> }

pub fn month(c: &Connection, year: i32, month: u32) -> AppResult<Month> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)
        .filter(|_| (1950..=2100).contains(&year))
        .ok_or_else(|| AppError::validation("month", "日历支持1950–2100年的有效月份"))?;
    let mut date = first;
    let mut days = Vec::new();
    let names = ["小寒","大寒","立春","雨水","惊蛰","春分","清明","谷雨","立夏","小满","芒种","夏至","小暑","大暑","立秋","处暑","白露","秋分","寒露","霜降","立冬","小雪","大雪","冬至"];
    let terms: Vec<_> = names.iter().filter_map(|name| lunar::solar_term_date(year, name).map(|d| (d, *name))).collect();
    while date.month() == month && date.year() == year {
        let label = lunar::solar_to_lunar(date).map(lunar_label).unwrap_or_default();
        days.push(Day { date, lunar: label, events: super::custom::today(c, date)?,
            solar_term: terms.iter().find(|(d, _)| *d == date).map(|(_, name)| name.to_string()) });
        match date.succ_opt() { Some(next) => date = next, None => break }
    }
    Ok(Month { year, month, days })
}
fn lunar_label(date: lunar::LunarDate) -> String {
    let numbers = ["一","二","三","四","五","六","七","八","九","十"];
    if date.day == 1 {
        let months = ["正","二","三","四","五","六","七","八","九","十","冬","腊"];
        return format!("{}{}月", if date.is_leap { "闰" } else { "" }, months[(date.month - 1) as usize]);
    }
    match date.day {
        2..=10 => format!("初{}", numbers[(date.day - 1) as usize]),
        11..=19 => format!("十{}", numbers[(date.day - 11) as usize]),
        20 => "二十".into(),
        21..=29 => format!("廿{}", numbers[(date.day - 21) as usize]),
        30 => "三十".into(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_month_keeps_current_catalog_year_and_marks_lunar_holiday() {
        let mut c = crate::db::test_database();
        super::super::synchronize(&mut c, NaiveDate::from_ymd_opt(2026,10,14).unwrap()).unwrap();
        let result = month(&c, 2024, 2).unwrap();
        assert_eq!(result.days.len(), 29);
        let spring = &result.days[9];
        assert_eq!(spring.lunar, "正月");
        assert!(spring.events.iter().any(|e| e.name == "春节"));
        assert_eq!(c.query_row("SELECT MIN(year) FROM festival_catalog", [], |r| r.get::<_,i32>(0)).unwrap(), 2026);
        assert!(month(&c, 2024, 13).is_err());
    }
}
