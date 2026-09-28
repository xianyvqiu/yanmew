use super::*;
use chrono::{FixedOffset, TimeZone};

fn time(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text).unwrap().to_utc()
}

fn schedule(json: &str) -> schedule::Schedule {
    serde_json::from_str(json).unwrap()
}

#[test]
fn calendar_skips_missing_month_days_and_reprojects_local_cursor() {
    let zone = FixedOffset::east_opt(8 * 3600).unwrap();
    let mut monthly = schedule(r#"{"repeat":"monthly","time":"09:00","days":[31]}"#);
    let next = monthly.next(&zone, time("2026-02-01T00:00:00Z")).unwrap();
    assert_eq!(next, time("2026-03-31T01:00:00Z"));
    monthly.set_cursor(&zone, next);
    assert_eq!(
        monthly.cursor(&Utc, next).unwrap(),
        time("2026-03-31T09:00:00Z")
    );
    let weekly = schedule(r#"{"repeat":"weekly","time":"09:00","weekdays":[1,5]}"#);
    assert_eq!(
        weekly.next(&zone, time("2026-09-28T01:00:00Z")).unwrap(),
        time("2026-10-02T01:00:00Z")
    );
    assert!(
        schedule(r#"{"repeat":"weekly","time":"09:00","weekdays":[1,1]}"#)
            .validate()
            .is_err()
    );
}

// 固定测试时区：3 月 29 日 02 点跳到 03 点，10 月 25 日 03 点回到 02 点。
// 不依赖测试电脑所在地区，也不新增运行时或测试依赖。
#[derive(Clone)]
struct SeasonalZone;
impl TimeZone for SeasonalZone {
    type Offset = FixedOffset;
    fn from_offset(_: &Self::Offset) -> Self {
        Self
    }
    fn offset_from_local_date(
        &self,
        date: &chrono::NaiveDate,
    ) -> chrono::LocalResult<Self::Offset> {
        self.offset_from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_local_datetime(
        &self,
        local: &chrono::NaiveDateTime,
    ) -> chrono::LocalResult<Self::Offset> {
        let spring = time("2026-03-29T02:00:00Z").naive_utc();
        let autumn = time("2026-10-25T02:00:00Z").naive_utc();
        let one_hour = chrono::Duration::hours(1);
        let winter = FixedOffset::east_opt(3600).unwrap();
        let summer = FixedOffset::east_opt(7200).unwrap();
        if *local >= spring && *local < spring + one_hour {
            chrono::LocalResult::None
        } else if *local >= autumn && *local < autumn + one_hour {
            chrono::LocalResult::Ambiguous(summer, winter)
        } else {
            chrono::LocalResult::Single(if *local >= spring + one_hour && *local < autumn {
                summer
            } else {
                winter
            })
        }
    }
    fn offset_from_utc_date(&self, date: &chrono::NaiveDate) -> Self::Offset {
        self.offset_from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_utc_datetime(&self, utc: &chrono::NaiveDateTime) -> Self::Offset {
        let summer = *utc >= time("2026-03-29T01:00:00Z").naive_utc()
            && *utc < time("2026-10-25T01:00:00Z").naive_utc();
        FixedOffset::east_opt(if summer { 7200 } else { 3600 }).unwrap()
    }
}

#[test]
fn daylight_saving_skips_nonexistent_time_and_never_uses_second_occurrence() {
    let daily = schedule(r#"{"repeat":"daily","time":"02:30"}"#);
    assert_eq!(
        daily
            .next(&SeasonalZone, time("2026-03-28T12:00:00Z"))
            .unwrap(),
        time("2026-03-30T00:30:00Z")
    );
    assert_eq!(
        daily
            .next(&SeasonalZone, time("2026-10-24T12:00:00Z"))
            .unwrap(),
        time("2026-10-25T00:30:00Z")
    );
    assert_eq!(
        daily
            .next(&SeasonalZone, time("2026-10-25T00:30:00Z"))
            .unwrap(),
        time("2026-10-26T01:30:00Z")
    );
}

#[test]
fn offline_recovery_is_atomic_idempotent_and_preserves_queued_records() {
    let mut db = database();
    let now = time("2026-10-03T12:00:00Z");
    db.execute_batch("CREATE TRIGGER fail_cursor BEFORE UPDATE ON reminders BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(dispatch(&mut db, now, &Utc, true).is_err());
    assert_eq!(
        list_occurrences(&db, None, None, None, 50)
            .unwrap()
            .items
            .len(),
        2
    );
    db.execute_batch("DROP TRIGGER fail_cursor;").unwrap();
    assert_eq!(dispatch(&mut db, now, &Utc, true).unwrap(), 1);
    assert_eq!(dispatch(&mut db, now, &Utc, true).unwrap(), 0);
    assert_eq!(
        get_occurrence(&db, 1).unwrap().delivery_state,
        ReminderDeliveryState::Queued
    );
    let missed = missed_summary(&db, None).unwrap();
    assert_eq!(missed.text, "你离开这段时间，喝水过期了！");
    assert_eq!(
        missed.occurrences.last().unwrap().scheduled_at,
        "2026-10-03T09:00:00.000Z"
    );
    assert_eq!(
        get_rule(&db, 2).unwrap().fire_at.as_deref(),
        Some("2026-10-04T09:00:00.000Z")
    );
}

#[test]
fn once_reschedule_keeps_history_and_old_completion_does_not_finish_new_plan() {
    let mut db = database();
    let now = time("2026-09-28T03:00:00Z");
    let input = ReminderInput {
        name: "服药".into(),
        content: "饭后".into(),
        schedule: None,
        fire_at: Some("2026-09-28T04:00:00Z".into()),
    };
    save_rule(&mut db, Some(1), input, "manual", now, &Utc).unwrap();
    update_occurrence_status(&mut db, 1, ReminderStatus::Done, now).unwrap();
    assert_eq!(get_rule(&db, 1).unwrap().status, ReminderStatus::Pending);
    let later = time("2026-09-28T04:00:00Z");
    assert_eq!(dispatch(&mut db, later, &Utc, false).unwrap(), 1);
    update_occurrence_status(&mut db, 1, ReminderStatus::Expired, later).unwrap();
    assert_eq!(get_rule(&db, 1).unwrap().status, ReminderStatus::Pending);
    assert!(get_rule(&db, 1).unwrap().fire_at.is_none());
    delete_rule(&db, 1).unwrap();
    assert!(list_occurrences(&db, Some(1), None, None, 50)
        .unwrap()
        .items
        .is_empty());
}

#[test]
fn new_rules_recover_all_once_and_only_latest_recurring_without_losing_edits() {
    let mut db = Connection::open_in_memory().unwrap();
    db.execute_batch(include_str!("../../../db/schema.sql"))
        .unwrap();
    let start = time("2026-09-28T00:00:00Z");
    for hour in [1, 2, 3] {
        save_rule(
            &mut db,
            None,
            ReminderInput {
                name: format!("提醒{hour}"),
                content: "详情".into(),
                schedule: None,
                fire_at: Some(format!("2026-09-28T0{hour}:00:00Z")),
            },
            "manual",
            start,
            &Utc,
        )
        .unwrap();
    }
    let mut input = ReminderInput {
        name: "喝水".into(),
        content: "一杯水".into(),
        schedule: Some(schedule(r#"{"version":1,"repeat":"daily","time":"09:00"}"#)),
        fire_at: None,
    };
    let initial = save_rule(&mut db, None, input.clone(), "chat", start, &Utc).unwrap();
    input.name = "上午喝水".into();
    let edited = save_rule(
        &mut db,
        Some(4),
        input,
        "manual",
        start + chrono::Duration::hours(12),
        &Utc,
    )
    .unwrap();
    assert_eq!(edited.fire_at, initial.fire_at);
    assert_eq!(edited.source, "chat");
    let now = time("2026-10-03T12:00:00Z");
    assert_eq!(dispatch(&mut db, now, &Utc, true).unwrap(), 4);
    let summary = missed_summary(&db, None).unwrap();
    assert_eq!(summary.occurrences.len(), 4);
    assert!(summary
        .occurrences
        .iter()
        .all(|item| item.status == ReminderStatus::Expired));
    assert_eq!(get_rule(&db, 1).unwrap().status, ReminderStatus::Expired);
    assert_eq!(get_rule(&db, 4).unwrap().status, ReminderStatus::Pending);
    acknowledge(&mut db, &[1, 2, 3, 4], ReminderDeliveryState::Missed).unwrap();
    assert_eq!(dispatch(&mut db, now, &Utc, true).unwrap(), 0);
    assert!(missed_summary(&db, None).unwrap().occurrences.is_empty());
}

fn database() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(include_str!("../../../db/schema.sql"))
        .unwrap();
    connection.execute_batch("INSERT INTO reminders(id,name,kind,schedule,content,status,fire_at,source,created_at)
        VALUES(1,'服药','once',NULL,'饭后服药','pending',NULL,'manual','2026-09-28T00:00:00Z'),
        (2,'喝水','daily','{\"repeat\":\"daily\",\"time\":\"09:00\"}','一杯水','pending','2026-09-29T01:00:00Z','manual','2026-09-28T00:00:00Z');
        INSERT INTO reminder_occurrences(id,reminder_id,scheduled_at,status,delivery_state,created_at)
        VALUES(1,1,'2026-09-28T01:00:00Z','pending','queued','2026-09-28T01:00:00Z'),
        (2,2,'2026-09-28T01:00:00Z','expired','missed','2026-09-28T02:00:00Z');").unwrap();
    connection
}

#[test]
fn completing_once_syncs_rule_but_recurring_keeps_next_fire_and_retries_are_idempotent() {
    let mut db = database();
    let now = DateTime::parse_from_rfc3339("2026-09-28T03:00:00Z")
        .unwrap()
        .to_utc();
    let first = update_occurrence_status(&mut db, 1, ReminderStatus::Done, now).unwrap();
    let again = update_occurrence_status(
        &mut db,
        1,
        ReminderStatus::Done,
        now + chrono::Duration::hours(1),
    )
    .unwrap();
    assert_eq!(first.completed_at, again.completed_at);
    assert_eq!(again.delivery_state, ReminderDeliveryState::Queued);
    let rule: String = db
        .query_row("SELECT status FROM reminders WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rule, "done");
    update_occurrence_status(&mut db, 2, ReminderStatus::Done, now).unwrap();
    let rule: (String, String) = db
        .query_row("SELECT status,fire_at FROM reminders WHERE id=2", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(rule, ("pending".into(), "2026-09-29T01:00:00Z".into()));
    let pending = update_occurrence_status(&mut db, 1, ReminderStatus::Pending, now).unwrap();
    assert!(pending.completed_at.is_none());
    let fire: Option<String> = db
        .query_row("SELECT fire_at FROM reminders WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert!(fire.is_none());
}

#[test]
fn acknowledge_only_claimed_delivery_and_page_without_losing_new_rows() {
    let mut db = database();
    let page = list_occurrences(&db, None, None, None, 1).unwrap();
    assert_eq!(page.next_after_id.as_deref(), Some("1"));
    acknowledge(&mut db, &[1, 2], ReminderDeliveryState::Queued).unwrap();
    assert_eq!(
        get_occurrence(&db, 1).unwrap().status,
        ReminderStatus::Pending
    );
    assert_eq!(
        get_occurrence(&db, 2).unwrap().delivery_state,
        ReminderDeliveryState::Missed
    );
    let next = list_occurrences(&db, None, None, Some(1), 1).unwrap();
    assert_eq!(next.items[0].id, "2");
    assert!(next.next_after_id.is_none());
    acknowledge(&mut db, &[2], ReminderDeliveryState::Missed).unwrap();
    acknowledge(&mut db, &[2], ReminderDeliveryState::Missed).unwrap();
    assert!(
        list_occurrences(&db, None, Some(ReminderDeliveryState::Missed), None, 50)
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn failed_rule_update_rolls_back_occurrence() {
    let mut db = database();
    db.execute_batch("CREATE TRIGGER reject_reminder BEFORE UPDATE ON reminders BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(update_occurrence_status(&mut db, 1, ReminderStatus::Done, Utc::now()).is_err());
    let record = get_occurrence(&db, 1).unwrap();
    assert_eq!(record.status, ReminderStatus::Pending);
    assert!(record.completed_at.is_none());
}
