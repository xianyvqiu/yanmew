use super::*;

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-20T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn database() -> Connection {
    let connection = crate::db::test_database();
    seed_sessions(&connection);
    connection
}

fn seed_sessions(connection: &Connection) {
    for session_id in [1, 2] {
        connection.execute(
            "INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(?1,1,'测试',?2,?2)",
            params![session_id,time_text(now())],
        ).unwrap();
    }
    connection
        .execute("INSERT INTO settings VALUES('current_session_id','1')", [])
        .unwrap();
}

fn scope(snapshot: &PetSnapshot) -> PetWriteScope {
    PetWriteScope {
        persona_id: 1,
        session_id: snapshot.session_id.parse().unwrap(),
        context_revision: 0,
        state_revision: snapshot.state_revision,
    }
}

#[test]
fn interaction_and_feeding_have_independent_clocks() {
    let mut connection = database();
    let initial = read(&connection, 1).unwrap();
    let elapsed = Duration::from_secs(44 * 60);
    let accumulated =
        checkpoint(&mut connection, 0, scope(&initial), elapsed, elapsed, now()).unwrap();
    assert_eq!(accumulated.idle_elapsed_ms, 2_640_000);
    let clicked = interact(
        &mut connection,
        0,
        scope(&accumulated),
        Interaction::Click,
        now(),
    )
    .unwrap();
    assert_eq!(clicked.hunger_elapsed_ms, 2_640_000);
    assert_eq!(clicked.idle_elapsed_ms, 0);
    assert!(clicked.last_fed_at.is_none());
    let fed = interact(
        &mut connection,
        0,
        scope(&clicked),
        Interaction::Feed,
        now(),
    )
    .unwrap();
    assert_eq!(fed.hunger_elapsed_ms, 0);
    assert_eq!(fed.last_fed_at, Some(time_text(now())));
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
fn checkpoint_and_branch_share_one_atomic_transaction() {
    let mut connection = database();
    let initial = read(&connection, 1).unwrap();
    let duration = Duration::from_secs(2700);
    let deadline = now() + chrono::Duration::hours(1);
    {
        let transaction = connection.transaction().unwrap();
        let accumulated =
            checkpoint_in_transaction(&transaction, 0, scope(&initial), duration, duration, now())
                .unwrap();
        choose_idle_branch_in_transaction(
            &transaction,
            0,
            scope(&accumulated),
            IdleBranch::Sleeping,
            Some(deadline),
            now(),
        )
        .unwrap();
        // 调用方后续失败，整个区间和分支都回滚，不能留下已经抽取的半套状态。
    }
    assert_eq!(read(&connection, 1).unwrap(), initial);
    {
        let transaction = connection.transaction().unwrap();
        let accumulated =
            checkpoint_in_transaction(&transaction, 0, scope(&initial), duration, duration, now())
                .unwrap();
        choose_idle_branch_in_transaction(
            &transaction,
            0,
            scope(&accumulated),
            IdleBranch::Sleeping,
            Some(deadline),
            now(),
        )
        .unwrap();
        transaction.commit().unwrap();
    }
    let sleeping = read(&connection, 1).unwrap();
    assert_eq!(sleeping.hunger_elapsed_ms, 2_700_000);
    assert_eq!(sleeping.sleep_until, Some(time_text(deadline)));
}

#[test]
fn stale_repeated_and_noncurrent_checkpoints_are_rejected() {
    let mut connection = database();
    let initial = read(&connection, 1).unwrap();
    let updated = checkpoint(
        &mut connection,
        0,
        scope(&initial),
        Duration::from_secs(1),
        Duration::from_secs(1),
        now(),
    )
    .unwrap();
    assert!(checkpoint(
        &mut connection,
        0,
        scope(&initial),
        Duration::from_secs(1),
        Duration::ZERO,
        now()
    )
    .is_err());
    assert!(checkpoint(
        &mut connection,
        1,
        scope(&updated),
        Duration::from_secs(1),
        Duration::ZERO,
        now()
    )
    .is_err());
    connection
        .execute(
            "UPDATE settings SET value='2' WHERE key='current_session_id'",
            [],
        )
        .unwrap();
    assert!(checkpoint(
        &mut connection,
        0,
        scope(&updated),
        Duration::from_secs(1),
        Duration::ZERO,
        now()
    )
    .is_err());
    assert_eq!(read(&connection, 1).unwrap(), updated);
    assert_eq!(read(&connection, 2).unwrap().hunger_elapsed_ms, 0);
    connection
        .execute(
            "UPDATE settings SET value='1' WHERE key='current_session_id'",
            [],
        )
        .unwrap();
    assert_eq!(read(&connection, 1).unwrap(), updated);
}

#[test]
fn sleep_deadline_survives_noncurrent_restore_and_expires_once() {
    let mut connection = database();
    let initial = read(&connection, 1).unwrap();
    let updated = checkpoint(
        &mut connection,
        0,
        scope(&initial),
        Duration::from_secs(2700),
        Duration::from_secs(2700),
        now(),
    )
    .unwrap();
    let deadline = now() + chrono::Duration::hours(1);
    let sleeping = choose_idle_branch(
        &mut connection,
        0,
        scope(&updated),
        IdleBranch::Sleeping,
        Some(deadline),
        now(),
    )
    .unwrap();
    assert!(choose_idle_branch(
        &mut connection,
        0,
        scope(&sleeping),
        IdleBranch::Bored,
        None,
        now()
    )
    .is_err());
    connection
        .execute(
            "UPDATE settings SET value='2' WHERE key='current_session_id'",
            [],
        )
        .unwrap();
    assert_eq!(restore_sleep(&mut connection, 1, now()).unwrap(), sleeping);
    let awake = restore_sleep(&mut connection, 1, deadline).unwrap();
    assert_eq!(awake.idle_branch, IdleBranch::None);
    assert_eq!(awake.idle_elapsed_ms, 0);
    assert_eq!(awake.hunger_elapsed_ms, sleeping.hunger_elapsed_ms);
    assert_eq!(awake.last_interacted_at, sleeping.last_interacted_at);
    assert_eq!(awake.state_revision, sleeping.state_revision + 1);
    assert_eq!(
        restore_sleep(&mut connection, 1, deadline + chrono::Duration::days(1)).unwrap(),
        awake
    );
}

#[test]
fn wake_and_away_guards_preserve_consistency() {
    let mut connection = database();
    let initial = read(&connection, 1).unwrap();
    let sleeping = choose_idle_branch(
        &mut connection,
        0,
        scope(&initial),
        IdleBranch::Sleeping,
        Some(now() + chrono::Duration::hours(1)),
        now(),
    )
    .unwrap();
    assert!(checkpoint(
        &mut connection,
        0,
        scope(&sleeping),
        Duration::from_secs(1),
        Duration::from_secs(1),
        now()
    )
    .is_err());
    let awake = interact(
        &mut connection,
        0,
        scope(&sleeping),
        Interaction::Wake,
        now(),
    )
    .unwrap();
    assert_eq!(awake.sleep_until, None);
    connection.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(1,'away',?1,?1,?1,?1)", [time_text(now())]).unwrap();
    assert!(interact(&mut connection, 0, scope(&awake), Interaction::Feed, now()).is_err());
    assert!(choose_idle_branch(
        &mut connection,
        0,
        scope(&awake),
        IdleBranch::Bored,
        None,
        now()
    )
    .is_err());
    assert_eq!(read(&connection, 1).unwrap(), awake);
}

#[test]
fn invalid_durations_and_failed_transactions_do_not_change_state() {
    let mut connection = database();
    let initial = read(&connection, 1).unwrap();
    assert!(checkpoint(
        &mut connection,
        0,
        scope(&initial),
        Duration::ZERO,
        Duration::from_secs(1),
        now()
    )
    .is_err());
    assert!(checkpoint(
        &mut connection,
        0,
        scope(&initial),
        Duration::MAX,
        Duration::ZERO,
        now()
    )
    .is_err());
    assert!(choose_idle_branch(
        &mut connection,
        0,
        scope(&initial),
        IdleBranch::Sleeping,
        Some(now()),
        now()
    )
    .is_err());
    {
        let transaction = connection.transaction().unwrap();
        interact_in_transaction(&transaction, 0, scope(&initial), Interaction::Feed, now())
            .unwrap();
        // 模拟组合事务的后续亲密度写入失败；丢弃事务必须回滚之前的互动。
        assert!(transaction
            .execute("UPDATE sessions SET intimacy=-1 WHERE id=1", [])
            .is_err());
    }
    assert_eq!(read(&connection, 1).unwrap(), initial);
    connection.execute_batch("CREATE TRIGGER reject_pet BEFORE UPDATE ON session_pet_states BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(interact(
        &mut connection,
        0,
        scope(&initial),
        Interaction::Feed,
        now()
    )
    .is_err());
    assert_eq!(read(&connection, 1).unwrap(), initial);
}

