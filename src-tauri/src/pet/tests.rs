use super::*;
use std::collections::VecDeque;

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-20T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn database() -> Connection {
    let connection = crate::db::test_database();
    for id in [1, 2] {
        connection.execute(
            "INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(?1,1,'测试','a','a')", [id],
        ).unwrap();
    }
    connection
        .execute("INSERT INTO settings VALUES('current_session_id','1')", [])
        .unwrap();
    connection
}

fn scope(connection: &Connection) -> PetWriteScope {
    PetWriteScope {
        persona_id: 1,
        session_id: 1,
        context_revision: 0,
        state_revision: pet_store::read(connection, 1).unwrap().state_revision,
    }
}

struct Random {
    values: VecDeque<u64>,
    bounds: Vec<u64>,
}

impl Random {
    fn new(values: &[u64]) -> Self {
        Self {
            values: values.iter().copied().collect(),
            bounds: vec![],
        }
    }
}

impl RandomSource for Random {
    fn below(&mut self, upper_exclusive: u64) -> AppResult<u64> {
        self.bounds.push(upper_exclusive);
        self.values
            .pop_front()
            .ok_or_else(|| AppError::new("TEST_RANDOM_ERROR", "没有随机数"))
    }
}

fn tick(minutes: u64) -> Tick {
    Tick {
        now: now(),
        effective_elapsed: Duration::from_secs(minutes * 60),
        idle_elapsed: Duration::from_secs(minutes * 60),
        allow_idle_transition: true,
    }
}

fn run(connection: &mut Connection, tick: Tick, random: &mut Random) -> AppResult<AdvanceResult> {
    advance(connection, 0, scope(connection), tick, random)
}

fn setting(connection: &Connection, key: &str, value: &str) {
    connection
        .execute(
            "UPDATE settings SET value=?1 WHERE key=?2",
            params![value, key],
        )
        .unwrap();
}

#[test]
fn default_boundary_selects_bored_once_and_preserves_hunger() {
    let mut connection = database();
    let mut random = Random::new(&[0]);
    let early = run(&mut connection, tick(44), &mut random).unwrap();
    assert_eq!(early.transition, Transition::None);
    assert_eq!(early.snapshot.idle_branch, IdleBranch::None);
    assert!(random.bounds.is_empty());
    let entered = run(&mut connection, tick(1), &mut random).unwrap();
    assert_eq!(entered.transition, Transition::EnteredBored);
    let later = run(&mut connection, tick(75), &mut random).unwrap();
    assert_eq!(later.transition, Transition::None);
    assert_eq!(later.snapshot.idle_branch, IdleBranch::Bored);
    assert!(later.hungry);
    assert_eq!(random.bounds, vec![2]);
}

#[test]
fn sleep_deadline_is_drawn_once_and_expiry_starts_new_idle_cycle() {
    let mut connection = database();
    let mut random = Random::new(&[1, 60]);
    let entered = run(&mut connection, tick(45), &mut random).unwrap();
    assert_eq!(entered.transition, Transition::EnteredSleep);
    let deadline = now() + chrono::Duration::minutes(120);
    assert_eq!(
        entered.snapshot.sleep_until,
        Some(deadline.to_rfc3339_opts(SecondsFormat::Millis, true))
    );
    let mut sleeping = tick(30);
    sleeping.now = now() + chrono::Duration::minutes(30);
    let continued = run(&mut connection, sleeping, &mut random).unwrap();
    assert_eq!(continued.transition, Transition::None);
    assert_eq!(continued.snapshot.idle_elapsed_ms, 45 * 60_000);
    assert_eq!(continued.snapshot.sleep_until, entered.snapshot.sleep_until);
    // 模拟关闭后恢复：UTC 前进一天，有效区间为零，不能补算离线时间。
    let mut reopened = tick(0);
    reopened.now = now() + chrono::Duration::days(1);
    let woke = run(&mut connection, reopened, &mut random).unwrap();
    assert_eq!(woke.transition, Transition::Woke);
    assert_eq!(woke.snapshot.idle_elapsed_ms, 0);
    assert_eq!(
        woke.snapshot.hunger_elapsed_ms,
        continued.snapshot.hunger_elapsed_ms
    );
    assert_eq!(woke.snapshot.sleep_until, None);
    assert_eq!(random.bounds, vec![2, 61]);
    let again = run(&mut connection, tick(0), &mut random).unwrap();
    assert_eq!(again.transition, Transition::None);
}

