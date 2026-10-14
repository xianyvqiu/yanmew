//! 固定版本离线历表：日期使用中国农历口径，不涉及本机时区或联网。
use chrono::{Datelike, Duration, NaiveDate};

pub const RESOURCE_VERSION: &str = "yanmew-calendar-v1";
/// 便携包需保留上游版权及MIT许可；声明不参与用户备份。
pub const SOURCE_LICENSE: &str = include_str!("../../resources/calendar/LUNAR_PYTHON_LICENSE");
const MONTHS: &[u8] = include_bytes!("../../resources/calendar/months.bin");
const TERMS: &[u8] = include_bytes!("../../resources/calendar/solar-terms.bin");
const TERM_NAMES: [&str; 24] = [
    "小寒", "大寒", "立春", "雨水", "惊蛰", "春分", "清明", "谷雨", "立夏", "小满", "芒种", "夏至",
    "小暑", "大暑", "立秋", "处暑", "白露", "秋分", "寒露", "霜降", "立冬", "小雪", "大雪", "冬至",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LunarDate {
    /// 农历年，1950年初可以是1949；不使用干支年替代数字年份。
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub is_leap: bool,
}

struct Month {
    start: i32,
    year: i32,
    month: u32,
    leap: bool,
    length: u32,
}
fn month(index: usize) -> Option<Month> {
    let bytes = MONTHS.get(index.checked_mul(8)?..index.checked_mul(8)?.checked_add(8)?)?;
    Some(Month {
        start: i32::from_le_bytes(bytes.get(..4)?.try_into().ok()?),
        year: i32::from(u16::from_le_bytes(bytes.get(4..6)?.try_into().ok()?)),
        month: u32::from(bytes[6] & 0x7f),
        leap: bytes[6] & 0x80 != 0,
        length: u32::from(bytes[7]),
    })
}
fn epoch() -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(1950, 1, 1)
}
fn supported(date: NaiveDate) -> bool {
    (1950..=2100).contains(&date.year())
}

