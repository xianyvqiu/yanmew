use super::*;
fn date(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 1, 1).unwrap()
}
fn database() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(include_str!("../../../db/schema.sql"))
        .unwrap();
    connection
        .pragma_update(None, "foreign_keys", true)
        .unwrap();
    connection
}
fn mapped(c: &Connection, code: &str) -> (i32, Option<u32>, Option<u32>, Option<u32>) {
    c.query_row(
        "SELECT year,solar_month,solar_day,lunar_day FROM festival_catalog WHERE code=?1",
        [code],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .unwrap()
}
#[test]
fn builtin_catalog_is_persisted_idempotently_with_clear_dates() {
    let mut c = database();
    assert_eq!(synchronize(&mut c, date(2024)).unwrap(), 13);
    assert_eq!(synchronize(&mut c, date(2024)).unwrap(), 0);
    assert_eq!(mapped(&c, "lunar_laba"), (2024, Some(1), Some(18), Some(8)));
    assert_eq!(
        mapped(&c, "lunar_new_year_eve"),
        (2024, Some(2), Some(9), Some(30))
    );
    assert_eq!(
        mapped(&c, "lunar_spring_festival"),
        (2024, Some(2), Some(10), Some(1))
    );
    assert_eq!(
        mapped(&c, "solar_term_qingming"),
        (2024, Some(4), Some(4), None)
    );
    assert_eq!(
        mapped(&c, "solar_term_winter_solstice"),
        (2024, Some(12), Some(21), None)
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM festival_catalog WHERE length(trim(blessing))=0",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row(
            "SELECT is_memorial FROM festival_catalog WHERE code='solar_term_qingming'",
            [],
            |r| r.get::<_, bool>(0)
        )
        .unwrap(),
        true
    );
    assert!(!c
        .query_row(
            "SELECT blessing FROM festival_catalog WHERE code='solar_term_qingming'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap()
        .contains("快乐"));
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM festival_catalog WHERE name='小年'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn forward_and_backward_year_updates_keep_ids_definitions_and_links() {
    let mut c = crate::db::test_database();
    synchronize(&mut c, date(2024)).unwrap();
    let id: i64 = c
        .query_row(
            "SELECT id FROM festival_catalog WHERE code='lunar_new_year_eve'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    c.execute(
        "UPDATE festival_catalog SET blessing='保留祝福',note='保留备注' WHERE id=?1",
        [id],
    )
    .unwrap();
    c.execute("INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(1,'日期测试','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z')",[]).unwrap();
    let _session = c.last_insert_rowid();
    c.execute("INSERT INTO custom_festivals(festival_id,kind,name,year,calendar_type,solar_month,solar_day,blessing,source,created_at,updated_at) VALUES(?1,'festival','会话记录',2024,'solar',1,2,'子表祝福','user','2024-01-01T00:00:00Z','2024-01-01T00:00:00Z')",params![id]).unwrap();
    synchronize(&mut c, date(2025)).unwrap();
    assert_eq!(
        mapped(&c, "lunar_new_year_eve"),
        (2025, Some(1), Some(28), Some(29))
    );
    assert_eq!(mapped(&c, "lunar_laba"), (2025, Some(1), Some(7), Some(8)));
    synchronize(&mut c, date(2024)).unwrap();
    assert_eq!(
        mapped(&c, "lunar_new_year_eve"),
        (2024, Some(2), Some(9), Some(30))
    );
    let row: (i64, String, String) = c
        .query_row(
            "SELECT id,blessing,note FROM festival_catalog WHERE code='lunar_new_year_eve'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, (id, "保留祝福".into(), "保留备注".into()));
    assert_eq!(
        c.query_row(
            "SELECT festival_id FROM custom_festivals",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        id
    );
    assert_eq!(
        c.query_row(
            "SELECT year FROM custom_festivals",
            [],
            |r| r.get::<_, i32>(0)
        )
        .unwrap(),
        2024
    ); // 回拨后母子表均恢复目标公历年份。
}
#[test]
fn outside_calendar_range_clears_derived_dates_but_keeps_solar_dates() {
    let mut c = database();
    synchronize(&mut c, date(2024)).unwrap();
    for year in [1949, 2101] {
        synchronize(&mut c, date(year)).unwrap();
        assert_eq!(mapped(&c, "solar_new_year"), (year, Some(1), Some(1), None));
        assert_eq!(mapped(&c, "lunar_new_year_eve"), (year, None, None, None));
        assert_eq!(c.query_row("SELECT count(*) FROM festival_catalog WHERE calendar_type<>'solar' AND (solar_month IS NOT NULL OR solar_day IS NOT NULL)",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    synchronize(&mut c, date(1950)).unwrap();
    assert_eq!(
        mapped(&c, "lunar_spring_festival"),
        (1950, Some(2), Some(17), Some(1))
    );
    assert!(mapped(&c, "lunar_laba").1.is_some());
}
#[test]
fn failed_year_update_rolls_back_every_row() {
    let mut c = database();
    synchronize(&mut c, date(2024)).unwrap();
    c.execute_batch("CREATE TRIGGER reject_year BEFORE UPDATE OF year ON festival_catalog WHEN NEW.code='lunar_dragon_boat' BEGIN SELECT RAISE(ABORT,'simulated failure'); END;").unwrap();
    assert!(synchronize(&mut c, date(2025)).is_err());
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM festival_catalog WHERE year=2024",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        13
    );
    assert_eq!(
        mapped(&c, "lunar_new_year_eve"),
        (2024, Some(2), Some(9), Some(30))
    );
    c.execute_batch("DROP TRIGGER reject_year").unwrap();
    synchronize(&mut c, date(2025)).unwrap();
    assert_eq!(
        mapped(&c, "lunar_new_year_eve"),
        (2025, Some(1), Some(28), Some(29))
    );
}
