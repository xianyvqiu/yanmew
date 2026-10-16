use super::*;
use chrono::FixedOffset;
use rusqlite::params;

struct Draw {
    value: u64,
    calls: usize,
    upper: u64,
}
impl RandomSource for Draw {
    fn below(&mut self, upper: u64) -> AppResult<u64> {
        self.calls += 1;
        self.upper = upper;
        Ok(self.value)
    }
}
fn draw(value: u64) -> Draw {
    Draw {
        value,
        calls: 0,
        upper: 0,
    }
}
fn time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value).unwrap().to_utc()
}
fn now() -> DateTime<Utc> {
    time("2026-10-08T03:00:00Z")
}
fn ledger_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT count(*) FROM currency_ledger", [], |row| row.get(0))
        .unwrap()
}
fn range(connection: &mut Connection, min: i64, max: i64) {
    config::save(
        connection,
        [
            ("sign_in_min_coins".into(), min.to_string()),
            ("sign_in_max_coins".into(), max.to_string()),
        ]
        .into(),
    )
    .unwrap();
}

#[test]
fn read_status_has_no_side_effect_and_needs_no_session_or_provider() {
    let connection = crate::db::test_database();
    let changes = connection.total_changes();
    let status = get_sign_in_status(&connection, now(), &Utc).unwrap();
    assert_eq!(status.local_date, "2026-10-08");
    assert!(!status.signed_in);
    assert_eq!(
        (status.reward_min.as_str(), status.reward_max.as_str()),
        ("1", "10")
    );
    assert_eq!(status.intimacy_thresholds, [10, 50, 150, 300]);
    assert_eq!(status.wallet.coins, "0");
    assert_eq!(connection.total_changes(), changes);
}

#[test]
fn reward_includes_both_endpoints_and_duplicate_does_not_draw_or_write() {
    for (offset, reward) in [(0, "1"), (9, "10")] {
        let mut connection = crate::db::test_database();
        let mut random = draw(offset);
        let result = daily_sign_in(&mut connection, now(), &Utc, &mut random).unwrap();
        assert_eq!(result.granted_coins.as_deref(), Some(reward));
        assert_eq!(result.status.wallet.coins, reward);
        assert!(result.status.signed_in);
        assert_eq!(random.upper, 10);
        let changes = connection.total_changes();
        let repeat = daily_sign_in(&mut connection, now(), &Utc, &mut random).unwrap();
        assert_eq!(repeat.granted_coins, None);
        assert_eq!(repeat.status, result.status);
        assert_eq!(random.calls, 1);
        assert_eq!(connection.total_changes(), changes);
        assert_eq!(ledger_count(&connection), 1);
    }
}

#[test]
fn configured_rewards_and_zero_reward_are_persisted_as_sign_in() {
    for (min, max, offset, expected) in [(30, 32, 2, "32"), (0, 0, 999, "0")] {
        let mut connection = crate::db::test_database();
        range(&mut connection, min, max);
        let mut random = draw(offset);
        let result = daily_sign_in(&mut connection, now(), &Utc, &mut random).unwrap();
        assert_eq!(result.granted_coins.as_deref(), Some(expected));
        assert_eq!(random.calls, usize::from(min != max));
        assert!(
            get_sign_in_status(&connection, now(), &Utc)
                .unwrap()
                .signed_in
        );
        assert_eq!(ledger_count(&connection), 1);
    }
}

#[test]
fn festival_ledger_blocks_ordinary_sign_in() {
    let mut connection = crate::db::test_database();
    super::super::apply_delta(
        &mut connection,
        &CoinChange {
            delta: 10,
            reason: CurrencyReason::SignInFestival,
            ref_id: None,
            plugin: None,
        },
        now(),
    )
    .unwrap();
    let mut random = draw(0);
    let result = daily_sign_in(&mut connection, now(), &Utc, &mut random).unwrap();
    assert_eq!(result.granted_coins, None);
    assert_eq!(result.status.wallet.coins, "10");
    assert_eq!(random.calls, 0);
    assert_eq!(ledger_count(&connection), 1);
}

