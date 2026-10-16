use super::*;
struct Fixed(u64);
impl RandomSource for Fixed {
    fn below(&mut self, upper: u64) -> AppResult<u64> {
        Ok(self.0 % upper)
    }
}
fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-08T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}
fn db() -> Connection {
    let c = crate::db::test_database();
    c.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at,travel_trigger_remaining_seconds) VALUES(1,1,'一','a','a',10),(2,1,'二','a','a',99)",[]).unwrap();
    c.execute("INSERT INTO settings VALUES('current_session_id','1')", [])
        .unwrap();
    c
}
fn advance(c: &mut Connection, seconds: u64) -> bool {
    let tx = c.transaction().unwrap();
    let away = advance_in_transaction(&tx, 1, seconds, now(), &mut Fixed(0)).unwrap();
    tx.commit().unwrap();
    away
}
fn wait(c: &Connection, id: i64) -> Option<i64> {
    c.query_row(
        "SELECT travel_trigger_remaining_seconds FROM sessions WHERE id=?1",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}
fn travel(c: &Connection) -> (i64, String, String, String) {
    c.query_row(
        "SELECT id,status,started_at,planned_return_at FROM travels LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .unwrap()
}
fn pending_letter(c: &Connection, id: i64) {
    c.execute("INSERT INTO letters(session_id,travel_id,sender,content,reply_due_at,reply_state,letter_kind,created_at) VALUES(1,?1,'user','你好','2026-10-08T00:40:00Z','pending','user','a')",[id]).unwrap();
}
fn facts() -> Facts {
    Facts {
        destination: "湖边".into(),
        activities: vec![
            Activity {
                elapsed_minutes: 0,
                description: "出发散步".into(),
            },
            Activity {
                elapsed_minutes: 30,
                description: "看湖水".into(),
            },
        ],
    }
}

#[test]
fn wait_draw_includes_both_seconds_endpoints_and_stays_on_interaction() {
    let mut c = db();
    assert_eq!(draw_wait(&c, &mut Fixed(0)).unwrap(), 7200);
    assert_eq!(draw_wait(&c, &mut Fixed(7200)).unwrap(), 14400);
    assert!(!advance(&mut c, 3));
    assert_eq!(wait(&c, 1), Some(7));
    let scope = crate::pet_store::PetWriteScope {
        persona_id: 1,
        session_id: 1,
        context_revision: 0,
        state_revision: crate::pet_store::read(&c, 1).unwrap().state_revision,
    };
    crate::pet::interact(
        &mut c,
        0,
        scope,
        crate::pet_store::Interaction::Click,
        now(),
    )
    .unwrap();
    assert_eq!(wait(&c, 1), Some(7));
    assert_eq!(wait(&c, 2), Some(99));
}
#[test]
fn crossing_zero_creates_one_departure_and_clears_counter() {
    let mut c = db();
    assert!(advance(&mut c, 12));
    assert_eq!(wait(&c, 1), None);
    let (id, status, start, end) = travel(&c);
    assert_eq!(status, "away");
    assert_eq!(start, time(now()));
    assert_eq!(end, time(now() + chrono::Duration::minutes(60)));
    let midway: String = c
        .query_row(
            "SELECT midway_letter_at FROM travels WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(midway, time(now() + chrono::Duration::minutes(30)));
    assert!(advance(&mut c, 50));
    assert_eq!(
        c.query_row("SELECT count(*) FROM travels", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(c.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(1,'away','a','b','a','a')",[]).is_err());
}
#[test]
fn departure_failure_rolls_back_pet_checkpoint_and_wait() {
    let mut c = db();
    c.execute_batch("CREATE TRIGGER fail_departure BEFORE INSERT ON travels BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    let snapshot = crate::pet_store::read(&c, 1).unwrap();
    let scope = crate::pet_store::PetWriteScope {
        persona_id: 1,
        session_id: 1,
        context_revision: 0,
        state_revision: snapshot.state_revision,
    };
    let tick = crate::pet::Tick {
        now: now(),
        effective_elapsed: Duration::from_secs(10),
        idle_elapsed: Duration::from_secs(10),
        allow_idle_transition: true,
    };
    assert!(crate::pet::advance_with_travel(&mut c, 0, scope, tick, &mut Fixed(0), 10).is_err());
    assert_eq!(wait(&c, 1), Some(10));
    assert_eq!(crate::pet_store::read(&c, 1).unwrap(), snapshot);
}
#[test]
fn expiry_restores_planned_time_and_gift_type_only_once() {
    let mut c = db();
    advance(&mut c, 10);
    let (id, _, _, planned) = travel(&c);
    pending_letter(&c, id);
    assert_eq!(
        recover_returns(&mut c, now() + chrono::Duration::minutes(59), &mut Fixed(0)).unwrap(),
        0
    );
    assert_eq!(
        recover_returns(&mut c, now() + chrono::Duration::days(1), &mut Fixed(0)).unwrap(),
        1
    );
    let state: (String, String, String, String, String) = c
        .query_row(
            "SELECT status,actual_return_at,gift_kind,midway_letter_state,gift_state FROM travels",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        state,
        (
            "returned".into(),
            planned,
            "accessory".into(),
            "cancelled".into(),
            "pending".into()
        )
    );
    assert_eq!(wait(&c, 1), Some(7200));
    assert_eq!(
        c.query_row("SELECT reply_state FROM letters", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "cancelled"
    );
    assert_eq!(
        recover_returns(&mut c, now() + chrono::Duration::days(2), &mut Fixed(99)).unwrap(),
        0
    );
    assert_eq!(wait(&c, 1), Some(7200));
    assert_eq!(travel(&c).1, "returned");
}
#[test]
fn switch_cancels_pending_work_and_retains_existing_letter() {
    let mut c = db();
    advance(&mut c, 10);
    let id = travel(&c).0;
    pending_letter(&c, id);
    assert!(crate::sessions::select(&mut c, 1, 2).unwrap());
    let states: (String, String, String, String) = c
        .query_row(
            "SELECT status,facts_state,gift_state,journal_state FROM travels",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        states,
        (
            "cancelled".into(),
            "cancelled".into(),
            "cancelled".into(),
            "cancelled".into()
        )
    );
    assert!((7200..=14400).contains(&wait(&c, 1).unwrap()));
    assert_eq!(wait(&c, 2), Some(99));
    assert_eq!(
        c.query_row("SELECT reply_state FROM letters", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "cancelled"
    );
    assert_eq!(
        recover_returns(&mut c, now() + chrono::Duration::days(1), &mut Fixed(0)).unwrap(),
        0
    );
    let wait = wait(&c, 1);
    crate::sessions::select(&mut c, 1, 1).unwrap();
    assert_eq!(super::tests::wait(&c, 1), wait);
}
#[test]
fn new_sessions_draw_wait_in_the_creation_transaction() {
    let mut c = db();
    let s = crate::sessions::create(&mut c, 1).unwrap();
    assert!((7200..=14400).contains(&wait(&c, s.id.parse().unwrap()).unwrap()));
}
#[test]
fn facts_are_immutable_and_cancelled_results_are_discarded() {
    let mut c = db();
    advance(&mut c, 10);
    let (id, _, start, end) = travel(&c);
    let prompt: String = c
        .query_row("SELECT prompt FROM personas WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert!(facts::save(&c, id, 1, 1, &prompt, &start, &end, &facts(), now()).unwrap());
    assert!(!facts::save(&c, id, 1, 1, &prompt, &start, &end, &facts(), now()).unwrap());
    crate::sessions::select(&mut c, 1, 2).unwrap();
    assert_eq!(
        c.query_row("SELECT facts_state FROM travels", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "ready"
    );
    assert!(!facts::save(&c, id, 1, 1, &prompt, &start, &end, &facts(), now()).unwrap());
    let mut c = db();
    advance(&mut c, 10);
    let (id, _, start, end) = travel(&c);
    crate::sessions::select(&mut c, 1, 2).unwrap();
    assert!(!facts::save(&c, id, 1, 1, &prompt, &start, &end, &facts(), now()).unwrap());
}
#[test]
fn facts_can_complete_after_offline_return_but_not_after_persona_edit() {
    let mut c = db();
    advance(&mut c, 10);
    let (id, _, start, end) = travel(&c);
    let prompt: String = c
        .query_row("SELECT prompt FROM personas WHERE id=1", [], |r| r.get(0))
        .unwrap();
    recover_returns(&mut c, now() + chrono::Duration::hours(2), &mut Fixed(99)).unwrap();
    c.execute("UPDATE personas SET prompt='已修改' WHERE id=1", [])
        .unwrap();
    assert!(!facts::save(&c, id, 1, 1, &prompt, &start, &end, &facts(), now()).unwrap());
    assert!(facts::save(&c, id, 1, 1, "已修改", &start, &end, &facts(), now()).unwrap());
}
#[test]
fn invalid_timeline_and_incomplete_ai_output_never_become_facts() {
    let f = facts();
    f.validate(60).unwrap();
    assert_eq!(f.happened(Duration::from_secs(29 * 60)).len(), 1);
    let mut invalid = f.clone();
    invalid.activities[1].elapsed_minutes = 0;
    assert!(invalid.validate(60).is_err());
    invalid.activities[1].elapsed_minutes = 61;
    assert!(invalid.validate(60).is_err());
    let mut choice = crate::ai::provider::Choice {
        reasoning_content: None,
        index: 0,
        content: serde_json::to_string(&f).unwrap(),
        tools: vec![],
        finish: crate::ai::provider::Finish::Length,
    };
    assert!(facts::parse(&choice, 60).is_err());
    choice.finish = crate::ai::provider::Finish::Stop;
    assert_eq!(facts::parse(&choice, 60).unwrap(), f);
}

#[test]
fn history_queries_keep_session_scope_and_hide_unissued_gifts() {
    let mut c=db();
    assert!(queries::current(&c,1).unwrap().is_none());
    advance(&mut c,10);
    let current=queries::current(&c,1).unwrap().unwrap();
    assert_eq!(current.session_id,"1");
    assert!(queries::current(&c,2).unwrap().is_none());
    assert!(queries::list(&c,2,None,30).unwrap().is_empty());
    assert!(queries::list(&c,1,Some(current.id.parse().unwrap()),30).unwrap().is_empty());
    assert!(queries::list(&c,1,None,0).is_err());
    recover_returns(&mut c,now()+chrono::Duration::hours(4),&mut Fixed(1)).unwrap();
    assert!(queries::current(&c,1).unwrap().is_none());
    let history=queries::list(&c,1,None,30).unwrap();
    assert_eq!(history.len(),1);
    assert_eq!(history[0].status,TravelStatus::Returned);
    assert_eq!(history[0].gift_state,"pending");
    assert!(history[0].gift_kind.is_none());
}
