use super::*;

#[test]
fn v13_standby_upgrade_preserves_artworks_sequence_and_unifies_legacy_thresholds() {
    for (bored, sleep, expected) in [(Some("25"), Some("40"), "25"), (None, Some("35"), "35"), (None, None, "45")] {
        let mut c = test_database();
        c.execute_batch("ALTER TABLE accessories DROP COLUMN render_layer;ALTER TABLE accessories DROP COLUMN layer_order;").unwrap();
        let old = PET_CREATION_MIGRATION.find("CREATE TABLE IF NOT EXISTS pet_artworks (").unwrap();
        c.execute_batch("DROP TABLE pet_artworks; DELETE FROM settings WHERE key IN ('standby_minutes','idle_event_minutes'); PRAGMA user_version=13;").unwrap();
        c.execute_batch(&PET_CREATION_MIGRATION[old..]).unwrap();
        for (key,value) in [("bored_minutes",bored),("sleep_idle_minutes",sleep)] {
            if let Some(value)=value { c.execute("INSERT INTO settings VALUES(?1,?2)", params![key,value]).unwrap(); }
        }
        c.execute_batch("INSERT INTO pet_artworks(id,persona_id,kind,name,category,image_file,preview_file,revision,created_at,updated_at) VALUES(7,1,'base','原形象','normal','formal.png','preview.png',8,'原创建时间','原修改时间'); INSERT INTO pet_artworks(id,persona_id,kind,name,created_at,updated_at) VALUES(100,1,'expression','已删除','a','a'); DELETE FROM pet_artworks WHERE id=100;").unwrap();
        initialize(&mut c).unwrap();
        let original:(String,String,String,i64,String,String)=c.query_row("SELECT name,image_file,preview_file,revision,created_at,updated_at FROM pet_artworks WHERE id=7",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
        assert_eq!(original,("原形象".into(),"formal.png".into(),"preview.png".into(),8,"原创建时间".into(),"原修改时间".into()));
        assert_eq!(config::get(&c).unwrap()["idle_event_minutes"],expected);
        assert_eq!(config::get(&c).unwrap()["standby_minutes"],"10");
        assert_eq!(c.query_row("SELECT count(*) FROM settings WHERE key IN ('bored_minutes','sleep_idle_minutes')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        c.execute("INSERT INTO pet_artworks(persona_id,kind,name,category,created_at,updated_at) VALUES(1,'expression','待机','standby','a','a')",[]).unwrap();
        assert!(c.last_insert_rowid()>100);
        assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
        assert!(!initialize(&mut c).unwrap());
        assert_eq!(config::get(&c).unwrap()["idle_event_minutes"],expected);
        assert_eq!(c.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
}
