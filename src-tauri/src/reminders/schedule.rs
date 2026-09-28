//! 按电脑当地日历计算，只有持久化触发时刻转换为 UTC。
use crate::error::{AppError, AppResult};
use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    Daily,
    Weekly,
    Monthly,
}
impl Repeat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }
}
fn version_one() -> u8 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Schedule {
    #[serde(default = "version_one")]
    pub version: u8,
    pub repeat: Repeat,
    pub time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekdays: Option<Vec<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<Vec<u32>>,
    /// 由后端计算的当地日历游标，切换时区时重新映射，不能作为用户输入使用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_local_at: Option<String>,
}

impl Schedule {
    pub fn validate(&self) -> AppResult<NaiveTime> {
        let invalid = || {
            AppError::validation(
                "schedule",
                "循环规则须为版本 1，时间须为 HH:MM，周几为 1–7，月日为 1–31且不重复",
            )
        };
        if self.version != 1 || self.time.len() != 5 {
            return Err(invalid());
        }
        let time = NaiveTime::parse_from_str(&self.time, "%H:%M").map_err(|_| invalid())?;
        if time.format("%H:%M").to_string() != self.time {
            return Err(invalid());
        }
        let valid_days = |days: &Option<Vec<u32>>, max| {
            days.as_ref().is_some_and(|days| {
                !days.is_empty()
                    && days.len() <= max as usize
                    && days.iter().all(|d| (1..=max).contains(d))
                    && days.iter().collect::<std::collections::BTreeSet<_>>().len() == days.len()
            })
        };
        let valid = match self.repeat {
            Repeat::Daily => self.weekdays.is_none() && self.days.is_none(),
            Repeat::Weekly => self.days.is_none() && valid_days(&self.weekdays, 7),
            Repeat::Monthly => self.weekdays.is_none() && valid_days(&self.days, 31),
        };
        if !valid {
            return Err(invalid());
        }
        Ok(time)
    }

    fn matches(&self, date: NaiveDate) -> bool {
        match self.repeat {
            Repeat::Daily => true,
            Repeat::Weekly => self
                .weekdays
                .as_ref()
                .is_some_and(|days| days.contains(&date.weekday().number_from_monday())),
            Repeat::Monthly => self
                .days
                .as_ref()
                .is_some_and(|days| days.contains(&date.day())),
        }
    }

    fn at<T: TimeZone>(&self, zone: &T, date: NaiveDate, time: NaiveTime) -> Option<DateTime<Utc>> {
        if !self.matches(date) {
            return None;
        }
        // None 跳过；重复当地时刻只采用第一次，不能在秋季回拨时发送两遍。
        zone.from_local_datetime(&date.and_time(time))
            .earliest()
            .map(|v| v.with_timezone(&Utc))
    }

    pub fn next<T: TimeZone>(&self, zone: &T, after: DateTime<Utc>) -> AppResult<DateTime<Utc>> {
        let time = self.validate()?;
        let date = after.with_timezone(zone).date_naive();
        for offset in 0..=370 {
            let date = date
                .checked_add_days(Days::new(offset))
                .ok_or_else(AppError::storage)?;
            if let Some(at) = self.at(zone, date, time).filter(|at| *at > after) {
                return Ok(at);
            }
        }
        Err(AppError::validation("schedule", "无法计算下一次提醒时间"))
    }

    /// 从当前日期向前找最近一次，离线多年也不逐个补建循环记录。
    pub fn latest<T: TimeZone>(
        &self,
        zone: &T,
        now: DateTime<Utc>,
        floor: DateTime<Utc>,
    ) -> AppResult<Option<DateTime<Utc>>> {
        let time = self.validate()?;
        let date = now.with_timezone(zone).date_naive();
        for offset in 0..=370 {
            let date = date
                .checked_sub_days(Days::new(offset))
                .ok_or_else(AppError::storage)?;
            if let Some(at) = self.at(zone, date, time).filter(|at| *at <= now) {
                return Ok((at >= floor).then_some(at));
            }
        }
        Ok(None)
    }

    pub fn set_cursor<T: TimeZone>(&mut self, zone: &T, at: DateTime<Utc>) {
        self.next_local_at = Some(
            at.with_timezone(zone)
                .naive_local()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string(),
        );
    }

    pub fn cursor<T: TimeZone>(
        &self,
        zone: &T,
        fallback: DateTime<Utc>,
    ) -> AppResult<DateTime<Utc>> {
        let Some(text) = &self.next_local_at else {
            return Ok(fallback);
        };
        let local = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S")
            .map_err(|_| AppError::storage())?;
        let time = self.validate()?;
        if local.time() != time || !self.matches(local.date()) {
            return Err(AppError::storage());
        }
        for offset in 0..=370 {
            let date = local
                .date()
                .checked_add_days(Days::new(offset))
                .ok_or_else(AppError::storage)?;
            if let Some(at) = self.at(zone, date, time) {
                return Ok(at);
            }
        }
        Err(AppError::storage())
    }
}
