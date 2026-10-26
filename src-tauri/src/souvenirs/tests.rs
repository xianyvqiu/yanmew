use super::*;
use chrono::{DateTime, Utc};
use generation::{save, Gift};
fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-13T01:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}
fn db() -> Connection {
    let c = crate::db::test_database();
    c.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'一','a','a'),(2,1,'二','a','a')",[]).unwrap();
    for (id, kind) in [(1, "text"), (2, "accessory")] {
        c.execute("INSERT INTO travels(id,session_id,status,started_at,planned_return_at,actual_return_at,midway_letter_at,gift_kind,facts_state,destination,activities_json,facts_version,facts_ready_at,created_at) VALUES(?1,1,'returned','2026-10-13T00:00:00Z','2026-10-13T01:00:00Z','2026-10-13T01:00:00Z','2026-10-13T00:30:00Z',?2,'ready','湖边','[{\"elapsed_minutes\":0,\"description\":\"散步\"}]',1,'a','a')",params![id,kind]).unwrap();
    }
    c
}
fn text() -> Gift {
    Gift::Text {
        title:Some("湖边明信片".into()),
        content: "湖边的明信片".into(),
        note: "给你挑了一张。".into(),
    }
}
fn accessory() -> Gift {
    Gift::Accessory {
        name: "小花".into(),
        display_text: "✿".into(),
        description: "湖边的小花。".into(),
    }
}
#[test]
fn text_snapshot_and_row_commit_once_delete_does_not_regrant() {
    let mut c = db();
    let tx = c.transaction().unwrap();
    save(&tx, 1, 1, "text", &text(), None, now()).unwrap();
    tx.commit().unwrap();
    let rows = list(&c, 1, None, 30).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].content, "湖边的明信片");
    assert!(get(&c, 2, rows[0].id.parse().unwrap()).is_err());
    let id = rows[0].id.parse().unwrap();
    assert!(favorite(&c, 1, id, true).unwrap().is_favorite);
    delete(&c, 1, id).unwrap();
    assert!(list(&c, 1, None, 30).unwrap().is_empty());
    let (state, snapshot): (String, String) = c
        .query_row(
            "SELECT gift_state,gift_snapshot_json FROM travels WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, "done");
    let snapshot: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(snapshot["content"], "湖边的明信片");
    let tx = c.transaction().unwrap();
    assert!(save(&tx, 1, 1, "text", &text(), None, now()).is_err());
    drop(tx);
    assert_eq!(
        c.query_row(
            "SELECT return_letter_state FROM travels WHERE id=1",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "pending"
    );
}
#[test]
fn accessory_is_free_unworn_and_does_not_consume_coins_or_tiers() {
    let mut c = db();
    let before = crate::currency::get_balance(&c).unwrap();
    let tx = c.transaction().unwrap();
    save(&tx, 2, 1, "accessory", &accessory(), None, now()).unwrap();
    tx.commit().unwrap();
    assert!(list(&c, 1, None, 30).unwrap().is_empty());
    let (source, cost, worn, image): (String, i64, bool, Option<String>) = c
        .query_row(
            "SELECT source,cost,is_worn,image_file FROM accessories WHERE travel_id=2",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(source, "travel");
    assert_eq!(cost, 0);
    assert!(!worn);
    assert!(image.is_none());
    assert_eq!(crate::currency::get_balance(&c).unwrap(), before);
    assert_eq!(
        c.query_row(
            "SELECT craft_tier_1_used+craft_tier_2_used+craft_tier_3_used FROM sessions WHERE id=1",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn insert_failure_rolls_back_snapshot_and_done_state() {
    let mut c = db();
    c.execute_batch("CREATE TRIGGER reject_gift BEFORE INSERT ON souvenirs BEGIN SELECT RAISE(ABORT,'forced failure'); END;").unwrap();
    let tx = c.transaction().unwrap();
    assert!(save(&tx, 1, 1, "text", &text(), None, now()).is_err());
    drop(tx);
    let state: (String, Option<String>) = c
        .query_row(
            "SELECT gift_state,gift_snapshot_json FROM travels WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, ("pending".into(), None));
}
#[test]
fn cancelled_unready_wrong_kind_and_cross_table_conflicts_do_not_grant() {
    let mut c = db();
    for (id, session, kind, gift) in [
        (1, 2, "text", text()),
        (1, 1, "accessory", accessory()),
        (2, 1, "text", text()),
    ] {
        let tx = c.transaction().unwrap();
        assert!(save(&tx, id, session, kind, &gift, None, now()).is_err());
        drop(tx);
    }
    c.execute("UPDATE travels SET facts_state='pending',destination=NULL,activities_json=NULL,facts_version=0,facts_ready_at=NULL WHERE id=1",[]).unwrap();
    let tx = c.transaction().unwrap();
    assert!(save(&tx, 1, 1, "text", &text(), None, now()).is_err());
    drop(tx);
    c.execute("UPDATE travels SET facts_state='cancelled',status='cancelled',actual_return_at=NULL,cancelled_at='a',gift_kind=NULL,gift_state='cancelled' WHERE id=1",[]).unwrap();
    let tx = c.transaction().unwrap();
    assert!(save(&tx, 1, 1, "text", &text(), None, now()).is_err());
    drop(tx);
    c.execute("INSERT INTO souvenirs(session_id,travel_id,content,note,created_at) VALUES(1,2,'异常旧礼物','a','a')",[]).unwrap();
    let tx = c.transaction().unwrap();
    assert_eq!(
        save(&tx, 2, 1, "accessory", &accessory(), None, now())
            .unwrap_err()
            .code,
        "TRAVEL_GIFT_CONFLICT"
    );
    drop(tx);
    assert_eq!(
        c.query_row("SELECT count(*) FROM accessories", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn deletion_and_favorite_are_scoped_pagination_is_validated() {
    let mut c = db();
    let tx = c.transaction().unwrap();
    save(&tx, 1, 1, "text", &text(), None, now()).unwrap();
    tx.commit().unwrap();
    let id = list(&c, 1, None, 30).unwrap()[0].id.parse().unwrap();
    assert!(favorite(&c, 2, id, true).is_err());
    assert!(delete(&c, 2, id).is_err());
    assert!(list(&c, 1, Some(0), 30).is_err());
    assert!(list(&c, 1, None, 201).is_err());
    assert_eq!(list(&c, 1, None, 30).unwrap().len(), 1);
}
