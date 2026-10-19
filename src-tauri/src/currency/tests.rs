use super::*;
use chrono::{FixedOffset, LocalResult, NaiveDate, NaiveDateTime};

fn time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value).unwrap().to_utc()
}
fn now() -> DateTime<Utc> {
    time("2026-10-08T03:00:00Z")
}
fn ordinary(delta: i64) -> CoinChange {
    CoinChange {
        delta,
        reason: CurrencyReason::DailySignIn,
        ref_id: None,
        plugin: None,
    }
}
fn ledger_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT count(*) FROM currency_ledger", [], |r| r.get(0))
        .unwrap()
}
fn plugin_change(delta: i64, request: &str) -> CoinChange {
    CoinChange {
        delta,
        reason: CurrencyReason::PluginGame,
        ref_id: None,
        plugin: Some(PluginLedgerContext {
            game_run_id: 1,
            plugin_id: "test.game".into(),
            plugin_version: "1.0".into(),
            display_reason: "本局结算".into(),
            request_id: request.into(),
        }),
    }
}

#[test]
fn initialized_wallet_is_global_and_reading_does_not_write() {
    let connection = crate::db::test_database();
    let before = connection.total_changes();
    let wallet = get_wallet(&connection).unwrap();
    assert_eq!(wallet.coins, "0");
    assert!(!wallet.updated_at.is_empty());
    assert_eq!(get_balance(&connection).unwrap(), 0);
    assert_eq!(connection.total_changes(), before);
    assert_eq!(ledger_count(&connection), 0);
    assert_eq!(
        connection
            .pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        14
    );
}

