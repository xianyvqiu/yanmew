use super::*;
#[test]
fn decimal_cents_are_exact() {
    assert_eq!(cents("12.50", false).unwrap(), 1250);
    assert_eq!(cents("-0.01", true).unwrap(), -1);
    assert!(cents("1.001", false).is_err());
    assert!(cents("1e3", false).is_err());
}
use chrono::TimeZone;
fn at(text: &str) -> DateTime<Utc> {
    Local
        .from_local_datetime(&datetime(text).unwrap())
        .earliest()
        .unwrap()
        .to_utc()
}
fn now() -> DateTime<Utc> {
    at("2026-10-20T12:00")
}
fn actor() -> Actor {
    Actor {
        source: "manual",
        name: "测试者".into(),
    }
}
fn act(c: &mut Connection, key: &str, action: Action) -> Value {
    mutate(c, key, action, now(), actor()).unwrap()
}
fn input(c: &Connection, amount: &str, kind: &str, time: &str) -> EntryInput {
    let category = c
        .query_row(
            "SELECT id FROM real_categories WHERE name=?1",
            [if kind == "income" { "工资" } else { "食物" }],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    EntryInput {
        title: "测试账单".into(),
        kind: kind.into(),
        category_id: category.to_string(),
        asset_id: "1".into(),
        description: "备注".into(),
        amount: amount.into(),
        occurred_local: Some(time.into()),
        recorder: None,
    }
}
fn save(c: &Connection, amount: &str, kind: &str) -> Action {
    Action::SaveEntry {
        id: None,
        revision: None,
        entry: input(c, amount, kind, "2026-10-20T10:00"),
        recurrence: None,
    }
}
#[test]
fn real_money_calibration_and_income_do_not_offset_expense_budget() {
    let mut c = crate::db::test_database();
    act(
        &mut c,
        "total",
        Action::SetTotal {
            amount: "1000".into(),
        },
    );
    let a = save(&c, "12.34", "expense");
    act(&mut c, "expense", a);
    let a = save(&c, "20", "income");
    act(&mut c, "income", a);
    assert_eq!(
        records::overview(&c, now()).unwrap()["totalAssetsCents"],
        "100766"
    );
    act(
        &mut c,
        "budgets",
        Action::SaveBudgets {
            day: Some("100".into()),
            month: Some("1000".into()),
            year: None,
        },
    );
    act(
        &mut c,
        "total2",
        Action::SetTotal {
            amount: "2000".into(),
        },
    );
    let meta = records::overview(&c, now()).unwrap();
    assert_eq!(meta["totalAssetsCents"], "200000");
    assert_eq!(meta["budgets"][0]["spentCents"], "1234");
    assert_eq!(meta["budgets"][1]["remainingCents"], "98766");
    assert_eq!(meta["budgets"][2]["amountCents"], Value::Null);
    assert_eq!(meta["budgets"][1]["elapsedDays"], 20);
    assert_eq!(meta["budgets"][1]["remainingDays"], 12);
    assert_eq!(crate::currency::get_balance(&c).unwrap(), 0);
}
#[test]
fn real_money_replay_edit_delete_and_stale_revision() {
    let mut c = crate::db::test_database();
    let a = save(&c, "20", "expense");
    let first = act(&mut c, "first", a.clone());
    assert_eq!(act(&mut c, "first", a)["alreadyApplied"], true);
    let different = save(&c, "30", "expense");
    assert!(mutate(&mut c, "first", different, now(), actor()).is_err());
    let id = first["id"].as_str().unwrap().to_owned();
    let e = input(&c, "25.01", "expense", "2026-10-20T10:00");
    act(
        &mut c,
        "edit",
        Action::SaveEntry {
            id: Some(id.clone()),
            revision: Some(1),
            entry: e.clone(),
            recurrence: None,
        },
    );
    assert_eq!(balances(&c).unwrap()[&1], -2501);
    assert!(mutate(
        &mut c,
        "stale",
        Action::SaveEntry {
            id: Some(id.clone()),
            revision: Some(1),
            entry: e,
            recurrence: None
        },
        now(),
        actor()
    )
    .is_err());
    act(&mut c, "delete", Action::DeleteEntry { id, revision: 2 });
    assert_eq!(balances(&c).unwrap().values().sum::<i128>(), 0);
}
#[test]
fn real_money_asset_name_edit_and_archived_category_preserve_history() {
    let mut c = crate::db::test_database();
    let a = save(&c, "5", "expense");
    act(&mut c, "spend", a);
    act(
        &mut c,
        "name",
        Action::SaveAsset {
            id: Some("1".into()),
            revision: Some(1),
            name: "钱包".into(),
            note: "保留余额".into(),
            balance: None,
            is_default: true,
        },
    );
    assert_eq!(balances(&c).unwrap()[&1], -500);
    let cat = c
        .query_row(
            "SELECT id FROM real_categories WHERE name='食物'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    act(
        &mut c,
        "archive",
        Action::DeleteCategory {
            id: cat.to_string(),
            revision: 1,
        },
    );
    let r = records::query(
        &c,
        records::Query {
            page: 1,
            ..Default::default()
        },
        now(),
    )
    .unwrap();
    assert_eq!(r["entries"][0]["categoryName"], "食物");
    assert_eq!(r["summary"]["expense"], "500");
}
#[test]
fn real_money_month_end_and_leap_day_keep_original_anchor() {
    let rule = Rule {
        unit: "month".into(),
        interval: 1,
    };
    let anchor = datetime("2026-01-31T09:30").unwrap();
    assert_eq!(
        cycles::due(anchor, &rule, 1).unwrap(),
        datetime("2026-02-28T09:30").unwrap()
    );
    assert_eq!(
        cycles::due(anchor, &rule, 2).unwrap(),
        datetime("2026-03-31T09:30").unwrap()
    );
    let rule = Rule {
        unit: "year".into(),
        interval: 1,
    };
    let anchor = datetime("2024-02-29T09:30").unwrap();
    assert_eq!(
        cycles::due(anchor, &rule, 1).unwrap().date(),
        date("2025-02-28").unwrap()
    );
    assert_eq!(
        cycles::due(anchor, &rule, 4).unwrap().date(),
        date("2028-02-29").unwrap()
    );
}
#[test]
fn real_money_missed_selection_live_tick_and_deleted_period_not_regenerated() {
    let mut c = crate::db::test_database();
    let e = input(&c, "5", "expense", "2026-10-01T09:00");
    mutate(
        &mut c,
        "plan",
        Action::SaveEntry {
            id: None,
            revision: None,
            entry: e,
            recurrence: Some(Rule {
                unit: "day".into(),
                interval: 1,
            }),
        },
        at("2026-09-30T12:00"),
        actor(),
    )
    .unwrap();
    let mut worker = cycles::Worker::default();
    worker.tick(&mut c, at("2026-10-03T10:00")).unwrap();
    let meta = records::overview(&c, now()).unwrap();
    assert_eq!(meta["pending"]["count"], 3);
    assert_eq!(meta["totalAssetsCents"], "0");
    let first = c
        .query_row("SELECT min(id) FROM real_occurrences", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap()
        .to_string();
    act(
        &mut c,
        "one",
        Action::ResolveMissed {
            ids: vec![first.clone()],
            through_id: None,
            record: true,
        },
    );
    assert_eq!(balances(&c).unwrap()[&1], -500);
    act(
        &mut c,
        "skip",
        Action::ResolveMissed {
            ids: vec![],
            through_id: Some(meta["pending"]["throughId"].as_str().unwrap().into()),
            record: false,
        },
    );
    assert_eq!(
        act(
            &mut c,
            "again",
            Action::ResolveMissed {
                ids: vec![first],
                through_id: None,
                record: true
            }
        )["changed"],
        0
    );
    let id = c
        .query_row("SELECT id FROM real_entries LIMIT 1", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap();
    act(
        &mut c,
        "erase",
        Action::DeleteEntry {
            id: id.to_string(),
            revision: 1,
        },
    );
    assert!(!worker.tick(&mut c, at("2026-10-03T11:00")).unwrap());
    assert_eq!(records::overview(&c, now()).unwrap()["pending"]["count"], 0);
    worker.tick(&mut c, at("2026-10-04T10:00")).unwrap();
    assert_eq!(balances(&c).unwrap()[&1], -500);
}
#[test]
fn real_money_snapshot_survives_future_amount_and_rule_changes() {
    let mut c = crate::db::test_database();
    let e = input(&c, "5", "expense", "2026-01-31T09:00");
    let r = mutate(
        &mut c,
        "plan",
        Action::SaveEntry {
            id: None,
            revision: None,
            entry: e,
            recurrence: Some(Rule {
                unit: "month".into(),
                interval: 1,
            }),
        },
        at("2026-01-01T12:00"),
        actor(),
    )
    .unwrap();
    let id = r["scheduleId"].as_str().unwrap().to_owned();
    let mut w = cycles::Worker::default();
    w.tick(&mut c, at("2026-03-01T12:00")).unwrap();
    let e = input(&c, "7", "expense", "2026-01-31T09:00");
    mutate(
        &mut c,
        "edit",
        Action::SaveSchedule {
            id: id.clone(),
            revision: 1,
            entry: e,
            rule: Rule {
                unit: "month".into(),
                interval: 1,
            },
            status: "active".into(),
        },
        at("2026-03-01T12:00"),
        actor(),
    )
    .unwrap();
    let m = records::overview(&c, now()).unwrap();
    act(
        &mut c,
        "fill",
        Action::ResolveMissed {
            ids: vec![],
            through_id: Some(m["pending"]["throughId"].as_str().unwrap().into()),
            record: true,
        },
    );
    assert_eq!(balances(&c).unwrap()[&1], -1000);
    w.tick(&mut c, at("2026-03-31T10:00")).unwrap();
    assert_eq!(balances(&c).unwrap()[&1], -1700);
    let e = input(&c, "8", "expense", "2026-04-01T09:00");
    mutate(
        &mut c,
        "rule",
        Action::SaveSchedule {
            id,
            revision: 2,
            entry: e,
            rule: Rule {
                unit: "day".into(),
                interval: 1,
            },
            status: "active".into(),
        },
        at("2026-03-31T12:00"),
        actor(),
    )
    .unwrap();
    w.tick(&mut c, at("2026-04-01T10:00")).unwrap();
    assert_eq!(balances(&c).unwrap()[&1], -2500);
    validate(&c).unwrap();
}
#[test]
fn real_money_v18_upgrade_preserves_old_wallet() {
    let mut c = crate::db::test_database();
    c.execute("UPDATE user_state SET coins=123", []).unwrap();
    for table in [
        "real_operations",
        "real_occurrences",
        "real_entries",
        "real_schedules",
        "real_budgets",
        "real_categories",
        "real_assets",
    ] {
        c.execute(&format!("DROP TABLE {table}"), []).unwrap();
    }
    // 回退 v19 挂件字段和索引，避免把当前结构误标为 v17 后重复加列。
    c.execute_batch(
        "ALTER TABLE accessories DROP COLUMN render_layer;
         ALTER TABLE accessories DROP COLUMN layer_order;
         CREATE UNIQUE INDEX idx_accessories_one_worn ON accessories(session_id) WHERE is_worn=1;",
    )
    .unwrap();
    c.pragma_update(None, "user_version", 17).unwrap();
    crate::db::initialize(&mut c).unwrap();
    assert_eq!(
        c.pragma_query_value::<i64, _>(None, "user_version", |r| r.get(0))
            .unwrap(),
        20
    );
    assert_eq!(crate::currency::get_balance(&c).unwrap(), 123);
    assert_eq!(
        records::overview(&c, now()).unwrap()["totalAssetsCents"],
        "0"
    );
}
#[test]
fn real_money_ai_confirmation_replay_and_cancelled_scope() {
    use crate::ai::provider::ToolCall;
    use std::sync::Arc;
    let rt = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    let lease = Arc::new(rt.requests.start("chat:1".into(), 1).unwrap());
    let mut tools = tools::Tools::new(&rt, &lease, 1, 1, 0);
    let e = rt
        .read(|s, _| Ok(input(&s.connection, "12.50", "expense", "2026-10-20T10:00")))
        .unwrap();
    let mut call = ToolCall {
        index: 0,
        id: "real-1".into(),
        name: "record_real_money".into(),
        arguments: json!({"entry":e,"recurrence":null,"confirmedRealMoney":false}),
    };
    assert!(tools.execute(&call).is_err());
    call.arguments["confirmedRealMoney"] = json!(true);
    assert_eq!(tools.execute(&call).unwrap()["alreadyApplied"], false);
    assert_eq!(tools.execute(&call).unwrap()["alreadyApplied"], true);
    rt.read(|s, _| {
        assert_eq!(
            s.connection.query_row(
                "SELECT count(*) FROM real_entries WHERE source='ai'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        assert_eq!(crate::currency::get_balance(&s.connection)?, 0);
        Ok(())
    })
    .unwrap();
    rt.requests.cancel_key("chat:1");
    call.id = "cancelled".into();
    assert!(tools.execute(&call).is_err());
}

#[test]
fn real_money_query_pages_aggregate_all_filtered_rows_and_keep_calendar_dates() {
    let mut c = crate::db::test_database();
    for i in 1..=13 {
        let mut entry = input(&c, "1.01", "expense", &format!("2026-10-20T10:{i:02}"));
        entry.title = format!("午餐 {i}");
        act(
            &mut c,
            &format!("page-{i}"),
            Action::SaveEntry {
                id: None,
                revision: None,
                entry,
                recurrence: None,
            },
        );
    }
    let mut other = input(&c, "99", "income", "2026-10-19T23:59");
    other.title = "工资".into();
    act(
        &mut c,
        "outside",
        Action::SaveEntry {
            id: None,
            revision: None,
            entry: other,
            recurrence: None,
        },
    );
    let q = records::Query {
        start: Some("2026-10-20".into()),
        end: Some("2026-10-20".into()),
        direction: "expense".into(),
        search: "午餐".into(),
        ascending: true,
        page: 2,
        ..Default::default()
    };
    let value = records::query(&c, q.clone(), now()).unwrap();
    assert_eq!(
        value["summary"],
        json!({"income":"0","expense":"1313","net":"-1313","count":13})
    );
    assert_eq!(value["entries"].as_array().unwrap().len(), 1);
    assert_eq!(value["entries"][0]["title"], "午餐 13");
    assert_eq!(value["totalPages"], 2);
    assert_eq!(value["categories"][0]["summary"]["expense"], "1313");
    assert_eq!(value["timeline"][0]["summary"]["expense"], "1313");
    let mut invalid = q;
    invalid.start = Some("2026-10-21".into());
    assert!(records::query(&c, invalid, now()).is_err());
}
#[test]
fn real_money_calendar_separates_actual_pending_and_future() {
    let mut c = crate::db::test_database();
    let entry = input(&c, "5", "expense", "2099-10-19T08:00");
    mutate(
        &mut c,
        "future",
        Action::SaveEntry {
            id: None,
            revision: None,
            entry,
            recurrence: Some(Rule {
                unit: "day".into(),
                interval: 1,
            }),
        },
        at("2099-10-18T12:00"),
        actor(),
    )
    .unwrap();
    let entry = input(&c, "12.50", "expense", "2099-10-20T10:00");
    mutate(
        &mut c,
        "actual",
        Action::SaveEntry {
            id: None,
            revision: None,
            entry,
            recurrence: None,
        },
        at("2099-10-20T12:00"),
        actor(),
    )
    .unwrap();
    let mut worker = cycles::Worker::default();
    worker.tick(&mut c, at("2099-10-20T12:00")).unwrap();
    let value =
        cycles::calendar(&c, date("2099-10-19").unwrap(), date("2099-10-21").unwrap()).unwrap();
    assert_eq!(value["days"][0]["pending"], 1);
    assert_eq!(value["days"][0]["expense"], "0");
    assert_eq!(value["days"][1]["pending"], 1);
    assert_eq!(value["days"][1]["expense"], "1250");
    assert_eq!(value["days"][2]["planned"], 1);
    assert_eq!(value["days"][2]["expense"], "0");
    assert_eq!(balances(&c).unwrap()[&1], -1250);
}

#[test]
fn real_money_live_new_future_plan_due_before_next_scan_is_not_missed() {
    let mut c = crate::db::test_database();
    let mut worker = cycles::Worker::default();
    worker.tick(&mut c, at("2026-10-20T10:00")).unwrap();
    let entry = input(&c, "5", "expense", "2026-10-20T10:01");
    mutate(
        &mut c,
        "live-new",
        Action::SaveEntry {
            id: None,
            revision: None,
            entry,
            recurrence: Some(Rule {
                unit: "day".into(),
                interval: 1,
            }),
        },
        at("2026-10-20T10:00"),
        actor(),
    )
    .unwrap();
    worker.tick(&mut c, at("2026-10-20T10:02")).unwrap();
    assert_eq!(balances(&c).unwrap()[&1], -500);
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM real_occurrences WHERE status='pending'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM real_occurrences WHERE status='recorded'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}