#[test]
fn distinct_thresholds_choose_earliest_even_when_both_crossed_in_one_tick() {
    for (bored, sleep, expected) in [
        ("10", "20", IdleBranch::Bored),
        ("20", "10", IdleBranch::Sleeping),
    ] {
        let mut connection = database();
        setting(&connection, "bored_minutes", bored);
        setting(&connection, "sleep_idle_minutes", sleep);
        setting(&connection, "sleep_min_minutes", "60");
        setting(&connection, "sleep_max_minutes", "60");
        let mut random = Random::new(&[]);
        let result = run(&mut connection, tick(25), &mut random).unwrap();
        assert_eq!(result.snapshot.idle_branch, expected);
        assert!(random.bounds.is_empty());
    }
}

#[test]
fn settings_changes_take_effect_without_reselecting_an_existing_branch() {
    let mut connection = database();
    let mut random = Random::new(&[]);
    run(&mut connection, tick(20), &mut random).unwrap();
    setting(&connection, "bored_minutes", "15");
    setting(&connection, "hungry_minutes", "15");
    let changed = run(&mut connection, tick(0), &mut random).unwrap();
    assert_eq!(changed.transition, Transition::EnteredBored);
    assert!(changed.hungry);
    setting(&connection, "sleep_idle_minutes", "1");
    setting(&connection, "hungry_minutes", "120");
    let stable = run(&mut connection, tick(0), &mut random).unwrap();
    assert_eq!(stable.snapshot.idle_branch, IdleBranch::Bored);
    assert_eq!(stable.transition, Transition::None);
    assert!(!stable.hungry);
}

#[test]
fn blocked_idle_does_not_choose_branch_or_consume_randomness() {
    let mut connection = database();
    let mut random = Random::new(&[]);
    let mut blocked = tick(120);
    blocked.idle_elapsed = Duration::ZERO;
    blocked.allow_idle_transition = false;
    let result = run(&mut connection, blocked, &mut random).unwrap();
    assert!(result.hungry);
    assert_eq!(result.snapshot.idle_elapsed_ms, 0);
    assert_eq!(result.transition, Transition::None);
    assert!(random.bounds.is_empty());
    // 即使之前已经累计到阈值，禁入期间也不能抽分支。
    connection
        .execute(
            "UPDATE session_pet_states SET idle_elapsed_ms=2700000 WHERE session_id=1",
            [],
        )
        .unwrap();
    let mut blocked = tick(0);
    blocked.allow_idle_transition = false;
    assert_eq!(
        run(&mut connection, blocked, &mut random)
            .unwrap()
            .transition,
        Transition::None
    );
}