#[test]
fn midnight_and_current_timezone_are_authoritative_and_status_never_awards() {
    let mut connection = crate::db::test_database();
    range(&mut connection, 2, 2);
    let east = FixedOffset::east_opt(8 * 3600).unwrap();
    let mut random = draw(0);
    daily_sign_in(
        &mut connection,
        time("2026-10-08T15:59:59Z"),
        &east,
        &mut random,
    )
    .unwrap();
    let at = time("2026-10-08T16:00:00Z");
    let next = get_sign_in_status(&connection, at, &east).unwrap();
    assert_eq!(next.local_date, "2026-10-09");
    assert!(!next.signed_in);
    // 同一时刻切换电脑时区，今日应重新按当前时区投影历史流水。
    assert!(get_sign_in_status(&connection, at, &Utc).unwrap().signed_in);
    assert_eq!(ledger_count(&connection), 1);
    assert_eq!(
        daily_sign_in(&mut connection, at, &east, &mut random)
            .unwrap()
            .status
            .wallet
            .coins,
        "4"
    );
    assert_eq!(ledger_count(&connection), 2);
}

#[test]
fn ledger_failure_rolls_back_balance_and_allows_retry() {
    let mut connection = crate::db::test_database();
    range(&mut connection, 4, 4);
    connection.execute_batch("CREATE TRIGGER fail_sign_in BEFORE INSERT ON currency_ledger BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    let before = get_wallet(&connection).unwrap();
    assert!(daily_sign_in(&mut connection, now(), &Utc, &mut draw(0)).is_err());
    assert_eq!(get_wallet(&connection).unwrap(), before);
    assert!(
        !get_sign_in_status(&connection, now(), &Utc)
            .unwrap()
            .signed_in
    );
    connection
        .execute_batch("DROP TRIGGER fail_sign_in")
        .unwrap();
    assert_eq!(
        daily_sign_in(&mut connection, now(), &Utc, &mut draw(0))
            .unwrap()
            .granted_coins
            .as_deref(),
        Some("4")
    );
}

#[test]
fn overflow_invalid_config_and_invalid_random_do_not_consume_today() {
    let mut connection = crate::db::test_database();
    connection
        .execute("UPDATE user_state SET coins=?1 WHERE id=1", [i64::MAX])
        .unwrap();
    assert_eq!(
        daily_sign_in(&mut connection, now(), &Utc, &mut draw(0))
            .unwrap_err()
            .code,
        "COIN_OVERFLOW"
    );
    connection
        .execute("UPDATE user_state SET coins=0 WHERE id=1", [])
        .unwrap();
    assert_eq!(
        daily_sign_in(&mut connection, now(), &Utc, &mut draw(10))
            .unwrap_err()
            .code,
        "RANDOM_SOURCE_ERROR"
    );
    connection
        .execute("UPDATE settings SET value='11' WHERE key='sign_in_min_coins'", [])
        .unwrap();
    let mut random = draw(0);
    assert!(daily_sign_in(&mut connection, now(), &Utc, &mut random).is_err());
    assert_eq!(random.calls, 0);
    assert_eq!(ledger_count(&connection), 0);
    assert_eq!(get_wallet(&connection).unwrap().coins, "0");
}

#[test]
fn concurrent_connections_award_once_and_reopen_keeps_global_status() {
    let path = std::env::temp_dir().join(format!(
        "yanmew-sign-in-{}-{}.db",
        std::process::id(),
        now().timestamp_nanos_opt().unwrap()
    ));
    let mut connection = Connection::open(&path).unwrap();
    crate::db::initialize(&mut connection).unwrap();
    range(&mut connection, 3, 3);
    drop(connection);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut connection = Connection::open(path).unwrap();
                connection
                    .busy_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                barrier.wait();
                daily_sign_in(&mut connection, now(), &Utc, &mut draw(0))
                    .unwrap()
                    .granted_coins
            })
        })
        .collect();
    assert_eq!(
        workers
            .into_iter()
            .filter_map(|worker| worker.join().unwrap())
            .count(),
        1
    );
    let connection = Connection::open(&path).unwrap();
    let status = get_sign_in_status(&connection, now(), &Utc).unwrap();
    assert!(status.signed_in);
    assert_eq!(status.wallet.coins, "3");
    assert_eq!(ledger_count(&connection), 1);
    let snapshot: (i64, Option<i64>, Option<String>) = connection
        .query_row(
            "SELECT delta,ref_id,plugin_id FROM currency_ledger WHERE reason=?1",
            params!["daily_sign_in"],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(snapshot, (3, None, None));
    drop(connection);
    std::fs::remove_file(path).unwrap();
}

