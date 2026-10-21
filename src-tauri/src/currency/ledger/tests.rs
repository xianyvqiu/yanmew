use super::*;
use chrono::FixedOffset;
#[test]
fn ledger_game_history_survives_without_installed_plugin_and_filters_by_snapshot() {
    let mut c = crate::db::test_database();
    c.execute("INSERT INTO game_runs(id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,started_at) VALUES(1,'old.game','历史小游戏','1.0.0',1,'running','2026-10-20T00:00:00Z')",[]).unwrap();
    let change = super::super::CoinChange {
        delta: 9,
        reason: crate::models::CurrencyReason::PluginGame,
        ref_id: None,
        plugin: Some(super::super::PluginLedgerContext {
            game_run_id: 1,
            plugin_id: "old.game".into(),
            plugin_version: "1.0.0".into(),
            display_reason: "完成奖励".into(),
            request_id: "history-request".into(),
        }),
    };
    super::super::apply_delta(
        &mut c,
        &change,
        DateTime::parse_from_rfc3339("2026-10-20T01:00:00Z")
            .unwrap()
            .to_utc(),
    )
    .unwrap();
    let mut q = query();
    q.plugin_id = "old.game".into();
    q.search = "历史小游戏".into();
    let r = report(&c, q);
    assert_eq!(r.entries[0].plugin_name.as_deref(), Some("历史小游戏"));
    assert_eq!(r.plugins[0].id, "old.game");
    assert_eq!(r.games[0].summary.income, "9");
    assert_eq!(r.balance, "9");
}
fn query() -> Query {
    Query {
        start: None,
        end: None,
        direction: "all".into(),
        reason: String::new(),
        plugin_id: String::new(),
        search: String::new(),
        ascending: false,
        page: 1,
    }
}
fn add(c: &Connection, delta: i64, reason: &str, stamp: &str) {
    c.execute(
        "INSERT INTO currency_ledger(delta,reason,created_at) VALUES(?1,?2,?3)",
        params![delta, reason, stamp],
    )
    .unwrap();
}
fn report(c: &Connection, q: Query) -> Report {
    let zone = FixedOffset::east_opt(8 * 3600).unwrap();
    read_with_date(
        c,
        q,
        |t| t.with_timezone(&zone).date_naive(),
        NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
    )
    .unwrap()
}
#[test]
fn ledger_pagination_and_filters_keep_exact_global_totals_without_writes() {
    let c = crate::db::test_database();
    for _ in 0..14 {
        add(&c, 5, "daily_sign_in", "2026-10-20T00:00:00.000Z");
    }
    add(&c, -15, "food_purchase", "2026-10-20T01:00:00.000Z");
    add(&c, 0, "daily_sign_in", "2026-10-20T01:01:00.000Z");
    let before = c.total_changes();
    let first = report(&c, query());
    assert_eq!(first.entries.len(), 12);
    assert_eq!(first.total_pages, 2);
    assert_eq!(first.summary.income, "70");
    assert_eq!(first.summary.expense, "15");
    assert_eq!(first.summary.net, "55");
    assert_eq!(first.summary.count, 16);
    let mut q = query();
    q.page = 2;
    let second = report(&c, q);
    assert_eq!(second.entries.len(), 4);
    assert!(!second
        .entries
        .iter()
        .any(|v| first.entries.iter().any(|a| a.id == v.id)));
    let mut q = query();
    q.search = "食物购买".into();
    let filtered = report(&c, q);
    assert_eq!(filtered.summary.count, 1);
    assert_eq!(filtered.summary.net, "-15");
    let mut q = query();
    q.direction = "income".into();
    q.ascending = true;
    let filtered = report(&c, q);
    assert_eq!(filtered.summary.count, 14);
    assert_eq!(filtered.entries[0].id, "1");
    assert_eq!(c.total_changes(), before);
    assert_eq!(super::super::get_wallet(&c).unwrap().coins, "0");
}
#[test]
fn ledger_dates_are_local_inclusive_and_empty_days_are_present() {
    let c = crate::db::test_database();
    add(&c, 3, "daily_sign_in", "2026-10-18T15:59:59.000Z");
    add(&c, 7, "daily_sign_in", "2026-10-18T16:00:00.000Z");
    add(&c, 11, "daily_sign_in", "2026-10-19T16:00:00.000Z");
    let mut q = query();
    q.start = Some("2026-10-19".into());
    q.end = q.start.clone();
    let r = report(&c, q);
    assert_eq!(r.summary.income, "7");
    assert_eq!(r.timeline.len(), 1);
    let mut q = query();
    q.start = Some("2026-10-17".into());
    q.end = Some("2026-10-20".into());
    let r = report(&c, q);
    assert_eq!(r.timeline.len(), 4);
    assert_eq!(r.timeline[0].summary.count, 0);
}
#[test]
fn ledger_sums_exceeding_i64_and_negative_minimum_stay_exact() {
    let c = crate::db::test_database();
    add(&c, i64::MAX, "daily_sign_in", "2026-10-20T00:00:00Z");
    add(&c, i64::MAX, "daily_sign_in", "2026-10-20T00:00:00Z");
    add(&c, i64::MIN, "food_purchase", "2026-10-20T00:00:00Z");
    let r = report(&c, query());
    assert_eq!(r.summary.income, "18446744073709551614");
    assert_eq!(r.summary.expense, "9223372036854775808");
    assert_eq!(r.summary.net, "9223372036854775806");
}
#[test]
fn ledger_rejects_invalid_filters_and_buckets_long_ranges() {
    let c = crate::db::test_database();
    for (start, end) in [("2026-10-20", "2026-10-19"), ("2026-02-30", "2026-10-19")] {
        let mut q = query();
        q.start = Some(start.into());
        q.end = Some(end.into());
        assert!(read(&c, q).is_err());
    }
    let mut q = query();
    q.reason = "sql'--".into();
    assert!(read(&c, q).is_err());
    let mut q = query();
    q.start = Some("2026-01-01".into());
    q.end = Some("2026-12-31".into());
    let r = report(&c, q);
    assert_eq!(r.bucket, "month");
    assert_eq!(r.timeline.len(), 12);
    assert_eq!(r.summary.count, 0);
    let mut q = query();
    q.start = Some("2026-10-01".into());
    q.end = Some("2026-10-31".into());
    let r = report(&c, q.clone());
    assert_eq!(r.bucket, "day");
    assert_eq!(r.timeline.len(), 31);
    q.end = Some("2026-11-01".into());
    assert_eq!(report(&c, q).bucket, "week");
}
#[test]
fn ledger_fall_back_day_includes_both_repeated_hours_without_fixed_day_length() {
    let c = crate::db::test_database();
    for time in [
        "2026-11-01T03:59:59Z",
        "2026-11-01T04:00:00Z",
        "2026-11-01T05:30:00Z",
        "2026-11-01T06:30:00Z",
        "2026-11-02T04:59:59Z",
        "2026-11-02T05:00:00Z",
    ] {
        add(&c, 1, "daily_sign_in", time);
    }
    let mut q = query();
    q.start = Some("2026-11-01".into());
    q.end = q.start.clone();
    let transition = DateTime::parse_from_rfc3339("2026-11-01T06:00:00Z")
        .unwrap()
        .to_utc();
    let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    let r = read_with_date(
        &c,
        q,
        |instant| {
            instant
                .with_timezone(
                    &FixedOffset::west_opt(if instant < transition {
                        4 * 3600
                    } else {
                        5 * 3600
                    })
                    .unwrap(),
                )
                .date_naive()
        },
        day,
    )
    .unwrap();
    assert_eq!(r.summary.income, "4");
    assert_eq!(r.timeline.len(), 1);
}
