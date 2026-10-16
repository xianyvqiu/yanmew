//! 全局自定义日子的年度匹配：保留农历定义，派生当年全部命中日期。
use crate::{
    error::{AppError, AppResult},
    lunar,
};
use chrono::{Datelike, NaiveDate};

pub(super) fn validate(calendar: &str, month: u32, day: u32, leap: bool) -> AppResult<()> {
    let valid = match calendar {
        // 闰年作定义校验：2月29日可记录，非闰年查询时跳过。
        "solar" => !leap && NaiveDate::from_ymd_opt(2000, month, day).is_some(),
        "lunar" => (1..=12).contains(&month) && (1..=30).contains(&day),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AppError::validation(
            "date",
            "日期无效；请填写有效公历月日或农历月日，公历不能标记闰月",
        ))
    }
}

pub(super) fn occurrences(
    year: i32,
    calendar: &str,
    month: u32,
    day: u32,
    leap: bool,
) -> Vec<NaiveDate> {
    if validate(calendar, month, day, leap).is_err() {
        return vec![];
    }
    if calendar == "solar" {
        return NaiveDate::from_ymd_opt(year, month, day)
            .into_iter()
            .collect();
    }
    if !(1950..=2100).contains(&year) {
        return vec![];
    }
    let mut dates = [year - 1, year]
        .into_iter()
        .filter_map(|lunar_year| lunar::lunar_to_solar(lunar_year, month, day, leap))
        .filter(|date| date.year() == year)
        .collect::<Vec<_>>();
    dates.sort_unstable();
    dates.dedup();
    dates
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_definitions_are_rejected_and_missing_year_dates_are_skipped() {
        assert!(validate("solar", 2, 29, false).is_ok());
        assert!(occurrences(2023, "solar", 2, 29, false).is_empty());
        assert_eq!(
            occurrences(2024, "solar", 2, 29, false),
            vec![NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()]
        );
        assert!(validate("solar", 4, 31, false).is_err());
        assert!(validate("solar", 1, 1, true).is_err());
        assert!(validate("lunar", 2, 31, false).is_err());
        assert!(occurrences(2024, "lunar", 2, 1, true).is_empty());
        assert!(occurrences(2101, "lunar", 1, 1, false).is_empty());
    }
    #[test]
    fn all_dates_in_a_solar_year_are_kept_including_previous_lunar_year() {
        let january = lunar::solar_to_lunar(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()).unwrap();
        let dates = occurrences(2024, "lunar", january.month, january.day, false);
        assert_eq!(dates.len(), 2);
        assert_eq!(dates[0], NaiveDate::from_ymd_opt(2024, 1, 1).unwrap());
        assert!(dates[1] > dates[0]);
        assert_eq!(
            occurrences(2023, "lunar", 2, 1, true),
            vec![NaiveDate::from_ymd_opt(2023, 3, 22).unwrap()]
        );
    }
}
