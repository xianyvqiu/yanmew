//! 系统节日母表：稳定code幂等入库，日期映射复用内置农历表。
use crate::{error::AppResult, lunar};
use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, Transaction, TransactionBehavior};
mod dates;
pub(crate) mod calendar;
pub(crate) mod session;
pub(crate) mod tools;
pub(crate) fn current_scope(store: &crate::runtime::Store, context: &crate::memory::management::Context) -> AppResult<i64> {
    let persona = crate::error::parse_id(&context.persona_id)?;
    let session = crate::error::parse_id(&context.session_id)?;
    crate::accessories::require_current(store, persona, session, context.context_revision)?;
    crate::sessions::get(&store.connection, persona, session)?;
    Ok(session)
}

struct Builtin {
    code: &'static str,
    name: &'static str,
    calendar: &'static str,
    month: Option<u32>,
    day: Option<u32>,
    term: Option<&'static str>,
    blessing: &'static str,
    memorial: bool,
}
const BUILTINS: &[Builtin] = &[
    Builtin {
        code: "solar_new_year",
        name: "元旦",
        calendar: "solar",
        month: Some(1),
        day: Some(1),
        term: None,
        blessing: "元旦快乐！愿新的一年平安顺遂。",
        memorial: false,
    },
    Builtin {
        code: "solar_labor_day",
        name: "劳动节",
        calendar: "solar",
        month: Some(5),
        day: Some(1),
        term: None,
        blessing: "劳动节快乐！辛苦了，愿你收获满满，也有时间好好休息。",
        memorial: false,
    },
    Builtin {
        code: "solar_national_day",
        name: "国庆节",
        calendar: "solar",
        month: Some(10),
        day: Some(1),
        term: None,
        blessing: "国庆节快乐！愿山河锦绣，愿你平安喜乐。",
        memorial: false,
    },
    Builtin {
        code: "lunar_laba",
        name: "腊八",
        calendar: "lunar",
        month: Some(12),
        day: Some(8),
        term: None,
        blessing: "腊八安康！愿一碗暖粥带来暖意，愿新年渐近，好事将至。",
        memorial: false,
    },
    Builtin {
        code: "lunar_new_year_eve",
        name: "除夕",
        calendar: "lunar",
        month: Some(12),
        day: None,
        term: None,
        blessing: "除夕快乐！愿团圆温暖，愿旧岁平安，新岁顺遂。",
        memorial: false,
    },
    Builtin {
        code: "lunar_spring_festival",
        name: "春节",
        calendar: "lunar",
        month: Some(1),
        day: Some(1),
        term: None,
        blessing: "新春快乐！愿你新的一年平安喜乐，万事顺意。",
        memorial: false,
    },
    Builtin {
        code: "lunar_lantern_festival",
        name: "元宵",
        calendar: "lunar",
        month: Some(1),
        day: Some(15),
        term: None,
        blessing: "元宵节快乐！愿灯火可亲，团圆常在。",
        memorial: false,
    },
    Builtin {
        code: "lunar_dragon_boat",
        name: "端午",
        calendar: "lunar",
        month: Some(5),
        day: Some(5),
        term: None,
        blessing: "端午安康！愿粽香相伴，平安常在。",
        memorial: false,
    },
    Builtin {
        code: "lunar_qixi",
        name: "七夕",
        calendar: "lunar",
        month: Some(7),
        day: Some(7),
        term: None,
        blessing: "七夕快乐！愿珍惜的人常在身旁，愿心意有所回应。",
        memorial: false,
    },
    Builtin {
        code: "lunar_mid_autumn",
        name: "中秋",
        calendar: "lunar",
        month: Some(8),
        day: Some(15),
        term: None,
        blessing: "中秋快乐！愿月圆人团圆，愿牵挂的人平安。",
        memorial: false,
    },
    Builtin {
        code: "lunar_double_ninth",
        name: "重阳",
        calendar: "lunar",
        month: Some(9),
        day: Some(9),
        term: None,
        blessing: "重阳安康！愿秋日晴朗，愿家人康健。",
        memorial: false,
    },
    Builtin {
        code: "solar_term_qingming",
        name: "清明",
        calendar: "solar_term",
        month: None,
        day: None,
        term: Some("清明"),
        blessing: "清明时节，愿思念有所安放，愿你与家人平安。",
        memorial: true,
    },
    Builtin {
        code: "solar_term_winter_solstice",
        name: "冬至",
        calendar: "solar_term",
        month: None,
        day: None,
        term: Some("冬至"),
        blessing: "冬至安康！愿寒冬有暖意，愿团圆有归处。",
        memorial: false,
    },
];

