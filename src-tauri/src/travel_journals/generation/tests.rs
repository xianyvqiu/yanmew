use super::*;
fn runtime(url: &str) -> Runtime {
    let r = crate::runtime::test_runtime(url);
    r.read(|s, _| {
        crate::travel_journals::tests::seed(&s.connection);
        crate::config::save(
            &mut s.connection,
            [
                ("travel_journal_min_chars".into(), "2".into()),
                ("travel_journal_max_chars".into(), "5".into()),
            ]
            .into_iter()
            .collect(),
        )?;
        Ok(())
    })
    .unwrap();
    r
}
fn output() -> Output {
    Output {
        title: "十月十三日去湖边散步，带回明信片，花了一小时".into(),
        content: "湖边散步".into(),
    }
}
fn choice() -> Choice {
    Choice {
        usage: None, reasoning_content: None,
        index: 0,
        content: json!({"title":output().title,"content":"湖边散步🙂ABC!?"}).to_string(),
        finish: Finish::Stop,
        tools: vec![],
    }
}
fn add_letter(r: &Runtime, body: &str) {
    r.read(|s,_|{let kind=if s.connection.query_row("SELECT EXISTS(SELECT 1 FROM letters WHERE travel_id=1 AND letter_kind='midway')",[],|r|r.get::<_,bool>(0))? {"return"} else {"midway"};s.connection.execute("INSERT INTO letters(session_id,travel_id,sender,content,created_at,letter_kind) VALUES(1,1,'ai',?1,'a',?2)",params![body,kind])?;Ok(())}).unwrap();
}
#[test]
fn parser_uses_han_count_excludes_title_and_rejects_truncated_or_multiline_title() {
    assert!(parse(&choice(), 4, 4).is_ok());
    assert!(parse(&choice(), 5, 8).is_err());
    let mut bad = choice();
    bad.finish = Finish::Length;
    assert!(parse(&bad, 2, 5).is_err());
    bad = choice();
    bad.content = json!({"title":"两行\n标题","content":"湖边散步"}).to_string();
    assert!(parse(&bad, 2, 5).is_err());
    bad = choice();
    bad.tools.push(crate::ai::provider::ToolCall {
        index: 0,
        id: "x".into(),
        name: "unexpected".into(),
        arguments: json!({}),
    });
    assert!(parse(&bad, 2, 5).is_err());
}
#[test]
fn capture_reads_immutable_gift_after_display_deleted_and_does_not_wait_for_return_letter() {
    let r = runtime("http://localhost");
    let (snapshot, lease) = capture(&r, 1).unwrap();
    let input = &snapshot.request.messages[1].content;
    assert!(input.contains("明信片"));
    assert!(input.contains("散步"));
    assert!(input.contains("3600"));
    assert!(snapshot.request.tools.is_empty());
    assert_eq!(snapshot.request.max_tokens, 32768);
    assert!(commit(&r, &snapshot, &lease, &output()).unwrap());
    r.read(|s, _| {
        assert_eq!(
            s.connection.query_row(
                "SELECT return_letter_state FROM travels WHERE id=1",
                [],
                |r| r.get::<_, String>(0)
            )?,
            "pending"
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn new_letter_does_not_invalidate_optional_snapshot_but_removed_used_letter_does() {
    let r = runtime("http://localhost");
    add_letter(&r, "原信");
    let (snapshot, lease) = capture(&r, 1).unwrap();
    add_letter(&r, "新到达的回来信");
    r.read(|s, secrets| require_snapshot(&r, s, secrets, &snapshot, &lease))
        .unwrap();
    r.read(|s, _| {
        s.connection
            .execute("DELETE FROM letters WHERE content='原信'", [])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        commit(&r, &snapshot, &lease, &output()).unwrap_err().code,
        "CONTEXT_CHANGED"
    );
}
#[test]
fn changed_persona_provider_range_or_cancellation_does_not_commit_old_output() {
    for change in [
        "UPDATE personas SET prompt='新的设定' WHERE id=1",
        "UPDATE provider_models SET model='new' WHERE purpose='chat' AND provider_id=(SELECT provider_id FROM provider_roles WHERE purpose='chat')",
        "UPDATE settings SET value='3' WHERE key='travel_journal_min_chars'",
    ] {
        let r = runtime("http://localhost");
        let (snapshot, lease) = capture(&r, 1).unwrap();
        r.read(|s, _| {
            s.connection.execute(change, [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            commit(&r, &snapshot, &lease, &output()).unwrap_err().code,
            "CONTEXT_CHANGED"
        );
    }
    let r = runtime("http://localhost");
    let (snapshot, lease) = capture(&r, 1).unwrap();
    lease.cancel.cancel();
    assert_eq!(
        commit(&r, &snapshot, &lease, &output()).unwrap_err().code,
        "CANCELLED"
    );
}
#[test]
fn budget_removes_only_optional_old_letters_and_keeps_database_content() {
    let r = runtime("http://localhost");
    add_letter(&r, &"旧".repeat(4000));
    add_letter(&r, "新信");
    r.read(|s, _| {
        s.connection.execute(
            "UPDATE provider_models SET context_window_override=6000 WHERE purpose='chat' AND provider_id=(SELECT provider_id FROM provider_roles WHERE purpose='chat')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let (snapshot, _lease) = capture(&r, 1).unwrap();
    assert!(snapshot.reduced);
    assert_eq!(snapshot.letters.len(), 1);
    assert_eq!(snapshot.letters[0].content, "新信");
    assert!(snapshot.request.messages[1].content.contains("明信片"));
    r.read(|s, _| {
        assert_eq!(
            s.connection
                .query_row("SELECT count(*) FROM letters", [], |r| r.get::<_, i64>(0))?,
            2
        );
        Ok(())
    })
    .unwrap();
}
#[tokio::test]
async fn real_worker_generation_is_once_and_deleted_done_record_is_not_restored() {
    let server = crate::ai::tests::Server::new(vec![crate::ai::tests::Response::json(
        json!({"title":output().title,"content":output().content}),
    )]);
    let r = runtime(&server.url);
    assert!(r.travel_journals.process(&r, 1).await.unwrap());
    r.read(|s, _| {
        let j = crate::travel_journals::list(&s.connection, 1, None, 30)?;
        assert_eq!(j.len(), 1);
        crate::travel_journals::delete(&mut s.connection, 1, 1, j[0].id.parse().unwrap())?;
        Ok(())
    })
    .unwrap();
    assert!(capture(&r, 1).is_err());
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    r.read(|s, _| {
        let fresh = Worker::default();
        assert_eq!(fresh.status(&s.connection, 1)?.pending_count, 0);
        assert!(crate::travel_journals::list(&s.connection, 1, None, 30)?.is_empty());
        Ok(())
    })
    .unwrap();
    server.finish();
}
#[test]
fn retry_and_status_are_scoped_and_cancelled_or_unready_travel_is_not_captured() {
    let r = runtime("http://localhost");
    r.travel_journals
        .finish(1, &Err(AppError::new("AI_ERROR", "失败")));
    r.read(|s, _| {
        assert_eq!(
            r.travel_journals.status(&s.connection, 1)?.failures.len(),
            1
        );
        r.travel_journals.retry(&s.connection, 2)?;
        assert_eq!(
            r.travel_journals.status(&s.connection, 1)?.failures.len(),
            1
        );
        r.travel_journals.retry(&s.connection, 1)?;
        assert!(r
            .travel_journals
            .status(&s.connection, 1)?
            .failures
            .is_empty());
        Ok(())
    })
    .unwrap();
    r.read(|s, _| {
        s.connection.execute(
            "UPDATE travels SET gift_state='pending',gift_snapshot_json=NULL WHERE id=1",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(capture(&r, 1).is_err());
    r.read(|s,_|{s.connection.execute("UPDATE travels SET status='cancelled',actual_return_at=NULL,cancelled_at='a',gift_kind=NULL,gift_state='cancelled',journal_state='cancelled' WHERE id=1",[])?;Ok(())}).unwrap();
    assert!(capture(&r, 1).is_err());
}
#[test]
fn old_generation_cannot_replace_manual_edit_and_tools_do_not_recover_deleted_journal() {
    let r = runtime("http://localhost");
    let (snapshot, lease) = capture(&r, 1).unwrap();
    r.read(|s, _| {
        let tx = s.connection.transaction()?;
        save(&tx, 1, 1, &output(), r.clock.sample()?.utc)?;
        tx.commit()?;
        let j = crate::travel_journals::get(&s.connection, 1, 1)?;
        let expected = crate::models::TravelJournalExpected {
            title: j.title,
            content: j.content,
            updated_at: j.updated_at,
        };
        crate::travel_journals::update(
            &mut s.connection,
            1,
            1,
            1,
            expected,
            "我改的标题",
            "我自己写的正文",
            r.clock.sample()?.utc,
        )?;
        Ok(())
    })
    .unwrap();
    assert!(commit(&r, &snapshot, &lease, &output()).is_err());
    let chat = Arc::new(r.requests.start("chat:1".into(), 1).unwrap());
    let mut tools =
        crate::travel_journal_tools::TravelJournalTools::new(&r, &chat, 1, 1, 0).unwrap();
    let call = crate::ai::provider::ToolCall {
        index: 0,
        id: "open".into(),
        name: crate::travel_journal_tools::OPEN_TOOL.into(),
        arguments: json!({"journal_id":"1"}),
    };
    assert!(tools.execute(&call).is_ok());
    r.read(|s, _| {
        assert_eq!(
            crate::travel_journals::get(&s.connection, 1, 1)?.title,
            "我改的标题"
        );
        crate::travel_journals::delete(&mut s.connection, 1, 1, 1)?;
        Ok(())
    })
    .unwrap();
    assert!(tools.execute(&call).is_err());
}
#[tokio::test]
async fn failed_output_stays_pending_and_later_retry_saves_one_record() {
    let server = crate::ai::tests::Server::new(vec![
        crate::ai::tests::Response::json(json!({"title":"标题","content":"字".repeat(9)})),
        crate::ai::tests::Response::json(
            json!({"title":output().title,"content":output().content}),
        ),
    ]);
    let r = runtime(&server.url);
    let result = r.travel_journals.process(&r, 1).await;
    assert!(result.is_err());
    r.travel_journals.finish(1, &result);
    r.read(|s, _| {
        assert_eq!(r.travel_journals.status(&s.connection, 1)?.pending_count, 1);
        assert!(crate::travel_journals::list(&s.connection, 1, None, 30)?.is_empty());
        r.travel_journals.retry(&s.connection, 1)
    })
    .unwrap();
    assert!(r.travel_journals.process(&r, 1).await.unwrap());
    r.read(|s, _| {
        assert_eq!(
            crate::travel_journals::list(&s.connection, 1, None, 30)?.len(),
            1
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    server.finish();
}
