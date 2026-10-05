use super::*;
use crate::ai::{
    provider::Choice,
    tests::{Response, Server},
};
use serde_json::json;

fn runtime(url: &str) -> Runtime {
    let runtime = crate::runtime::test_runtime(url);
    runtime
        .read(|store, _| {
            store
                .connection
                .execute("UPDATE sessions SET title='新会话'", [])?;
            Ok(())
        })
        .unwrap();
    runtime
}

fn completed(runtime: &Runtime, content: &str) -> i64 {
    runtime
        .read(|store, _| {
            let round = start_round(&mut store.connection, 1, 1, content)?;
            let id = round.round.id.parse::<i64>().unwrap();
            complete_round(&mut store.connection, 1, 1, id, "已保存回复", None)?;
            Ok(id)
        })
        .unwrap()
}

fn title(runtime: &Runtime) -> String {
    runtime
        .read(|store, _| Ok(get(&store.connection, 1, 1)?.title))
        .unwrap()
}

#[test]
fn incomplete_later_and_manually_named_rounds_do_not_start_title_requests() {
    let runtime = runtime("http://127.0.0.1:9/v1");
    let pending = runtime
        .read(|s, _| start_round(&mut s.connection, 1, 1, "未完整"))
        .unwrap();
    let id = pending.round.id.parse().unwrap();
    assert!(capture(&runtime, 1, 1, 0, id).unwrap().is_none());
    runtime
        .read(|s, _| interrupt_round(&mut s.connection, 1, 1, id, None, None))
        .unwrap();
    assert!(capture(&runtime, 1, 1, 0, id).unwrap().is_none());
    let first = completed(&runtime, "完整问题");
    let second = completed(&runtime, "后续问题");
    assert!(capture(&runtime, 1, 1, 0, second).unwrap().is_none());
    runtime
        .read(|s, _| rename(&mut s.connection, 1, 1, "我的标题"))
        .unwrap();
    assert!(capture(&runtime, 1, 1, 0, first).unwrap().is_none());
    assert!(!runtime.requests.is_active("session-title:1").unwrap());
}

#[test]
fn title_output_must_be_complete_short_single_line_json() {
    let choice = |text: String, finish| Choice {
        index: 0,
        content: text,
        tools: vec![],
        finish,
    };
    assert_eq!(
        parse_title(&choice(
            json!({"title":"  周末的一顿饭  "}).to_string(),
            Finish::Stop
        ))
        .unwrap(),
        "周末的一顿饭"
    );
    for value in [
        "一个原始文本标题".to_string(),
        json!({"title":""}).to_string(),
        json!({"title":"喵".repeat(13)}).to_string(),
        json!({"title":"第一行\n第二行"}).to_string(),
        json!({"title":"标题","other":"多余字段"}).to_string(),
    ] {
        assert_eq!(
            parse_title(&choice(value, Finish::Stop)).unwrap_err().code,
            "TITLE_INVALID_OUTPUT"
        );
    }
    assert!(parse_title(&choice(
        json!({"title":"合法标题"}).to_string(),
        Finish::Length
    ))
    .is_err());
}

#[test]
fn first_completed_reply_uses_first_user_message_and_generates_once() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(json!({"title":"猫与周末计划"}))]);
        let runtime = runtime(&server.url);
        let partial = runtime
            .read(|s, _| start_round(&mut s.connection, 1, 1, "最早用户消息"))
            .unwrap();
        let interrupted = partial.round.id.parse().unwrap();
        runtime
            .read(|s, _| interrupt_round(&mut s.connection, 1, 1, interrupted, None, None))
            .unwrap();
        let first = completed(&runtime, "首次完整轮用户消息");
        let before = runtime.read(|s, _| get(&s.connection, 1, 1)).unwrap();
        assert!(generate_first_title(runtime.clone(), 1, 1, 0, first)
            .await
            .unwrap());
        assert_eq!(title(&runtime), "猫与周末计划");
        let after = runtime.read(|s, _| get(&s.connection, 1, 1)).unwrap();
        assert_eq!(after.memory_revision, before.memory_revision);
        assert_eq!(after.intimacy, before.intimacy);
        assert!(!generate_first_title(runtime.clone(), 1, 1, 0, first)
            .await
            .unwrap());
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0]["messages"][1]["content"],
            json!({"first_user_message":"最早用户消息"}).to_string()
        );
        assert_eq!(requests[0]["max_tokens"], 256);
        drop(requests);
        server.finish();
    });
}

#[test]
fn late_title_cannot_overwrite_renamed_changed_deleted_or_switched_sources() {
    for action in ["rename", "source", "delete", "context", "provider"] {
        let runtime = runtime("http://127.0.0.1:9/v1");
        let round = completed(&runtime, "原始问题");
        let (snapshot, lease) = capture(&runtime, 1, 1, 0, round).unwrap().unwrap();
        runtime
            .read(|s, _| {
                match action {
                    "rename" => {
                        rename(&mut s.connection, 1, 1, "用户手动名称")?;
                    }
                    "source" => {
                        s.connection.execute(
                            "UPDATE messages SET content='编辑后的来源' WHERE id=?1",
                            [snapshot.user_id],
                        )?;
                    }
                    "delete" => {
                        delete(&mut s.connection, 1, 1)?;
                    }
                    "context" => {
                        s.context_revision += 1;
                    }
                    "provider" => {
                        s.connection
                            .execute("UPDATE providers SET model='changed-model'", [])?;
                    }
                    _ => unreachable!(),
                }
                Ok(())
            })
            .unwrap();
        assert!(
            !commit(&runtime, &snapshot, &lease, "迟到标题").unwrap(),
            "{action}"
        );
    }
}

#[test]
fn cancelled_title_does_not_overwrite_manual_default_name() {
    let runtime = runtime("http://127.0.0.1:9/v1");
    let round = completed(&runtime, "原始问题");
    let (snapshot, lease) = capture(&runtime, 1, 1, 0, round).unwrap().unwrap();
    runtime
        .read(|s, _| {
            rename(&mut s.connection, 1, 1, "新会话")?;
            runtime.requests.cancel_key("session-title:1");
            Ok(())
        })
        .unwrap();
    assert_eq!(
        commit(&runtime, &snapshot, &lease, "迟到标题")
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    assert_eq!(title(&runtime), "新会话");
}

#[test]
fn failed_or_invalid_title_preserves_completed_chat_and_pet_health() {
    tauri::async_runtime::block_on(async {
        for response in [Response {status:500,mime:"application/json",chunks:vec![(0,b"{}".to_vec())]},
            Response::json(json!({"title":""})),
            Response {status:200,mime:"application/json",chunks:vec![(0,json!({"choices":[{"message":{"content":"{\"title\":\"截断标题\"}"},"finish_reason":"length"}]}).to_string().into_bytes())]}] {
            let server = Server::new(vec![response]);
            let runtime = runtime(&server.url);
            let round = completed(&runtime,"原始问题");
            runtime.pet_view().unwrap();
            assert!(generate_first_title(runtime.clone(),1,1,0,round).await.is_err());
            assert_eq!(title(&runtime),"新会话");
            runtime.read(|s,_| {
                assert_eq!(round_snapshot(&s.connection,1,1,round)?.round.status,crate::models::ChatRoundStatus::Completed);
                assert_eq!(get(&s.connection,1,1)?.intimacy,1);
                s.pet.require_ai()?;
                Ok(())
            }).unwrap();
            assert!(!runtime.requests.is_active("session-title:1").unwrap());
            server.finish();
        }
    });
}
