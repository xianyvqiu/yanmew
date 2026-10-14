use super::*;
use generation::{save, Output};
fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-10-13T02:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}
pub(super) fn database() -> Connection {
    let c = crate::db::test_database();
    c.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'一','a','a'),(2,1,'二','a','a')",[]).unwrap();
    seed(&c);
    c
}
pub(super) fn seed(c: &Connection) {
    c.execute("INSERT INTO travels(id,session_id,status,started_at,planned_return_at,actual_return_at,midway_letter_at,gift_kind,gift_state,gift_snapshot_json,facts_state,destination,activities_json,facts_version,facts_ready_at,created_at) VALUES(1,1,'returned','2026-10-13T00:00:00Z','2026-10-13T01:00:00Z','2026-10-13T01:00:00Z','2026-10-13T00:30:00Z','text','done',?1,'ready','湖边',?2,1,'a','a')",params![r#"{"schema_version":1,"kind":"text","content":"明信片","note":"给你的"}"#,r#"[{"elapsed_minutes":0,"description":"散步"}]"#]).unwrap();
}
fn output() -> Output {
    Output {
        title: "十月十三日去湖边散步，带回明信片，花了一小时".into(),
        content: "湖边散步".into(),
    }
}
fn generate(c: &mut Connection) -> TravelJournal {
    let tx = c.transaction().unwrap();
    save(&tx, 1, 1, &output(), now()).unwrap();
    tx.commit().unwrap();
    let id = list(c, 1, None, 30).unwrap()[0].id.parse().unwrap();
    get(c, 1, id).unwrap()
}
fn expected(j: &TravelJournal) -> TravelJournalExpected {
    TravelJournalExpected {
        title: j.title.clone(),
        content: j.content.clone(),
        updated_at: j.updated_at.clone(),
    }
}
#[test]
fn generated_row_and_done_commit_once_no_memory_or_reward() {
    let mut c = database();
    let before = crate::currency::get_balance(&c).unwrap();
    let j = generate(&mut c);
    let tx = c.transaction().unwrap();
    assert!(save(&tx, 1, 1, &output(), now()).is_err());
    drop(tx);
    assert_eq!(list(&c, 1, None, 30).unwrap().len(), 1);
    assert_eq!(j.travel_id, "1");
    assert_eq!(crate::currency::get_balance(&c).unwrap(), before);
    for table in ["short_memories", "long_memories"] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    assert_eq!(
        c.query_row("SELECT intimacy FROM sessions WHERE id=1", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn failed_insert_rolls_back_done_flag() {
    let mut c = database();
    c.execute_batch("CREATE TRIGGER reject_journal BEFORE INSERT ON travel_journals BEGIN SELECT RAISE(ABORT,'forced'); END;").unwrap();
    let tx = c.transaction().unwrap();
    assert!(save(&tx, 1, 1, &output(), now()).is_err());
    drop(tx);
    assert_eq!(
        c.query_row("SELECT journal_state FROM travels WHERE id=1", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "pending"
    );
    assert!(list(&c, 1, None, 30).unwrap().is_empty());
}
#[test]
fn manual_edit_allows_any_nonempty_length_and_rejects_stale_snapshot() {
    let mut c = database();
    let j = generate(&mut c);
    let id = j.id.parse().unwrap();
    let changed = update(&mut c, 1, 1, id, expected(&j), "我的标题", "很短", now()).unwrap();
    assert_eq!(changed.content, "很短");
    assert_eq!(
        update(
            &mut c,
            1,
            1,
            id,
            expected(&j),
            "过期标题",
            "旧窗口修改",
            now()
        )
        .unwrap_err()
        .code,
        "JOURNAL_CONFLICT"
    );
    assert!(update(&mut c, 1, 1, id, expected(&changed), " ", "有效正文", now()).is_err());
    assert_eq!(get(&c, 1, id).unwrap().content, "很短");
}
#[test]
fn delete_preserves_travel_gift_letters_and_done_and_edit_cannot_revive() {
    let mut c = database();
    let j = generate(&mut c);
    let id = j.id.parse().unwrap();
    c.execute("INSERT INTO souvenirs(session_id,travel_id,content,note,created_at) VALUES(1,1,'明信片','备注','a')",[]).unwrap();
    c.execute("INSERT INTO letters(session_id,travel_id,sender,content,created_at,letter_kind) VALUES(1,1,'ai','途中信','a','midway')",[]).unwrap();
    let before: String = c
        .query_row(
            "SELECT gift_snapshot_json FROM travels WHERE id=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let deleted = delete(&mut c, 1, 1, id).unwrap();
    assert_eq!(deleted.deleted_id, j.id);
    assert!(get(&c, 1, id).is_err());
    assert!(update(&mut c, 1, 1, id, expected(&j), "标题", "正文", now()).is_err());
    let tx = c.transaction().unwrap();
    assert!(save(&tx, 1, 1, &output(), now()).is_err());
    drop(tx);
    assert_eq!(
        c.query_row("SELECT journal_state FROM travels WHERE id=1", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "done"
    );
    assert_eq!(
        c.query_row(
            "SELECT gift_snapshot_json FROM travels WHERE id=1",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        before
    );
    for table in ["souvenirs", "letters"] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
#[test]
fn wrong_session_travel_and_invalid_pagination_do_not_modify() {
    let mut c = database();
    let j = generate(&mut c);
    let id = j.id.parse().unwrap();
    assert!(get(&c, 2, id).is_err());
    assert!(delete(&mut c, 2, 1, id).is_err());
    assert!(delete(&mut c, 1, 2, id).is_err());
    assert!(update(&mut c, 1, 2, id, expected(&j), "标题", "正文", now()).is_err());
    assert!(list(&c, 1, Some(0), 30).is_err());
    assert!(list(&c, 1, None, 201).is_err());
    assert_eq!(get(&c, 1, id).unwrap(), j);
}