#[test]
fn feeding_clicking_and_stroking_reward_the_same_session_atomically() {
    let mut connection = database();
    let mut random = Random::new(&[]);
    run(&mut connection, tick(20), &mut random).unwrap();
    for (interaction, expected) in [
        (Interaction::Click, 1),
        (Interaction::Stroke, 2),
        (Interaction::Feed, 3),
    ] {
        let write_scope = scope(&connection);
        let result = interact(&mut connection, 0, write_scope, interaction, now()).unwrap();
        assert_eq!(result.intimacy, expected);
        assert_eq!(result.snapshot.idle_elapsed_ms, 0);
        assert_eq!(
            result.snapshot.hunger_elapsed_ms,
            if expected == 3 { 0 } else { 20 * 60_000 }
        );
        assert_eq!(result.snapshot.last_fed_at.is_some(), expected == 3);
    }
    let unchanged: (i64, i64) = connection
        .query_row(
            "SELECT intimacy,memory_revision FROM sessions WHERE id=2",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(unchanged, (0, 0));
    assert_eq!(
        connection
            .query_row(
                "SELECT memory_revision FROM sessions WHERE id=1",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn legitimate_interactions_wake_sleep_but_plain_wake_does_not_reward() {
    for interaction in [
        Interaction::Click,
        Interaction::Stroke,
        Interaction::Feed,
        Interaction::Wake,
    ] {
        let mut connection = database();
        run(&mut connection, tick(45), &mut Random::new(&[1, 0])).unwrap();
        let write_scope = scope(&connection);
        let result = interact(&mut connection, 0, write_scope, interaction, now()).unwrap();
        assert_eq!(result.snapshot.idle_branch, IdleBranch::None);
        assert_eq!(result.snapshot.sleep_until, None);
        assert_eq!(result.snapshot.idle_elapsed_ms, 0);
        assert_eq!(
            result.intimacy,
            if matches!(interaction, Interaction::Wake) {
                0
            } else {
                1
            }
        );
    }
}

#[test]
fn stale_interaction_cannot_duplicate_intimacy_reward() {
    let mut connection = database();
    let original = scope(&connection);
    let first = interact(&mut connection, 0, original, Interaction::Feed, now()).unwrap();
    assert_eq!(
        interact(&mut connection, 0, original, Interaction::Feed, now())
            .unwrap_err()
            .code,
        "CONTEXT_CHANGED"
    );
    assert_eq!(pet_store::read(&connection, 1).unwrap(), first.snapshot);
    assert_eq!(
        connection
            .query_row("SELECT intimacy FROM sessions WHERE id=1", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn failed_reward_rolls_back_state_and_invalid_stored_values_are_rejected() {
    for use_trigger in [false, true] {
        let mut connection = database();
        run(&mut connection, tick(20), &mut Random::new(&[])).unwrap();
        if use_trigger {
            connection.execute_batch("CREATE TRIGGER reject_intimacy BEFORE UPDATE OF intimacy ON sessions BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        } else {
            connection
                .execute_batch("DROP TRIGGER sessions_intimacy_update;")
                .unwrap();
            connection
                .execute("UPDATE sessions SET intimacy=?1 WHERE id=1", [i64::MAX])
                .unwrap();
        }
        let before = pet_store::read(&connection, 1).unwrap();
        let write_scope = scope(&connection);
        assert!(interact(&mut connection, 0, write_scope, Interaction::Feed, now()).is_err());
        assert_eq!(pet_store::read(&connection, 1).unwrap(), before);
    }
}

#[test]
fn intimacy_saturates_and_feeding_still_works_at_limit() {
    let mut connection = database();
    connection
        .execute("UPDATE sessions SET intimacy=998 WHERE id=1", [])
        .unwrap();
    for interaction in [Interaction::Click, Interaction::Stroke, Interaction::Feed] {
        run(&mut connection, tick(1), &mut Random::new(&[])).unwrap();
        let write_scope = scope(&connection);
        let result = interact(&mut connection, 0, write_scope, interaction, now()).unwrap();
        assert_eq!(result.intimacy, 999);
        assert_eq!(result.snapshot.idle_elapsed_ms, 0);
        if matches!(interaction, Interaction::Feed) {
            assert_eq!(result.snapshot.hunger_elapsed_ms, 0);
            assert!(result.snapshot.last_fed_at.is_some());
        }
    }
    for invalid in [-1, 1000, i64::MAX] {
        assert!(rewarded_intimacy(invalid).is_err());
        assert!(connection
            .execute("UPDATE sessions SET intimacy=?1 WHERE id=1", [invalid])
            .is_err());
    }
    assert!(connection
        .execute("UPDATE sessions SET intimacy=1.5 WHERE id=1", [])
        .is_err());
}

#[test]
fn random_failure_and_branch_write_failure_roll_back_accumulation() {
    for mode in 0..3 {
        let mut connection = database();
        let before = pet_store::read(&connection, 1).unwrap();
        let mut random = match mode {
            0 => Random::new(&[]),
            1 => Random::new(&[2]), // 超出 [0,2) 的错误适配器结果。
            _ => Random::new(&[0]),
        };
        if mode == 2 {
            connection.execute_batch("CREATE TRIGGER reject_branch BEFORE UPDATE ON session_pet_states WHEN NEW.idle_branch!='none' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
        }
        assert!(run(&mut connection, tick(45), &mut random).is_err());
        assert_eq!(pet_store::read(&connection, 1).unwrap(), before);
    }
}

#[test]
fn away_and_changed_context_reject_ticks_and_rewards() {
    for away in [false, true] {
        let mut connection = database();
        let write_scope = scope(&connection);
        let before = pet_store::read(&connection, 1).unwrap();
        if away {
            connection.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(1,'away','a','b','c','a')", []).unwrap();
        } else {
            connection
                .execute(
                    "UPDATE settings SET value='2' WHERE key='current_session_id'",
                    [],
                )
                .unwrap();
        }
        let expected = if away { "PET_AWAY" } else { "CONTEXT_CHANGED" };
        assert_eq!(
            advance(
                &mut connection,
                0,
                write_scope,
                tick(45),
                &mut Random::new(&[])
            )
            .unwrap_err()
            .code,
            expected
        );
        assert_eq!(
            interact(&mut connection, 0, write_scope, Interaction::Feed, now())
                .unwrap_err()
                .code,
            expected
        );
        assert_eq!(pet_store::read(&connection, 1).unwrap(), before);
    }
}

#[test]
fn invalid_timing_and_invalid_settings_leave_database_unchanged() {
    let mut connection = database();
    let before = pet_store::read(&connection, 1).unwrap();
    let mut invalid = tick(1);
    invalid.allow_idle_transition = false;
    assert!(run(&mut connection, invalid, &mut Random::new(&[])).is_err());
    let mut invalid = tick(0);
    invalid.idle_elapsed = Duration::from_secs(1);
    assert!(run(&mut connection, invalid, &mut Random::new(&[])).is_err());
    setting(&connection, "sleep_min_minutes", "121");
    assert!(run(&mut connection, tick(45), &mut Random::new(&[])).is_err());
    assert_eq!(pet_store::read(&connection, 1).unwrap(), before);
}