fn fixed_date(c: &mut Connection, _session: i64, month: u32, day: u32, calendar: &str) {
    crate::festivals::custom::create(c, crate::festivals::custom::Input {
        kind: "birthday".into(), name: "生日".into(), calendar_type: calendar.into(),
        month, day, is_leap_month: false, blessing: "生日快乐".into(), note: None,
    }, "user", now().date_naive(), now()).unwrap();
}

#[test]
fn combined_festivals_award_ten_once_without_random_and_without_session() {
    let mut c = crate::db::test_database();
    let at = time("2024-01-01T03:00:00Z");
    let session = crate::sessions::create(&mut c, 1).unwrap().id.parse().unwrap();
    fixed_date(&mut c, session, 1, 1, "solar");
    range(&mut c, 30, 32);
    let mut random = draw(999);
    let result = daily_sign_in(&mut c, at, &Utc, &mut random).unwrap();
    assert_eq!(result.granted_coins.as_deref(), Some("10"));
    assert_eq!(result.status.festivals.len(), 2);
    assert_eq!(random.calls, 0);
    assert_eq!(ledger_count(&c), 1);
    assert_eq!(c.query_row("SELECT reason FROM currency_ledger", [], |r| r.get::<_, String>(0)).unwrap(), "sign_in_festival");
    // 改会话或删掉日期仍不能重新领取；状态查询不修改日期缓存。
    c.execute("DELETE FROM settings WHERE key='current_session_id'", []).unwrap();
    let changes = c.total_changes();
    let status = get_sign_in_status(&c, at, &Utc).unwrap();
    assert_eq!(status.festivals.len(), 2);
    assert_eq!(c.total_changes(), changes);
    assert!(daily_sign_in(&mut c, at, &Utc, &mut random).unwrap().granted_coins.is_none());
}

#[test]
fn global_dates_remain_visible_across_sessions_and_both_lunar_occurrences_receive_festival_reward() {
    let mut c = crate::db::test_database();
    let first = crate::sessions::create(&mut c, 1).unwrap().id.parse().unwrap();
    fixed_date(&mut c, first, 11, 20, "lunar");
    let mut random = draw(0);
    let at = time("2024-12-20T03:00:00Z");
    assert_eq!(daily_sign_in(&mut c, at, &Utc, &mut random).unwrap().granted_coins.as_deref(), Some("10"));
    // 第二次命中也有效，不仅依赖缓存中的首次公历日期。
    assert_eq!(random.calls, 0);
    let second = crate::sessions::create(&mut c, 1).unwrap();
    assert_ne!(second.id, first.to_string());
    assert!(!get_sign_in_status(&c, at, &Utc).unwrap().festivals.is_empty());
    assert!(get_sign_in_status(&c, at, &Utc).unwrap().signed_in);
}

#[test]
fn memorial_blessing_and_festival_ledger_failure_do_not_consume_sign_in() {
    let mut c = crate::db::test_database();
    let date = crate::lunar::solar_term_date(2024, "清明").unwrap();
    let at = date.and_hms_opt(3, 0, 0).unwrap().and_utc();
    let status = get_sign_in_status(&c, at, &Utc).unwrap();
    assert!(status.festivals.iter().any(|f| f.is_memorial && !f.blessing.contains("快乐")));
    c.execute_batch("CREATE TRIGGER reject_festival BEFORE INSERT ON currency_ledger BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
    assert!(daily_sign_in(&mut c, at, &Utc, &mut draw(0)).is_err());
    assert_eq!(get_wallet(&c).unwrap().coins, "0");
    assert!(!get_sign_in_status(&c, at, &Utc).unwrap().signed_in);
    c.execute_batch("DROP TRIGGER reject_festival").unwrap();
    assert_eq!(daily_sign_in(&mut c, at, &Utc, &mut draw(0)).unwrap().granted_coins.as_deref(), Some("10"));
}
