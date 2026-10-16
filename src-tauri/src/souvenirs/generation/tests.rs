use super::*;
fn runtime(url: &str, kind: &str) -> Runtime {
    let r = crate::runtime::test_runtime(url);
    r.read(|store,_|{
        store.connection.execute("INSERT INTO travels(id,session_id,status,started_at,planned_return_at,actual_return_at,midway_letter_at,gift_kind,facts_state,destination,activities_json,facts_version,facts_ready_at,created_at) VALUES(1,1,'returned','2026-10-13T00:00:00Z','2026-10-13T01:00:00Z','2026-10-13T01:00:00Z','2026-10-13T00:30:00Z',?1,'ready','湖边','[{\"elapsed_minutes\":0,\"description\":\"散步\"}]',1,'a','a')",[kind])?;Ok(())
    }).unwrap();
    r
}
fn text() -> Gift {
    Gift::Text {
        content: "明信片".into(),
        note: "湖边的小礼物。".into(),
    }
}
#[test]
fn text_gift_does_not_read_broken_image_configuration() {
    let r = runtime("http://localhost", "text");
    let (snapshot, lease) = capture(&r, 1).unwrap();
    r.read(|s, _| {
        s.connection.execute(
            "INSERT INTO settings(key,value) VALUES('image_generation_config','broken-json')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let p = Prepared {
        gift: text(),
        png: None,
        image_revision: None,
        notice: None,
    };
    assert!(commit(&r, &snapshot, &lease, &p).unwrap());
    assert_eq!(
        r.read(|s, _| Ok(super::super::list(&s.connection, 1, None, 30)?.len()))
            .unwrap(),
        1
    );
}
#[test]
fn parse_rejects_truncation_tools_wrong_kind_and_prose_visual() {
    let choice = Choice {
        reasoning_content: None,
        index: 0,
        content: json!({"kind":"text","content":"明信片","note":"给你的"}).to_string(),
        tools: vec![],
        finish: Finish::Stop,
    };
    assert!(parse(&choice, "text").is_ok());
    assert!(parse(&choice, "accessory").is_err());
    let mut bad = choice.clone();
    bad.finish = Finish::Length;
    assert!(parse(&bad, "text").is_err());
    bad = choice.clone();
    bad.tools.push(crate::ai::provider::ToolCall {
        index: 0,
        id: "x".into(),
        name: "unexpected".into(),
        arguments: json!({}),
    });
    assert!(parse(&bad, "text").is_err());
    let bad = Choice {
        reasoning_content: None,
        content:
            json!({"kind":"accessory","name":"花","display_text":"普通花朵","description":"我的花"})
                .to_string(),
        ..choice
    };
    assert!(parse(&bad, "accessory").is_err());
}
#[test]
fn startup_classifies_returned_and_overdue_away_but_allows_future_return_once() {
    let r = runtime("http://localhost", "accessory");
    r.read(|s,_|{
        for (id,planned) in [(2,"2026-10-13T01:00:00Z"),(3,"2099-10-13T01:00:00Z")] {
            s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(?1,1,'另一会话','a','a')",[id])?;
            s.connection.execute("INSERT INTO travels(id,session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(?1,?1,'away','2026-10-13T00:00:00Z',?2,'2026-10-13T00:30:00Z','a')",params![id,planned])?;
        }
        r.travel_gifts.initialize(&s.connection,DateTime::parse_from_rfc3339("2026-10-13T02:00:00Z").unwrap().with_timezone(&Utc))
    }).unwrap();
    assert!(!r.travel_gifts.take_image_permission(1).unwrap());
    assert!(!r.travel_gifts.take_image_permission(2).unwrap());
    assert!(r.travel_gifts.take_image_permission(3).unwrap());
    assert!(!r.travel_gifts.take_image_permission(3).unwrap());
    r.read(|s, _| r.travel_gifts.retry(&s.connection, 3))
        .unwrap();
    assert!(!r.travel_gifts.take_image_permission(3).unwrap());
}
#[test]
fn changed_persona_or_provider_does_not_commit_old_result() {
    let r = runtime("http://localhost", "text");
    let (snapshot, lease) = capture(&r, 1).unwrap();
    r.read(|s, _| {
        s.connection
            .execute("UPDATE personas SET prompt='已改变' WHERE id=1", [])?;
        Ok(())
    })
    .unwrap();
    let p = Prepared {
        gift: text(),
        png: None,
        image_revision: None,
        notice: None,
    };
    assert_eq!(
        commit(&r, &snapshot, &lease, &p).unwrap_err().code,
        "CONTEXT_CHANGED"
    );
    drop(lease);
    let (snapshot, lease) = capture(&r, 1).unwrap();
    r.read(|s, _| {
        s.connection.execute(
            "UPDATE provider_models SET model='new-model' WHERE purpose='chat' AND provider_id=(SELECT provider_id FROM provider_roles WHERE purpose='chat')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(commit(&r, &snapshot, &lease, &p).is_err());
    assert_eq!(
        r.read(|s, _| Ok(super::super::list(&s.connection, 1, None, 30)?.len()))
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn database_retry_reuses_prepared_result_and_does_not_request_again() {
    let server = crate::ai::tests::Server::new(vec![crate::ai::tests::Response::json(
        json!({"kind":"accessory","name":"小花","display_text":"✿","description":"湖边的小花。"}),
    )]);
    let r = runtime(&server.url, "accessory");
    r.read(|s,_|{s.connection.execute_batch("CREATE TRIGGER fail_accessory BEFORE INSERT ON accessories BEGIN SELECT RAISE(ABORT,'forced'); END;")?;
        r.travel_gifts.initialize(&s.connection,DateTime::parse_from_rfc3339("2026-10-12T02:00:00Z").unwrap().with_timezone(&Utc))?;Ok(())}).unwrap();
    // 在线新返回资格；初始化前已经返回的记录始终作为恢复任务。
    r.travel_gifts.queue.lock().unwrap().attempted.remove(&1);
    let first = r.travel_gifts.process(&r, 1).await;
    assert!(first.is_err());
    assert_eq!(r.travel_gifts.queue.lock().unwrap().prepared.len(), 1);
    assert!(!r.travel_gifts.take_image_permission(1).unwrap());
    r.read(|s, _| {
        s.connection.execute_batch("DROP TRIGGER fail_accessory;")?;
        r.travel_gifts.retry(&s.connection, 1)
    })
    .unwrap();
    assert!(r.travel_gifts.process(&r, 1).await.unwrap());
    assert_eq!(server.requests.lock().unwrap().len(), 1);
    r.read(|s,_|{let (state,image):(String,Option<String>)=s.connection.query_row("SELECT t.gift_state,a.image_file FROM travels t JOIN accessories a ON a.travel_id=t.id WHERE t.id=1",[],|r|Ok((r.get(0)?,r.get(1)?)))?;assert_eq!(state,"done");assert!(image.is_none());Ok(())}).unwrap();
    server.finish();
}
#[test]
fn status_retry_preserve_image_attempt_and_backoff_are_session_scoped() {
    let r = runtime("http://localhost", "text");
    r.travel_gifts
        .finish(1, 1, &Err(AppError::new("AI_ERROR", "失败")));
    r.travel_gifts.queue.lock().unwrap().attempted.insert(1);
    r.read(|s, _| {
        let state = r.travel_gifts.status(&s.connection, 1)?;
        assert_eq!(state.pending_count, 1);
        assert_eq!(state.failures.len(), 1);
        assert!(r.travel_gifts.status(&s.connection, 2)?.failures.is_empty());
        r.travel_gifts.retry(&s.connection, 2)?;
        assert_eq!(r.travel_gifts.status(&s.connection, 1)?.failures.len(), 1);
        r.travel_gifts.retry(&s.connection, 1)?;
        assert!(r.travel_gifts.status(&s.connection, 1)?.failures.is_empty());
        Ok(())
    })
    .unwrap();
    assert!(r.travel_gifts.queue.lock().unwrap().attempted.contains(&1));
}