#[test]
fn disk_reopen_keeps_elapsed_branch_and_sleep_deadline() {
    let root = std::env::temp_dir().join(format!(
        "yanmew-pet-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let paths = crate::platform::contracts::AppPaths {
        data_root: root.clone(),
    };
    let sleeping;
    {
        let (mut connection, _) = crate::db::open(&paths).unwrap();
        // 本例验证自建会话状态，移除新库的教程会话再准备固定 ID。
        connection.execute_batch("DELETE FROM settings WHERE key='current_session_id'; DELETE FROM sessions;").unwrap();
        seed_sessions(&connection);
        let initial = read(&connection, 1).unwrap();
        let updated = checkpoint(
            &mut connection,
            0,
            scope(&initial),
            Duration::from_secs(2700),
            Duration::from_secs(2700),
            now(),
        )
        .unwrap();
        sleeping = choose_idle_branch(
            &mut connection,
            0,
            scope(&updated),
            IdleBranch::Sleeping,
            Some(now() + chrono::Duration::hours(1)),
            now(),
        )
        .unwrap();
    }
    {
        let (mut connection, _) = crate::db::open(&paths).unwrap();
        assert_eq!(read(&connection, 1).unwrap(), sleeping);
        let awake = restore_sleep(&mut connection, 1, now() + chrono::Duration::days(7)).unwrap();
        assert_eq!(awake.hunger_elapsed_ms, sleeping.hunger_elapsed_ms);
        assert_eq!(awake.idle_elapsed_ms, 0);
        connection
            .execute("DELETE FROM sessions WHERE id=1", [])
            .unwrap();
        assert!(read(&connection, 1).is_err());
        assert!(read(&connection, 2).is_ok());
    }
    std::fs::remove_file(root.join("yanwenzi.db")).unwrap();
    std::fs::remove_dir(root).unwrap();
}
