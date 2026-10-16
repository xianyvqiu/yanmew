use super::*;
use crate::platform::contracts::{Clock, ClockSample};
use chrono::{Datelike, Local, NaiveDate};
use std::{sync::Arc, time::Duration};

struct FixedClock(chrono::DateTime<chrono::Utc>);
impl Clock for FixedClock {
    fn sample(&self) -> AppResult<ClockSample> {
        Ok(ClockSample {
            utc: self.0,
            running: Duration::ZERO,
        })
    }
}
fn at(root: &Path, year: i32) -> Runtime {
    let mut runtime = fixture(root);
    let local = NaiveDate::from_ymd_opt(year, 1, 1)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .single()
        .unwrap();
    runtime.clock = Arc::new(FixedClock(local.to_utc()));
    runtime
}
fn dates(runtime: &Runtime, year: i32) {
    runtime.read(|s, _| {
        // 来源缓存故意填写其他月日；定义和文本才是需要保存的原数据。
        s.connection.execute("INSERT INTO custom_festivals(kind,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,blessing,note,source,created_at,updated_at) VALUES('birthday','农历生日',?1,'lunar',6,6,11,20,0,'原祝福','原备注','ai',?2,?2)", rusqlite::params![year, NOW])?;
        s.connection.execute("INSERT INTO custom_festivals(kind,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,blessing,source,created_at,updated_at) VALUES('custom','闰二月',?1,'lunar',6,6,2,1,1,'闰月祝福','user',?2,?2)", rusqlite::params![year, NOW])?;
        s.connection.execute("INSERT INTO custom_festivals(kind,name,year,calendar_type,solar_month,solar_day,blessing,source,created_at,updated_at) VALUES('anniversary','二月二十九',?1,'solar',2,29,'闰日祝福','user',?2,?2)", rusqlite::params![year, NOW])?;
        Ok(())
    }).unwrap();
}

#[test]
fn importing_current_or_old_year_recomputes_only_new_records_and_preserves_definitions() {
    for source_year in [2023, 2024] {
        let source_dir = Directory::new();
        let source = fixture(&source_dir.0);
        dates(&source, source_year);
        let target_dir = Directory::new();
        let target = at(&target_dir.0, 2024);
        let backup = make(&source, &source_dir.0, PackageKind::Full, &[], false);
        let preview = prepare(&target, &backup).unwrap().unwrap();
        confirm(&target, input(&preview)).unwrap();
        target
            .read(|s, _| {
                let c = &s.connection;
                let _sid: i64 = c.query_row("SELECT MAX(id) FROM sessions", [], |r| r.get(0))?;
                let day = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
                let records = crate::festivals::custom::list(c, None, 50, day)?;
                assert!(records.iter().all(|r| r.year == 2024));
                let lunar = records.iter().find(|r| r.name == "农历生日").unwrap();
                assert_eq!((lunar.solar_month, lunar.solar_day), (Some(1), Some(1)));
                assert_eq!(
                    (lunar.lunar_month, lunar.lunar_day, lunar.is_leap_month),
                    (Some(11), Some(20), false)
                );
                assert!(lunar.festival_id.is_none());
                assert_eq!(
                    (
                        &lunar.blessing,
                        lunar.note.as_deref(),
                        lunar.source.as_str(),
                        lunar.created_at.as_str(),
                        lunar.updated_at.as_str()
                    ),
                    (&"原祝福".to_string(), Some("原备注"), "ai", NOW, NOW)
                );
                assert_eq!(
                    lunar.occurrences,
                    [day, NaiveDate::from_ymd_opt(2024, 12, 20).unwrap()]
                );
                for date in &lunar.occurrences {
                    assert!(crate::festivals::custom::today(c, *date)?
                        .iter()
                        .any(|r| r.id == lunar.id && r.scope == "custom"));
                }
                let leap = records.iter().find(|r| r.name == "闰二月").unwrap();
                assert_eq!((leap.solar_month, leap.solar_day), (None, None));
                assert!(leap.occurrences.is_empty());
                assert!(leap.is_leap_month);
                let solar = records.iter().find(|r| r.name == "二月二十九").unwrap();
                assert_eq!(
                    solar.occurrences,
                    [NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()]
                );
                assert_eq!(count(c, "festival_catalog"), 13);
                assert!(c
                    .prepare("PRAGMA foreign_key_check")?
                    .query([])?
                    .next()?
                    .is_none());
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn importing_outside_calendar_range_clears_lunar_cache_and_skips_missing_solar_date() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    dates(&source, 2024);
    let target_dir = Directory::new();
    let target = at(&target_dir.0, 2101);
    let backup = make(&source, &source_dir.0, PackageKind::Full, &[], false);
    let preview = prepare(&target, &backup).unwrap().unwrap();
    confirm(&target, input(&preview)).unwrap();
    target
        .read(|s, _| {
            let _sid: i64 = s
                .connection
                .query_row("SELECT MAX(id) FROM sessions", [], |r| r.get(0))?;
            let records = crate::festivals::custom::list(
                &s.connection,
                None,
                50,
                NaiveDate::from_ymd_opt(2101, 1, 1).unwrap(),
            )?;
            for record in records.iter().filter(|r| r.calendar_type == "lunar") {
                assert_eq!(
                    (record.year, record.solar_month, record.solar_day),
                    (2101, None, None)
                );
                assert!(record.occurrences.is_empty());
                assert!(record.lunar_month.is_some());
            }
            let solar = records.iter().find(|r| r.name == "二月二十九").unwrap();
            assert_eq!(
                (solar.year, solar.solar_month, solar.solar_day),
                (2101, Some(2), Some(29))
            );
            assert!(solar.occurrences.is_empty());
            Ok(())
        })
        .unwrap();
}

#[test]
fn calendar_refresh_failure_rolls_back_full_import_and_can_retry_same_preview() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    dates(&source, 2023);
    let target_dir = Directory::new();
    let target = at(&target_dir.0, 2024);
    target.read(|s, _| { s.connection.execute("UPDATE user_state SET coins=888 WHERE id=1", [])?;
        s.connection.execute_batch("CREATE TRIGGER reject_import_date BEFORE UPDATE ON custom_festivals BEGIN SELECT RAISE(ABORT,'calendar failure'); END;")?; Ok(()) }).unwrap();
    let backup = make(&source, &source_dir.0, PackageKind::Full, &[], false);
    let preview = prepare(&target, &backup).unwrap().unwrap();
    let before = target
        .read(|s, _| {
            Ok((
                count(&s.connection, "sessions"),
                count(&s.connection, "custom_festivals"),
                s.context_revision,
            ))
        })
        .unwrap();
    assert!(confirm(&target, input(&preview)).is_err());
    target
        .read(|s, _| {
            assert_eq!(
                (
                    count(&s.connection, "sessions"),
                    count(&s.connection, "custom_festivals"),
                    s.context_revision
                ),
                before
            );
            assert_eq!(
                s.connection
                    .query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0))?,
                888
            );
            s.connection
                .execute_batch("DROP TRIGGER reject_import_date")?;
            Ok(())
        })
        .unwrap();
    assert!(current_preview(&target, &preview.preview_id).is_ok());
    assert!(confirm(&target, input(&preview)).is_ok());
    target.read(|s, _| {
        let year: i32 = s.connection.query_row("SELECT year FROM custom_festivals WHERE name='农历生日'", [], |r| r.get(0))?;
        assert_eq!(year, target.clock.sample()?.utc.with_timezone(&Local).year()); Ok(())
    }).unwrap();
}