#[test]
fn credits_debits_and_zero_cost_record_correct_sources_and_balance() {
    let mut connection = crate::db::test_database();
    apply_delta(&mut connection, &ordinary(5), now()).unwrap();
    apply_delta(
        &mut connection,
        &CoinChange {
            delta: -4,
            reason: CurrencyReason::AccessoryCraft,
            ref_id: Some(42),
            plugin: None,
        },
        now(),
    )
    .unwrap();
    apply_delta(
        &mut connection,
        &CoinChange {
            delta: 2,
            reason: CurrencyReason::AccessorySell,
            ref_id: Some(42),
            plugin: None,
        },
        now(),
    )
    .unwrap();
    apply_delta(
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
    let applied = apply_delta(
        &mut connection,
        &CoinChange {
            delta: 0,
            reason: CurrencyReason::AccessoryCraft,
            ref_id: Some(43),
            plugin: None,
        },
        now(),
    )
    .unwrap();
    assert_eq!(applied.balance, 13);
    assert_eq!(ledger_count(&connection), 5);
    let sum: i64 = connection
        .query_row("SELECT sum(delta) FROM currency_ledger", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sum, get_balance(&connection).unwrap());
    assert_eq!(
        get_wallet(&connection).unwrap().updated_at,
        "2026-10-08T03:00:00.000Z"
    );
    let row: (String, Option<i64>, Option<i64>) = connection
        .query_row(
            "SELECT reason,ref_id,game_run_id FROM currency_ledger WHERE id=?1",
            [applied.ledger_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, ("accessory_craft".into(), Some(43), None));
}

#[test]
fn insufficient_balance_and_integer_overflow_preserve_wallet_and_ledger() {
    let mut connection = crate::db::test_database();
    let before = get_wallet(&connection).unwrap();
    let debit = CoinChange {
        delta: i64::MIN,
        reason: CurrencyReason::AccessoryCraft,
        ref_id: Some(1),
        plugin: None,
    };
    assert_eq!(
        apply_delta(&mut connection, &debit, now())
            .unwrap_err()
            .code,
        "INSUFFICIENT_COINS"
    );
    assert_eq!(get_wallet(&connection).unwrap(), before);
    connection
        .execute("UPDATE user_state SET coins=?1 WHERE id=1", [i64::MAX])
        .unwrap();
    let maximum = get_wallet(&connection).unwrap();
    assert_eq!(
        apply_delta(&mut connection, &ordinary(1), now())
            .unwrap_err()
            .code,
        "COIN_OVERFLOW"
    );
    assert_eq!(get_wallet(&connection).unwrap(), maximum);
    assert_eq!(ledger_count(&connection), 0);
    assert_eq!(
        serde_json::to_value(maximum).unwrap()["coins"],
        "9223372036854775807"
    );
}

#[test]
fn invalid_reason_sign_or_source_is_rejected_before_any_write() {
    let mut connection = crate::db::test_database();
    let before = get_wallet(&connection).unwrap();
    let changes = [
        ordinary(-1),
        CoinChange {
            delta: 9,
            reason: CurrencyReason::SignInFestival,
            ref_id: None,
            plugin: None,
        },
        CoinChange {
            delta: -1,
            reason: CurrencyReason::AccessoryCraft,
            ref_id: None,
            plugin: None,
        },
        CoinChange {
            delta: 1,
            reason: CurrencyReason::AccessoryCraft,
            ref_id: Some(1),
            plugin: None,
        },
        CoinChange {
            delta: -1,
            reason: CurrencyReason::AccessorySell,
            ref_id: Some(1),
            plugin: None,
        },
        CoinChange {
            delta: 1,
            reason: CurrencyReason::DailySignIn,
            ref_id: Some(1),
            plugin: None,
        },
        CoinChange {
            delta: 1,
            reason: CurrencyReason::PluginGame,
            ref_id: None,
            plugin: None,
        },
        plugin_change(1, " "),
    ];
    for change in changes {
        assert_eq!(
            apply_delta(&mut connection, &change, now())
                .unwrap_err()
                .code,
            "VALIDATION_ERROR"
        );
    }
    assert_eq!(get_wallet(&connection).unwrap(), before);
    assert_eq!(ledger_count(&connection), 0);
}

#[test]
fn ledger_failure_rolls_back_balance_even_when_parent_handles_error() {
    let mut connection = crate::db::test_database();
    let before = get_wallet(&connection).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_ledger BEFORE INSERT ON currency_ledger BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    let mut transaction = connection.transaction().unwrap();
    transaction
        .execute(
            "UPDATE settings SET value='dark' WHERE key='theme_mode'",
            [],
        )
        .unwrap();
    assert!(apply_delta_in_tx(&mut transaction, &ordinary(5), now()).is_err());
    transaction.commit().unwrap();
    assert_eq!(get_wallet(&connection).unwrap(), before);
    assert_eq!(ledger_count(&connection), 0);
    assert_eq!(
        connection
            .query_row(
                "SELECT value FROM settings WHERE key='theme_mode'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "dark"
    );
}

#[test]
fn wallet_failure_does_not_leave_a_ledger_record() {
    let mut connection = crate::db::test_database();
    let before = get_wallet(&connection).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_wallet BEFORE UPDATE ON user_state BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(apply_delta(&mut connection, &ordinary(5), now()).is_err());
    assert_eq!(get_wallet(&connection).unwrap(), before);
    assert_eq!(ledger_count(&connection), 0);
}

#[test]
fn outer_transaction_controls_commit_and_rolls_back_linked_business() {
    let mut connection = crate::db::test_database();
    let initial_theme: String = connection
        .query_row(
            "SELECT value FROM settings WHERE key='theme_mode'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    {
        let mut transaction = connection.transaction().unwrap();
        transaction
            .execute(
                "UPDATE settings SET value='dark' WHERE key='theme_mode'",
                [],
            )
            .unwrap();
        apply_delta_in_tx(&mut transaction, &ordinary(5), now()).unwrap();
        assert_eq!(get_balance(&transaction).unwrap(), 5);
        // 模拟关联业务失败，外层回滚，保存点不提前提交。
        transaction.rollback().unwrap();
    }
    assert_eq!(get_balance(&connection).unwrap(), 0);
    assert_eq!(ledger_count(&connection), 0);
    assert_eq!(
        connection
            .query_row(
                "SELECT value FROM settings WHERE key='theme_mode'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        initial_theme
    );
    let mut transaction = connection.transaction().unwrap();
    apply_delta_in_tx(&mut transaction, &ordinary(3), now()).unwrap();
    apply_delta_in_tx(&mut transaction, &ordinary(4), now()).unwrap();
    transaction.commit().unwrap();
    assert_eq!(get_balance(&connection).unwrap(), 7);
    assert_eq!(ledger_count(&connection), 2);
}

#[test]
fn plugin_ledger_requires_matching_run_and_duplicate_request_cannot_charge_twice() {
    let mut connection = crate::db::test_database();
    connection.execute_batch("INSERT INTO game_runs(id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,started_at)
        VALUES(1,'test.game','测试游戏','1.0',1,'running','2026-10-08T00:00:00.000Z');").unwrap();
    let applied = apply_delta(&mut connection, &plugin_change(5, "request-1"), now()).unwrap();
    assert_eq!(applied.balance, 5);
    assert!(apply_delta(&mut connection, &plugin_change(5, "request-1"), now()).is_err());
    assert_eq!(get_balance(&connection).unwrap(), 5);
    let mut mismatch = plugin_change(5, "request-2");
    mismatch.plugin.as_mut().unwrap().plugin_version = "2.0".into();
    assert!(apply_delta(&mut connection, &mismatch, now()).is_err());
    apply_delta(&mut connection, &plugin_change(0, "request-3"), now()).unwrap();
    assert_eq!(ledger_count(&connection), 2);
    assert_eq!(get_balance(&connection).unwrap(), 5);
    let row: (String, String, String) = connection.query_row(
        "SELECT plugin_id,plugin_display_reason,wallet_request_id FROM currency_ledger WHERE id=?1", [applied.ledger_id],
        |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        row,
        ("test.game".into(), "本局结算".into(), "request-1".into())
    );
}

#[test]
fn local_today_projects_utc_records_and_recomputes_after_timezone_change() {
    let mut connection = crate::db::test_database();
    for at in [
        "2026-10-07T15:59:59.999Z",
        "2026-10-07T16:00:00Z",
        "2026-10-08T15:59:59.999Z",
        "2026-10-08T16:00:00Z",
    ] {
        apply_delta(&mut connection, &ordinary(1), time(at)).unwrap();
    }
    apply_delta(
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
    let east = FixedOffset::east_opt(8 * 3600).unwrap();
    assert_eq!(
        count_today_reason(&connection, CurrencyReason::DailySignIn, now(), &east).unwrap(),
        2
    );
    assert_eq!(
        count_today_reason(&connection, CurrencyReason::SignInFestival, now(), &east).unwrap(),
        1
    );
    assert_eq!(
        count_today_reason(
            &connection,
            CurrencyReason::DailySignIn,
            time("2026-10-08T16:00:00Z"),
            &east
        )
        .unwrap(),
        1
    );
    assert_eq!(
        count_today_reason(
            &connection,
            CurrencyReason::DailySignIn,
            time("2026-10-08T16:00:00Z"),
            &Utc
        )
        .unwrap(),
        2
    );
    assert_eq!(
        count_today_reason(
            &connection,
            CurrencyReason::DailySignIn,
            now(),
            &FixedOffset::west_opt(5 * 3600).unwrap()
        )
        .unwrap(),
        2
    );
}

// UTC 转本地即可统计；故意禁止本地时刻反推，验证夏令时不依赖固定一天长度。
#[derive(Clone)]
struct SeasonalZone;
impl TimeZone for SeasonalZone {
    type Offset = FixedOffset;
    fn from_offset(_: &Self::Offset) -> Self {
        Self
    }
    fn offset_from_local_date(&self, _: &NaiveDate) -> LocalResult<FixedOffset> {
        unreachable!()
    }
    fn offset_from_local_datetime(&self, _: &NaiveDateTime) -> LocalResult<FixedOffset> {
        unreachable!()
    }
    fn offset_from_utc_date(&self, date: &NaiveDate) -> FixedOffset {
        self.offset_from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        let summer = *utc >= time("2026-03-29T01:00:00Z").naive_utc()
            && *utc < time("2026-10-25T01:00:00Z").naive_utc();
        FixedOffset::east_opt(if summer { 7200 } else { 3600 }).unwrap()
    }
}

#[test]
fn daylight_saving_counts_23_and_25_hour_dates_without_losing_repeated_hour() {
    let mut connection = crate::db::test_database();
    for at in [
        "2026-03-28T22:59:59.999Z",
        "2026-03-28T23:00:00Z",
        "2026-03-29T01:30:00Z",
        "2026-03-29T21:59:59.999Z",
        "2026-03-29T22:00:00Z",
        "2026-10-24T21:59:59.999Z",
        "2026-10-24T22:00:00Z",
        "2026-10-25T00:30:00Z",
        "2026-10-25T01:30:00Z",
        "2026-10-25T22:59:59.999Z",
        "2026-10-25T23:00:00Z",
    ] {
        apply_delta(&mut connection, &ordinary(1), time(at)).unwrap();
    }
    assert_eq!(
        count_today_reason(
            &connection,
            CurrencyReason::DailySignIn,
            time("2026-03-29T12:00:00Z"),
            &SeasonalZone
        )
        .unwrap(),
        3
    );
    assert_eq!(
        count_today_reason(
            &connection,
            CurrencyReason::DailySignIn,
            time("2026-10-25T12:00:00Z"),
            &SeasonalZone
        )
        .unwrap(),
        4
    );
}

#[test]
fn committed_wallet_and_ledger_survive_disk_reopen() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("yanmew-wallet-{}-{unique}.db", std::process::id()));
    {
        let connection = crate::db::test_database();
        connection
            .execute("VACUUM INTO ?1", [path.to_str().unwrap()])
            .unwrap();
    }
    let expected = {
        let mut connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        apply_delta(&mut connection, &ordinary(8), now()).unwrap();
        get_wallet(&connection).unwrap()
    };
    {
        let connection = Connection::open(&path).unwrap();
        assert_eq!(get_wallet(&connection).unwrap(), expected);
        assert_eq!(ledger_count(&connection), 1);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn current_intimacy_query_checks_persona_session_and_revision_without_rewards() {
    let runtime = crate::runtime::test_runtime("https://example.com/v1");
    runtime
        .read(|store, _| {
            let persona: i64 = store.connection.query_row(
                "SELECT id FROM personas WHERE is_active=1",
                [],
                |r| r.get(0),
            )?;
            let first = crate::sessions::create(&mut store.connection, persona)?;
            let second = crate::sessions::create(&mut store.connection, persona)?;
            store
                .connection
                .execute("UPDATE sessions SET intimacy=999 WHERE id=?1", [&second.id])?;
            let first_id: i64 = first.id.parse().unwrap();
            let second_id: i64 = second.id.parse().unwrap();
            let revision = store.context_revision;
            let view =
                crate::commands::query_session_intimacy(store, persona, second_id, revision)?;
            assert_eq!(view.intimacy, 999);
            assert_eq!(view.maximum, 999);
            assert_eq!(view.session_id, second.id);
            assert_eq!(
                crate::commands::query_session_intimacy(store, persona, first_id, revision)
                    .unwrap_err()
                    .code,
                "CONTEXT_CHANGED"
            );
            assert_eq!(
                crate::commands::query_session_intimacy(store, persona, second_id, revision + 1)
                    .unwrap_err()
                    .code,
                "CONTEXT_CHANGED"
            );
            assert_eq!(
                crate::commands::query_session_intimacy(store, persona + 1, second_id, revision)
                    .unwrap_err()
                    .code,
                "CONTEXT_CHANGED"
            );
            assert!(
                get_session_intimacy(&store.connection, persona + 1, second_id, revision).is_err()
            );
            assert_eq!(get_balance(&store.connection)?, 0);
            assert_eq!(ledger_count(&store.connection), 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn missing_wallet_is_reported_instead_of_creating_a_new_balance() {
    let mut connection = crate::db::test_database();
    connection.execute("DELETE FROM user_state", []).unwrap();
    assert_eq!(
        get_wallet(&connection).unwrap_err().code,
        "WALLET_UNAVAILABLE"
    );
    assert_eq!(
        apply_delta(&mut connection, &ordinary(5), now())
            .unwrap_err()
            .code,
        "WALLET_UNAVAILABLE"
    );
    assert_eq!(ledger_count(&connection), 0);
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM user_state", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
