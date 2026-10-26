use super::*;
use generation::{Key, Kind};
struct Fixed(u64);
impl RandomSource for Fixed {
    fn below(&mut self, upper: u64) -> AppResult<u64> {
        Ok(self.0 % upper)
    }
}
fn now() -> DateTime<Utc> {
    parse_time("2026-10-08T00:00:00Z").unwrap()
}
fn minute(n: i64) -> DateTime<Utc> {
    now() + chrono::Duration::minutes(n)
}
fn scope() -> Scope {
    Scope {
        persona: 1,
        session: 1,
        revision: 0,
    }
}
fn db() -> Connection {
    let c = crate::db::test_database();
    c.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'一','a','a'),(2,1,'二','a','a')",[]).unwrap();
    c.execute("INSERT INTO settings VALUES('current_session_id','1')", [])
        .unwrap();
    c.execute("INSERT INTO travels(id,session_id,status,started_at,planned_return_at,midway_letter_at,facts_state,destination,activities_json,facts_version,facts_ready_at,created_at) VALUES(1,1,'away',?1,?2,?3,'ready','湖边',?4,1,?1,?1)",params![crate::travel::time(now()),crate::travel::time(minute(60)),crate::travel::time(minute(30)),r#"[{"elapsed_minutes":0,"description":"出发"},{"elapsed_minutes":40,"description":"散步"}]"#]).unwrap();
    c
}
fn user(c: &mut Connection) -> Letter {
    send(c, 0, scope(), 1, " 你好 ", None, now(), &mut Fixed(0)).unwrap()
}
fn midway(c: &mut Connection) -> Letter {
    generation::save(
        c,
        Key {
            travel: 1,
            kind: Kind::Midway,
        },
        "湖边很好 (^_^)",
        minute(30),
    )
    .unwrap()
    .unwrap()
}
fn reply(c: &mut Connection, id: i64, at: i64) -> Option<Letter> {
    generation::save(
        c,
        Key {
            travel: 1,
            kind: Kind::Reply(id),
        },
        "收到你的信 (^_^)",
        minute(at),
    )
    .unwrap()
}
fn pending(c: &Connection, id: i64) -> Letter {
    get(c, 1, id).unwrap()
}

#[test]
fn deadline_draw_is_inclusive_and_remains_after_reopen() {
    let mut c = db();
    let first = user(&mut c);
    assert_eq!(first.content, "你好");
    assert!(first.is_read);
    assert_eq!(first.reply_state, LetterReplyState::Pending);
    assert_eq!(first.reply_due_at, Some(crate::travel::time(minute(10))));
    let last = send(
        &mut c,
        0,
        scope(),
        1,
        "另一封信",
        None,
        now(),
        &mut Fixed(20),
    )
    .unwrap();
    assert_eq!(last.reply_due_at, Some(crate::travel::time(minute(30))));
    let id = first.id.parse().unwrap();
    assert!(reply(&mut c, id, 9).is_none());
    assert_eq!(pending(&c, id).reply_due_at, first.reply_due_at);
    assert!(reply(&mut c, id, 10).is_some());
    assert_eq!(pending(&c, id).reply_due_at, first.reply_due_at);
}
#[test]
fn user_reply_consumes_ai_qualification_even_if_user_reply_is_deleted() {
    let mut c = db();
    let ai = midway(&mut c);
    let parent = ai.id.parse().unwrap();
    let sent = send(
        &mut c,
        0,
        scope(),
        1,
        "谢谢",
        Some(parent),
        minute(31),
        &mut Fixed(0),
    )
    .unwrap();
    assert!(get(&c, 1, parent).unwrap().has_been_replied);
    assert_eq!(sent.reply_due_at, Some(crate::travel::time(minute(41))));
    delete(&c, 1, sent.id.parse().unwrap()).unwrap();
    assert_eq!(
        send(
            &mut c,
            0,
            scope(),
            1,
            "再来",
            Some(parent),
            minute(32),
            &mut Fixed(0)
        )
        .unwrap_err()
        .code,
        "LETTER_ALREADY_REPLIED"
    );
}
#[test]
fn ai_reply_consumes_user_qualification_and_deleted_reply_does_not_regenerate() {
    let mut c = db();
    let sent = user(&mut c);
    let id = sent.id.parse().unwrap();
    let ai = reply(&mut c, id, 10).unwrap();
    assert_eq!(pending(&c, id).reply_state, LetterReplyState::Completed);
    assert!(pending(&c, id).has_been_replied);
    delete(&c, 1, ai.id.parse().unwrap()).unwrap();
    assert!(reply(&mut c, id, 11).is_none());
    assert_eq!(list(&c, 1, None, 30).unwrap().len(), 1);
}
#[test]
fn deleting_original_disposes_late_reply_but_keeps_existing_children() {
    let mut c = db();
    let sent = user(&mut c);
    let id = sent.id.parse().unwrap();
    delete(&c, 1, id).unwrap();
    assert!(reply(&mut c, id, 10).is_none());
    let sent = user(&mut c);
    let id = sent.id.parse().unwrap();
    let ai = reply(&mut c, id, 10).unwrap();
    delete(&c, 1, id).unwrap();
    let ai = get(&c, 1, ai.id.parse().unwrap()).unwrap();
    assert_eq!(ai.reply_to_id, None);
    assert_eq!(ai.travel_id, Some("1".into()));
    assert_eq!(
        c.query_row("SELECT count(*) FROM travels", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn expiry_wins_over_due_reply_and_cancels_original_task() {
    let mut c = db();
    let sent = user(&mut c);
    let id = sent.id.parse().unwrap();
    assert!(reply(&mut c, id, 60).is_none());
    crate::travel::recover_returns(&mut c, minute(60), &mut Fixed(99)).unwrap();
    assert!(reply(&mut c, id, 60).is_none());
    assert_eq!(pending(&c, id).reply_state, LetterReplyState::Cancelled);
    assert_eq!(
        send(
            &mut c,
            0,
            scope(),
            1,
            "已回来",
            None,
            minute(61),
            &mut Fixed(0)
        )
        .unwrap_err()
        .code,
        "LETTER_NOT_AWAY"
    );
}
#[test]
fn switched_cancelled_travel_never_writes_late_reply() {
    let mut c = db();
    let sent = user(&mut c);
    crate::sessions::select(&mut c, 1, 2).unwrap();
    assert!(reply(&mut c, sent.id.parse().unwrap(), 10).is_none());
    assert_eq!(
        pending(&c, sent.id.parse().unwrap()).reply_state,
        LetterReplyState::Cancelled
    );
    assert_eq!(
        send(
            &mut c,
            1,
            scope(),
            1,
            "旧窗口",
            None,
            minute(1),
            &mut Fixed(0)
        )
        .unwrap_err()
        .code,
        "CONTEXT_CHANGED"
    );
}
#[test]
fn midway_waits_for_midpoint_and_deleted_result_keeps_done() {
    let mut c = db();
    let key = Key {
        travel: 1,
        kind: Kind::Midway,
    };
    assert!(generation::save(&mut c, key, "(^_^)", minute(29))
        .unwrap()
        .is_none());
    let ai = midway(&mut c);
    delete(&c, 1, ai.id.parse().unwrap()).unwrap();
    assert!(generation::save(&mut c, key, "(^_^)", minute(40))
        .unwrap()
        .is_none());
    assert_eq!(
        c.query_row("SELECT midway_letter_state FROM travels", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "done"
    );
}
#[test]
fn return_letter_waits_for_real_gift_snapshot_and_never_recreates_after_delete() {
    let mut c = db();
    crate::travel::recover_returns(&mut c, minute(70), &mut Fixed(99)).unwrap();
    let key = Key {
        travel: 1,
        kind: Kind::Return,
    };
    assert!(generation::save(&mut c, key, "已回来 (^_^)", minute(70))
        .unwrap()
        .is_none());
    c.execute(
        "UPDATE travels SET gift_state='done',gift_snapshot_json=?1 WHERE id=1",
        [r#"{"schema_version":1,"kind":"text","content":"石头","note":"湖边带回"}"#],
    )
    .unwrap();
    let ai = generation::save(&mut c, key, "带回石头 (^_^)", minute(71))
        .unwrap()
        .unwrap();
    delete(&c, 1, ai.id.parse().unwrap()).unwrap();
    assert!(generation::save(&mut c, key, "再回来 (^_^)", minute(72))
        .unwrap()
        .is_none());
    assert_eq!(
        c.query_row("SELECT return_letter_state FROM travels", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "done"
    );
}
#[test]
fn generated_reply_failure_rolls_back_insert_and_completed_marker() {
    let mut c = db();
    let sent = user(&mut c);
    let id = sent.id.parse().unwrap();
    c.execute_batch("CREATE TRIGGER reject_completion BEFORE UPDATE OF reply_state ON letters WHEN NEW.reply_state='completed' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(generation::save(
        &mut c,
        Key {
            travel: 1,
            kind: Kind::Reply(id)
        },
        "(^_^)",
        minute(10)
    )
    .is_err());
    assert_eq!(list(&c, 1, None, 30).unwrap().len(), 1);
    assert!(!pending(&c, id).has_been_replied);
    assert_eq!(pending(&c, id).reply_state, LetterReplyState::Pending);
}
#[test]
fn failed_user_reply_does_not_consume_ai_qualification() {
    let mut c = db();
    let ai = midway(&mut c);
    let id = ai.id.parse().unwrap();
    c.execute_batch("CREATE TRIGGER reject_user BEFORE INSERT ON letters WHEN NEW.sender='user' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(send(
        &mut c,
        0,
        scope(),
        1,
        "你好",
        Some(id),
        minute(31),
        &mut Fixed(0)
    )
    .is_err());
    assert!(!get(&c, 1, id).unwrap().has_been_replied);
}
#[test]
fn scope_target_pagination_and_flags_are_isolated() {
    let mut c = db();
    let a = user(&mut c);
    let b = user(&mut c);
    let aid = a.id.parse().unwrap();
    let bid = b.id.parse().unwrap();
    assert!(get(&c, 2, aid).is_err());
    assert!(delete(&c, 2, aid).is_err());
    assert!(flags(&c, 2, aid, Some(true), None).is_err());
    assert_eq!(list(&c, 1, Some(bid), 1).unwrap()[0].id, a.id);
    assert_eq!(list(&c, 1, None, 1).unwrap()[0].id, b.id);
    let marked = flags(&c, 1, aid, Some(true), Some(false)).unwrap();
    assert!(marked.is_favorite);
    assert!(!marked.is_read);
    assert!(send(
        &mut c,
        0,
        scope(),
        2,
        "错误旅行",
        None,
        now(),
        &mut Fixed(0)
    )
    .is_err());
    assert!(send(
        &mut c,
        0,
        scope(),
        1,
        "不能回复用户信",
        Some(aid),
        now(),
        &mut Fixed(0)
    )
    .is_err());
    assert!(send(&mut c, 0, scope(), 1, " \n ", None, now(), &mut Fixed(0)).is_err());
}
#[test]
fn incomplete_ai_output_is_rejected_and_expression_is_preserved() {
    let mut choice = crate::ai::provider::Choice {
        usage: None, reasoning_content: None,
        index: 0,
        finish: crate::ai::provider::Finish::Length,
        tools: vec![],
        content: r#"{"content":"你好","expression":"(^_^)"}"#.into(),
    };
    assert!(generation::parse(&choice).is_err());
    choice.finish = crate::ai::provider::Finish::Stop;
    assert_eq!(generation::parse(&choice).unwrap(), "你好\n(^_^)");
    choice.content = r#"{"content":"","expression":"(^_^)"}"#.into();
    assert!(generation::parse(&choice).is_err());
}

#[test]
fn complete_letter_accepts_fences_extra_metadata_and_wide_expression() {
    let choice=crate::ai::provider::Choice{usage: None, reasoning_content:None,index:0,finish:crate::ai::provider::Finish::Stop,tools:vec![],content:"```json\n{\"content\":\"我回来了，带回一颗小石头。\",\"expression\":\"(ﾉ◕ヮ◕)ﾉ*:･ﾟ✧\",\"title\":\"回来啦\"}\n```".into()};
    assert!(crate::expressions::validate_expression("(ﾉ◕ヮ◕)ﾉ*:･ﾟ✧").is_err());
    assert_eq!(generation::parse(&choice).unwrap(),"我回来了，带回一颗小石头。\n(ﾉ◕ヮ◕)ﾉ*:･ﾟ✧");
}
#[test]
fn missing_or_invalid_decoration_keeps_valid_letter_body() {
    for suffix in ["",",\"expression\":null",",\"expression\":42",",\"expression\":\"\"",",\"expression\":\"坏\\u0000符号\""] {
        let choice=crate::ai::provider::Choice{usage: None, reasoning_content:None,index:0,finish:crate::ai::provider::Finish::Stop,tools:vec![],content:format!("{{\"content\":\"完整来信\"{suffix}}}")};
        assert_eq!(generation::parse(&choice).unwrap(),"完整来信\n(^_^)");
    }
}
#[test]
fn invalid_letter_body_and_unfinished_output_have_specific_errors() {
    let mut choice=crate::ai::provider::Choice{usage: None, reasoning_content:None,index:0,finish:crate::ai::provider::Finish::Stop,tools:vec![],content:"{\"expression\":\"(^_^)\"}".into()};
    assert!(generation::parse(&choice).unwrap_err().message.contains("缺少正文"));
    choice.content="{\"content\":\"坏\\u0000正文\"}".into();assert!(generation::parse(&choice).unwrap_err().message.contains("控制字符"));
    choice.content="```json\n{\"content\":\"未关闭代码块\"}".into();assert!(generation::parse(&choice).is_err());
    choice.finish=crate::ai::provider::Finish::Length;assert!(generation::parse(&choice).unwrap_err().message.contains("被截断"));
    choice.finish=crate::ai::provider::Finish::ContentFilter;assert!(generation::parse(&choice).unwrap_err().message.contains("拦截"));
}