/// 母表农历节日按春节所在农历年计算；腊月属于当年春节前的上一农历年。
fn lunar_date(year: i32, month: Option<u32>, day: Option<u32>, leap: bool) -> Option<NaiveDate> {
    if !(1950..=2100).contains(&year) {
        return None;
    }
    let month = month?;
    let date = lunar::lunar_to_solar(if month == 12 { year - 1 } else { year }, month, day?, leap)?;
    (date.year() == year).then_some(date)
}
fn eve(year: i32) -> Option<NaiveDate> {
    let date = lunar::lunar_to_solar(year, 1, 1, false)?.pred_opt()?;
    (date.year() == year && (1950..=2100).contains(&year)).then_some(date)
}
fn mapping(
    code: &str,
    calendar: &str,
    year: i32,
    month: Option<u32>,
    day: Option<u32>,
    leap: bool,
    term: Option<&str>,
) -> (Option<NaiveDate>, Option<u32>) {
    match calendar {
        "lunar" if code == "lunar_new_year_eve" => {
            let date = eve(year);
            (date, date.and_then(lunar::solar_to_lunar).map(|l| l.day))
        }
        "lunar" => (lunar_date(year, month, day, leap), day),
        "solar_term" => (term.and_then(|t| lunar::solar_term_date(year, t)), None),
        _ => (None, None),
    }
}

/// 初始化可复用当前事务；不提交调用者事务、不删除母表或子表。
pub(crate) fn ensure_in_transaction(tx: &Transaction<'_>, today: NaiveDate) -> AppResult<usize> {
    let year = today.year();
    let mut changed = 0;
    for item in BUILTINS {
        let (mapped, lunar_day) = mapping(
            item.code,
            item.calendar,
            year,
            item.month,
            item.day,
            false,
            item.term,
        );
        let (solar_month, solar_day) = if item.calendar == "solar" {
            (item.month, item.day)
        } else {
            (mapped.map(|d| d.month()), mapped.map(|d| d.day()))
        };
        changed += tx.execute(
            "INSERT INTO festival_catalog(code,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,solar_term,blessing,is_memorial)
             SELECT ?1,?2,?3,?4,?5,?6,?7,?8,0,?9,?10,?11 WHERE NOT EXISTS(SELECT 1 FROM festival_catalog WHERE code=?1)",
            params![item.code,item.name,year,item.calendar,solar_month,solar_day,if item.calendar=="lunar"{item.month}else{None},lunar_day,item.term,item.blessing,item.memorial],
        )?;
    }
    let mut statement = tx.prepare("SELECT id,code,calendar_type,lunar_month,lunar_day,is_leap_month,solar_term FROM festival_catalog WHERE year<>?1")?;
    let outdated = statement
        .query_map([year], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<u32>>(3)?,
                r.get::<_, Option<u32>>(4)?,
                r.get::<_, bool>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for (id, code, calendar, month, day, leap, term) in outdated {
        if calendar == "solar" {
            changed += tx.execute(
                "UPDATE festival_catalog SET year=?1 WHERE id=?2",
                params![year, id],
            )?;
        } else {
            let (date, lunar_day) =
                mapping(&code, &calendar, year, month, day, leap, term.as_deref());
            changed += tx.execute("UPDATE festival_catalog SET year=?1,solar_month=?2,solar_day=?3,lunar_day=?4 WHERE id=?5",params![year,date.map(|d|d.month()),date.map(|d|d.day()),lunar_day,id])?;
        }
    }
    changed += session::refresh_in_transaction(tx, today)?;
    Ok(changed)
}

/// 后台日期变化时使用短写事务；失败时全部母表更新一并回滚。
pub(crate) fn synchronize(connection: &mut Connection, today: NaiveDate) -> AppResult<usize> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let count = ensure_in_transaction(&tx, today)?;
    tx.commit()?;
    Ok(count)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod session_tests;