/// 返回1950-01-01至2100-12-31的农历日期；范围外返回None。
pub fn solar_to_lunar(date: NaiveDate) -> Option<LunarDate> {
    if !supported(date) {
        return None;
    }
    let offset = date.signed_duration_since(epoch()?).num_days();
    let (mut low, mut high) = (0, MONTHS.len() / 8);
    while low < high {
        let middle = low + (high - low) / 2;
        if i64::from(month(middle)?.start) <= offset {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    let entry = month(low.checked_sub(1)?)?;
    let day = u32::try_from(offset - i64::from(entry.start) + 1).ok()?;
    if day > entry.length {
        return None;
    }
    Some(LunarDate {
        year: entry.year,
        month: entry.month,
        day,
        is_leap: entry.leap,
    })
}

/// 按农历年和闰月查询，结果仍限制在支持的公历范围；无效日或闰月返回None。
pub fn lunar_to_solar(year: i32, lunar_month: u32, day: u32, is_leap: bool) -> Option<NaiveDate> {
    if !(1949..=2100).contains(&year)
        || !(1..=12).contains(&lunar_month)
        || !(1..=30).contains(&day)
    {
        return None;
    }
    let key = (year, lunar_month, is_leap);
    let (mut low, mut high) = (0, MONTHS.len() / 8);
    while low < high {
        let middle = low + (high - low) / 2;
        let entry = month(middle)?;
        if (entry.year, entry.month, entry.leap) < key {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    let entry = month(low)?;
    if (entry.year, entry.month, entry.leap) != key || day > entry.length {
        return None;
    }
    let date =
        epoch()?.checked_add_signed(Duration::days(i64::from(entry.start) + i64::from(day) - 1))?;
    supported(date).then_some(date)
}

/// 返回指定公历年中节气所在的中国历法日期，支持全部24节气（含清明/冬至）。
pub fn solar_term_date(year: i32, term_name: &str) -> Option<NaiveDate> {
    if !(1950..=2100).contains(&year) {
        return None;
    }
    let term = TERM_NAMES.iter().position(|name| *name == term_name)?;
    let index = (usize::try_from(year - 1950).ok()? * 24 + term) * 2;
    let days = u16::from_le_bytes(TERMS.get(index..index + 2)?.try_into().ok()?);
    NaiveDate::from_ymd_opt(year, 1, 1)?.checked_add_signed(Duration::days(i64::from(days)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn official_reference_dates_and_leap_month() {
        // 香港天文台2023/2024公开历表独立核对，期望不是由生成器计算。
        assert_eq!(lunar_to_solar(2024, 1, 1, false), Some(date(2024, 2, 10)));
        assert_eq!(lunar_to_solar(2024, 8, 15, false), Some(date(2024, 9, 17)));
        assert_eq!(solar_term_date(2024, "立春"), Some(date(2024, 2, 4)));
        assert_eq!(solar_term_date(2024, "清明"), Some(date(2024, 4, 4)));
        assert_eq!(solar_term_date(2024, "冬至"), Some(date(2024, 12, 21)));
        assert_eq!(lunar_to_solar(2023, 2, 1, false), Some(date(2023, 2, 20)));
        assert_eq!(lunar_to_solar(2023, 2, 1, true), Some(date(2023, 3, 22)));
        assert_eq!(
            solar_to_lunar(date(2023, 3, 22)),
            Some(LunarDate {
                year: 2023,
                month: 2,
                day: 1,
                is_leap: true
            })
        );
        assert_eq!(
            solar_to_lunar(date(2024, 1, 1)),
            Some(LunarDate {
                year: 2023,
                month: 11,
                day: 20,
                is_leap: false
            })
        );
        assert_eq!(lunar_to_solar(2033, 11, 1, false), Some(date(2033, 11, 22)));
        assert_eq!(lunar_to_solar(2033, 11, 1, true), Some(date(2033, 12, 22)));
        assert_eq!(
            solar_to_lunar(date(2100, 1, 1)),
            Some(LunarDate {
                year: 2099,
                month: 11,
                day: 21,
                is_leap: false
            })
        );
        assert_eq!(
            solar_to_lunar(date(2100, 12, 31)),
            Some(LunarDate {
                year: 2100,
                month: 12,
                day: 1,
                is_leap: false
            })
        );
    }

    #[test]
    fn every_supported_day_round_trips_and_months_are_continuous() {
        let mut current = date(1950, 1, 1);
        let last = date(2100, 12, 31);
        while current <= last {
            let lunar = solar_to_lunar(current).unwrap();
            assert_eq!(
                lunar_to_solar(lunar.year, lunar.month, lunar.day, lunar.is_leap),
                Some(current)
            );
            current = current.succ_opt().unwrap();
        }
        for index in 0..MONTHS.len() / 8 - 1 {
            let entry = month(index).unwrap();
            let next = month(index + 1).unwrap();
            assert!([29, 30].contains(&entry.length));
            assert_eq!(entry.start + entry.length as i32, next.start);
            assert!((entry.year, entry.month, entry.leap) < (next.year, next.month, next.leap));
        }
        for year in 1950..=2100 {
            let days = TERM_NAMES.map(|name| solar_term_date(year, name).unwrap());
            assert!(days.iter().all(|day| day.year() == year));
            assert!(days.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

    #[test]
    fn bounds_invalid_dates_and_resource_hashes() {
        assert!(solar_to_lunar(date(1949, 12, 31)).is_none());
        assert!(solar_to_lunar(date(2101, 1, 1)).is_none());
        for (year, month, day, leap) in [
            (2024, 0, 1, false),
            (2024, 13, 1, false),
            (2024, 1, 0, false),
            (2024, 1, 30, false),
            (2024, 2, 1, true),
            (1948, 1, 1, false),
            (2101, 1, 1, false),
        ] {
            assert!(lunar_to_solar(year, month, day, leap).is_none());
        }
        assert!(solar_term_date(1949, "冬至").is_none());
        assert!(solar_term_date(2101, "清明").is_none());
        assert!(solar_term_date(2024, "未知节气").is_none());
        use sha2::{Digest, Sha256};
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../resources/calendar/manifest.json")).unwrap();
        assert_eq!(manifest["resource_version"], RESOURCE_VERSION);
        for (name, bytes) in [("months.bin", MONTHS), ("solar-terms.bin", TERMS)] {
            assert_eq!(
                manifest["files"][name]["sha256"],
                format!("{:x}", Sha256::digest(bytes))
            );
        }
        assert!(SOURCE_LICENSE.contains("Copyright (c) 2020 6tail"));
    }
}