#[test]
fn global_dates_full_restore_and_partial_packages_preserve_scope_for_new_and_legacy_formats() {
    for legacy in [false,true] {
        for kind in [PackageKind::Full,PackageKind::Session,PackageKind::PersonaFull,PackageKind::UserProfile] {
            let source_dir=Directory::new();let source=fixture(&source_dir.0);
            dates(&source,2024);
            let target_dir=Directory::new();let target=at(&target_dir.0,2024);
            target.read(|s,_|{s.connection.execute("UPDATE custom_festivals SET name='本机日期',note='不可被部分导入覆盖'",[])?;Ok(())}).unwrap();
            let full=kind==PackageKind::Full;
            let backup=make(&source,&source_dir.0,kind,if full {&[]}else if kind==PackageKind::UserProfile {&["10"]}else{&["2"]},kind==PackageKind::UserProfile);
            {
                let mut zip=zip::ZipArchive::new(std::fs::File::open(&backup.0).unwrap()).unwrap();
                let data:Value=serde_json::from_reader(zip.by_name("data.json").unwrap()).unwrap();
                assert_eq!(data.get("custom_festivals").is_some(),full);
                if full {assert!(data["custom_festivals"].as_array().unwrap().iter().all(|row|row.get("session_id").is_none()));}
            }
            if legacy {
                rewrite(&backup.0,|manifest,data| {
                    manifest.schema_version=12;
                    if !full {data.as_object_mut().unwrap().insert("session_festivals".into(),serde_json::json!([{
                        "id":"999","session_id":"2","festival_code":null,"kind":"birthday","name":"旧部分日期","year":2024,
                        "calendar_type":"solar","solar_month":2,"solar_day":29,"lunar_month":null,"lunar_day":null,"is_leap_month":0,
                        "blessing":"原祝福","note":null,"source":"ai","created_at":NOW,"updated_at":NOW
                    }]));}
                });
            }
            let original=target.read(|s,_|Ok(serde_json::to_value(crate::festivals::custom::list(&s.connection,None,50,NaiveDate::from_ymd_opt(2024,1,1).unwrap())?).unwrap())).unwrap();
            let preview=prepare(&target,&backup).unwrap().unwrap();
            if legacy&&!full {assert!(preview.warnings.iter().any(|warning|warning.contains("跳过")));}
            confirm(&target,input(&preview)).unwrap();
            target.read(|s,_|{
                let current=crate::festivals::custom::list(&s.connection,None,50,NaiveDate::from_ymd_opt(2024,1,1).unwrap())?;
                if full {
                    assert_eq!(current.len(),4);
                    assert!(current.iter().any(|row|row.name=="农历生日"));
                    assert!(!current.iter().any(|row|row.name=="本机日期"));
                } else {assert_eq!(serde_json::to_value(current).unwrap(),original);}
                Ok(())
            }).unwrap();
        }
    }
}
