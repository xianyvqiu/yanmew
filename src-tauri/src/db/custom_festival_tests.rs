use super::*;
use chrono::Datelike;

fn legacy() -> Connection {
    let c=test_database();
    c.execute_batch("ALTER TABLE accessories DROP COLUMN render_layer; ALTER TABLE accessories DROP COLUMN layer_order;").unwrap();
    let start=SCHEMA.find("CREATE TABLE IF NOT EXISTS custom_festivals (").unwrap();
    let end=start+SCHEMA[start..].find("\n);").unwrap()+3;
    let old=SCHEMA[start..end].replace("custom_festivals","session_festivals")
        .replace("    festival_id", "    session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,\n    festival_id");
    c.execute_batch("DROP TABLE custom_festivals;").unwrap();
    c.execute_batch(&old).unwrap();
    c.execute_batch("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'甲','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),(2,1,'乙','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'); PRAGMA user_version=12;").unwrap();
    for (id,session) in [(7,1),(8,2),(100,1)] {
        c.execute("INSERT INTO session_festivals(id,session_id,kind,name,year,calendar_type,solar_month,solar_day,blessing,note,source,created_at,updated_at) VALUES(?1,?2,'birthday','同名生日',?3,'solar',2,29,'保留祝福','保留备注','ai','2024-01-01T00:00:00Z','2024-02-01T00:00:00Z')",params![id,session,chrono::Local::now().year()]).unwrap();
    }
    c.execute("DELETE FROM session_festivals WHERE id=100",[]).unwrap();
    c
}

#[test]
fn global_date_migration_preserves_duplicates_ids_text_sequence_and_survives_session_deletion() {
    let mut c=legacy();
    initialize(&mut c).unwrap();
    assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
    assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE name='session_festivals'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(c.query_row("SELECT count(*) FROM pragma_table_info('custom_festivals') WHERE name='session_id'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(c.query_row("SELECT group_concat(id) FROM custom_festivals ORDER BY id",[],|r|r.get::<_,String>(0)).unwrap(),"7,8");
    assert_eq!(c.query_row("SELECT count(*) FROM custom_festivals WHERE name='同名生日' AND blessing='保留祝福' AND note='保留备注' AND source='ai' AND created_at='2024-01-01T00:00:00Z' AND updated_at='2024-02-01T00:00:00Z'",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    c.execute("DELETE FROM sessions",[]).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM custom_festivals",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    let row=crate::festivals::custom::create(&mut c,crate::festivals::custom::Input {kind:"custom".into(),name:"全局日期".into(),calendar_type:"solar".into(),month:1,day:2,is_leap_month:false,blessing:"祝福".into(),note:None},"user",chrono::Local::now().date_naive(),chrono::Utc::now()).unwrap();
    assert!(row.id.parse::<i64>().unwrap()>100);
    assert!(!initialize(&mut c).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn global_date_migration_failure_rolls_back_old_table_and_version() {
    let mut c=legacy();
    c.pragma_update(None,"foreign_keys",false).unwrap();
    c.execute("UPDATE session_festivals SET festival_id=999999 WHERE id=7",[]).unwrap();
    assert!(initialize(&mut c).is_err());
    assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),12);
    assert_eq!(c.query_row("SELECT count(*) FROM session_festivals",[],|r|r.get::<_,i64>(0)).unwrap(),2);
    assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE name='custom_festivals'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
}
