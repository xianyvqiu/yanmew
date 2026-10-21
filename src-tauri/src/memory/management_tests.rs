use super::*;
use crate::ai::tests::{Response, Server};
use serde_json::json;

const GENERATED_SHORT: &str = "用户询问猫的生活习惯，助手说明猫喜欢安静环境并需要规律喂食。";
const OLD_TITLE: &str = "原始长期标题 alphatitle";
const OLD_CONTENT: &str = "原始长期事实 alphabody";

#[test]
fn session_round_numbers_start_at_one_and_long_range_uses_real_sources(){
 let runtime=crate::runtime::test_runtime("http://127.0.0.1:1/v1");
 for n in 0..12{completed_round(&runtime,1,n);}
 let session=runtime.read(|s,_|{let created=crate::sessions::create(&mut s.connection,1)?;let id=parse_id(&created.id)?;s.connection.execute("UPDATE settings SET value=?1 WHERE key='current_session_id'",[id.to_string()])?;Ok(id)}).unwrap();
 let first=completed_round(&runtime,session,1);
  runtime.read(|s,_|{s.connection.execute("INSERT INTO chat_rounds(session_id,status,stop_reason,created_at) VALUES(?1,'interrupted','unknown','a')",[session])?;Ok(())}).unwrap();
 let third=completed_round(&runtime,session,3);let trigger=completed_round(&runtime,session,4);
 let ids=runtime.read(|s,_|{
  let mut ids=vec![];for round in [first,third,trigger]{s.connection.execute("INSERT INTO short_memories(session_id,round_id,content,created_at,updated_at) VALUES(?1,?2,'事实','a','a')",params![session,round])?;ids.push(s.connection.last_insert_rowid());}
  s.connection.execute("INSERT INTO long_memories(session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(?1,'整合','事实',2,?2,'a','a')",params![session,trigger])?;
  let long=s.connection.last_insert_rowid();for (n,id) in ids.iter().take(2).enumerate(){s.connection.execute("INSERT INTO long_memory_sources(long_memory_id,short_memory_id,source_order) VALUES(?1,?2,?3)",params![long,id,n+1])?;}
  Ok((ids,long))
 }).unwrap();
 let cx=Context{persona_id:"1".into(),session_id:session.to_string(),context_revision:0};
 let short=get(&runtime,cx.clone(),Kind::Short,ids.0[0].to_string()).unwrap();assert!(first>12);assert_eq!(short.record.round_number,Some(1));
 let long=get(&runtime,cx,Kind::Long,ids.1.to_string()).unwrap();assert_eq!(long.record.source_start_round_number,Some(1));assert_eq!(long.record.source_end_round_number,Some(3));assert_eq!(long.record.activated_by_round_number,Some(4));assert_eq!(long.sources.iter().map(|s|s.round_number).collect::<Vec<_>>(),vec![Some(1),Some(3)]);
}

struct Fixture {
    runtime: Runtime,
    rounds: [i64; 3],
    shorts: [String; 3],
}

fn context() -> Context {
    Context {
        persona_id: "1".into(),
        session_id: "1".into(),
        context_revision: 0,
    }
}

fn assert_error<T>(result: AppResult<T>, code: &str) {
    match result {
        Err(error) => assert_eq!(error.code, code),
        Ok(_) => panic!("预期错误 {code}，操作却成功"),
    }
}

fn completed_round(runtime: &Runtime, session: i64, number: usize) -> i64 {
    runtime
        .read(|store, _| {
            let round = crate::sessions::start_round(
                &mut store.connection,
                1,
                session,
                &format!("第{number}轮用户原文：讨论猫的生活习惯。"),
            )?;
            let id = parse_id(&round.round.id)?;
            crate::sessions::complete_round(
                &mut store.connection,
                1,
                session,
                id,
                &format!("第{number}轮助手原文：猫需要规律喂食。"),
                None,
            )?;
            Ok(id)
        })
        .unwrap()
}

fn fixture(url: &str) -> Fixture {
    let runtime = crate::runtime::test_runtime(url);
    let rounds = std::array::from_fn(|index| completed_round(&runtime, 1, index + 1));
    // 自增 ID 与回合顺序故意不同，分页必须依据来源回合。
    let shorts = ["9".to_string(), "3".to_string(), "7".to_string()];
    runtime
        .read(|store, _| {
            for (index, round) in rounds.iter().enumerate() {
                store.connection.execute(
                    "INSERT INTO short_memories(id,session_id,round_id,content,integration_state,created_at,updated_at)
                     VALUES(?1,1,?2,?3,?4,'a','a')",
                    params![
                        parse_id(&shorts[index])?,
                        round,
                        format!("第{}轮原始短期事实", index + 1),
                        if index < 2 { "integrated" } else { "pending" }
                    ],
                )?;
            }
            store.connection.execute(
                "INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at)
                 VALUES(1,1,?1,?2,2,?3,'a','a')",
                params![OLD_TITLE, OLD_CONTENT, rounds[2]],
            )?;
            for (index, short) in shorts.iter().take(2).enumerate() {
                store.connection.execute(
                    "INSERT INTO long_memory_sources(long_memory_id,short_memory_id,source_order) VALUES(1,?1,?2)",
                    params![parse_id(short)?, index as i64 + 1],
                )?;
            }
            store.connection.execute(
                "UPDATE sessions SET memory_revision=memory_revision+1 WHERE id=1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    Fixture {
        runtime,
        rounds,
        shorts,
    }
}

fn short_detail(f: &Fixture, index: usize) -> Detail {
    get(&f.runtime, context(), Kind::Short, f.shorts[index].clone()).unwrap()
}

fn long_detail(f: &Fixture) -> Detail {
    get(&f.runtime, context(), Kind::Long, "1".into()).unwrap()
}

fn scalar(runtime: &Runtime, sql: &str) -> i64 {
    runtime
        .read(|store, _| Ok(store.connection.query_row(sql, [], |row| row.get(0))?))
        .unwrap()
}

fn fts_count(runtime: &Runtime, term: &str) -> i64 {
    runtime
        .read(|store, _| {
            Ok(store.connection.query_row(
                "SELECT COUNT(*) FROM long_memories_fts WHERE long_memories_fts MATCH ?1 AND session_id=1",
                [term],
                |row| row.get(0),
            )?)
        })
        .unwrap()
}

fn seed_embedding(runtime: &Runtime) {
    runtime
        .read(|store, _| {
            store.connection.execute(
                "INSERT INTO long_memory_embeddings(long_memory_id,session_id,model_key,dimensions,vector,content_hash,updated_at)
                 VALUES(1,1,'test-model:2',2,X'0000803F00000000','old-hash','a')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn pagination_uses_round_and_trigger_order_with_scoped_sources_and_relations() {
    let f = fixture("http://127.0.0.1:9/v1");
    let first = list(&f.runtime, context(), Kind::Short, None, 2).unwrap();
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>(),
        vec![f.shorts[2].clone(), f.shorts[1].clone()]
    );
    assert_eq!(first.next_after_id, Some(f.shorts[1].clone()));
    let second = list(&f.runtime, context(), Kind::Short, first.next_after_id, 2).unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].id, f.shorts[0]);
    assert_eq!(second.next_after_id, None);
    assert_error(
        list(&f.runtime, context(), Kind::Short, None, 0),
        "VALIDATION_ERROR",
    );
    assert_error(
        list(&f.runtime, context(), Kind::Long, None, 101),
        "VALIDATION_ERROR",
    );

    let long = long_detail(&f);
    assert_eq!(long.record.source_count, Some(2));
    assert_eq!(
        long.record.activated_by_round_id,
        Some(f.rounds[2].to_string())
    );
    assert_eq!(
        long.record.covered_through_round_id,
        Some(f.rounds[1].to_string())
    );
    assert_eq!(
        long.sources
            .iter()
            .map(|source| (source.short_memory_id.clone(), source.source_order))
            .collect::<Vec<_>>(),
        vec![(f.shorts[0].clone(), 1), (f.shorts[1].clone(), 2)]
    );
    assert_eq!(
        short_detail(&f, 0).related_long_memories,
        vec![Related {
            id: "1".into(),
            title: OLD_TITLE.into()
        }]
    );
    assert!(short_detail(&f, 2).related_long_memories.is_empty());

    let next_rounds: [i64; 3] =
        std::array::from_fn(|index| completed_round(&f.runtime, 1, index + 4));
    f.runtime.read(|store, _| {
        for (index, round) in next_rounds.iter().take(2).enumerate() {
            store.connection.execute("INSERT INTO short_memories(id,session_id,round_id,content,integration_state,created_at,updated_at) VALUES(?1,1,?2,'下一组事实','integrated','a','a')", params![20 + index as i64, round])?;
        }
        // 较小 ID 对应较新的触发回合，防止长期列表误按 ID 排序。
        store.connection.execute("DELETE FROM long_memory_sources WHERE long_memory_id=1", [])?;
        store.connection.execute("UPDATE long_memories SET title='下一条长期',content='下一组正文',activated_by_round_id=?1 WHERE id=1", [next_rounds[2]])?;
        store.connection.execute("INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(2,1,?1,?2,2,?3,'a','a')", params![OLD_TITLE, OLD_CONTENT, f.rounds[2]])?;
        store.connection.execute_batch("INSERT INTO long_memory_sources VALUES(1,20,1); INSERT INTO long_memory_sources VALUES(1,21,2);
            INSERT INTO long_memory_sources VALUES(2,9,1); INSERT INTO long_memory_sources VALUES(2,3,2);
            INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'另一会话','a','a');")?;
        Ok(())
    }).unwrap();
    let other_round = completed_round(&f.runtime, 2, 100);
    f.runtime.read(|store, _| {
        store.connection.execute("INSERT INTO short_memories(id,session_id,round_id,content,created_at,updated_at) VALUES(100,2,?1,'其他会话事实','a','a')", [other_round])?;
        store.connection.execute("INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(100,2,'其他会话标题','其他会话正文',1,?1,'a','a')", [other_round])?;
        Ok(())
    }).unwrap();
    let longs = list(&f.runtime, context(), Kind::Long, None, 1).unwrap();
    assert_eq!(
        longs.items[0].activated_by_round_id,
        Some(next_rounds[2].to_string())
    );
    assert_eq!(longs.items[0].id, "1");
    assert_eq!(longs.next_after_id.as_deref(), Some("1"));
    let old = list(&f.runtime, context(), Kind::Long, longs.next_after_id, 1).unwrap();
    assert_eq!(old.items[0].id, "2");
    assert_eq!(old.next_after_id, None);
    for kind in [Kind::Short, Kind::Long] {
        assert_error(get(&f.runtime, context(), kind, "100".into()), "NOT_FOUND");
        assert_error(
            list(&f.runtime, context(), kind, Some("100".into()), 2),
            "NOT_FOUND",
        );
    }
    assert_error(
        get(
            &f.runtime,
            Context {
                session_id: "2".into(),
                ..context()
            },
            Kind::Short,
            "100".into(),
        ),
        "CONTEXT_CHANGED",
    );
    assert_error(
        get(
            &f.runtime,
            Context {
                context_revision: 1,
                ..context()
            },
            Kind::Short,
            f.shorts[0].clone(),
        ),
        "CONTEXT_CHANGED",
    );
}

#[test]
fn editing_integrated_short_preserves_long_and_accepts_manual_lengths_but_rejects_stale_detail() {
    let f = fixture("http://127.0.0.1:9/v1");
    let original = short_detail(&f, 0);
    let long = long_detail(&f);
    let mut current = original.clone();
    for content in [
        "短".to_string(),
        "人工事实".repeat(40),
        "人工\u{0000}记录".to_string(),
    ] {
        let edited = save(
            &f.runtime,
            context(),
            current.clone(),
            None,
            content.clone(),
        )
        .unwrap();
        assert_eq!(edited.record.id, original.record.id);
        assert_eq!(edited.record.round_id, original.record.round_id);
        assert_eq!(
            edited.record.integration_state,
            Some(MemoryIntegrationState::Integrated)
        );
        assert_eq!(edited.record.content, content);
        assert_eq!(edited.memory_revision, current.memory_revision + 1);
        assert_eq!(edited.related_long_memories, original.related_long_memories);
        current = edited;
    }
    assert_error(
        save(&f.runtime, context(), original, None, "旧草稿".into()),
        "MEMORY_CONFLICT",
    );
    for invalid in [" \n\t", ""] {
        assert_error(
            save(&f.runtime, context(), current.clone(), None, invalid.into()),
            "VALIDATION_ERROR",
        );
        assert_eq!(short_detail(&f, 0), current);
    }
    let unchanged_long = long_detail(&f);
    assert_eq!(unchanged_long.record, long.record);
    assert_eq!(unchanged_long.sources.len(), 2);
    assert_eq!(
        unchanged_long.sources[0].short_memory_id,
        long.sources[0].short_memory_id
    );
    assert_eq!(unchanged_long.sources[1], long.sources[1]);
    assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM short_memories"), 3);
    assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM long_memories"), 1);
}

#[test]
fn long_edit_updates_fts_and_invalidates_vector_atomically_and_failure_rolls_everything_back() {
    let f = fixture("http://127.0.0.1:9/v1");
    seed_embedding(&f.runtime);
    let original = long_detail(&f);
    let edited = save(
        &f.runtime,
        context(),
        original.clone(),
        Some("新标题 betatitle".into()),
        "新事实 betabody".into(),
    )
    .unwrap();
    assert_eq!(edited.record.id, original.record.id);
    assert_eq!(edited.record.source_count, original.record.source_count);
    assert_eq!(
        edited.record.activated_by_round_id,
        original.record.activated_by_round_id
    );
    assert_eq!(
        edited.record.covered_through_round_id,
        original.record.covered_through_round_id
    );
    assert_eq!(edited.sources, original.sources);
    assert_eq!(edited.memory_revision, original.memory_revision + 1);
    for old in ["alphatitle", "alphabody"] {
        assert_eq!(fts_count(&f.runtime, old), 0);
    }
    for new in ["betatitle", "betabody"] {
        assert_eq!(fts_count(&f.runtime, new), 1);
    }
    assert_eq!(
        scalar(&f.runtime, "SELECT COUNT(*) FROM long_memory_embeddings"),
        0
    );

    seed_embedding(&f.runtime);
    f.runtime.read(|store, _| {
        store.connection.execute_batch("CREATE TEMP TRIGGER fail_management_revision BEFORE UPDATE OF memory_revision ON sessions
            WHEN new.id=1 BEGIN SELECT RAISE(ABORT,'模拟版本写入失败'); END;")?;
        Ok(())
    }).unwrap();
    assert_error(
        save(
            &f.runtime,
            context(),
            edited.clone(),
            Some("未提交标题 gammatitle".into()),
            "未提交正文 gammabody".into(),
        ),
        "STORAGE_ERROR",
    );
    assert_eq!(long_detail(&f), edited);
    for retained in ["betatitle", "betabody"] {
        assert_eq!(fts_count(&f.runtime, retained), 1);
    }
    for rejected in ["gammatitle", "gammabody"] {
        assert_eq!(fts_count(&f.runtime, rejected), 0);
    }
    assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM long_memory_embeddings WHERE dimensions=2 AND vector=X'0000803F00000000' AND content_hash='old-hash'"), 1);
    assert_eq!(
        scalar(
            &f.runtime,
            "SELECT COUNT(*) FROM long_memory_sources WHERE long_memory_id=1"
        ),
        2
    );
    assert_error(
        save(
            &f.runtime,
            context(),
            edited.clone(),
            Some("多行\n标题".into()),
            "正文".into(),
        ),
        "VALIDATION_ERROR",
    );
    assert_error(
        save(
            &f.runtime,
            context(),
            edited,
            Some(" ".into()),
            "正文".into(),
        ),
        "VALIDATION_ERROR",
    );
}

#[test]
fn short_preview_uses_only_original_messages_then_confirms_edited_content_once_in_place() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(json!({"content": GENERATED_SHORT}))]);
        let f = fixture(&server.url);
        let before = short_detail(&f, 0);
        let original_long = long_detail(&f);
        let generated = preview(
            f.runtime.clone(),
            context(),
            Kind::Short,
            f.shorts[0].clone(),
        )
        .await
        .unwrap();
        assert_eq!(short_detail(&f, 0), before);
        assert_eq!(generated.detail, before);
        assert_eq!(generated.content, GENERATED_SHORT);
        let sent = server.requests.lock().unwrap()[0].clone();
        assert_eq!(sent["messages"].as_array().unwrap().len(), 2);
        assert!(sent["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains(FACT_RULE));
        assert!(sent
            .get("tools")
            .is_none_or(|value| value.is_null() || value.as_array().is_some_and(Vec::is_empty)));
        let payload: serde_json::Value =
            serde_json::from_str(sent["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            payload,
            json!({"user_message":"第1轮用户原文：讨论猫的生活习惯。", "assistant_message":"第1轮助手原文：猫需要规律喂食。"})
        );
        let manual = "人工修改事实".repeat(30);
        let saved = confirm(
            &f.runtime,
            context(),
            generated.preview_id.clone(),
            None,
            manual.clone(),
        )
        .unwrap();
        assert_eq!(saved.record.id, before.record.id);
        assert_eq!(saved.record.content, manual);
        assert_eq!(saved.record.round_id, before.record.round_id);
        assert_eq!(
            saved.record.integration_state,
            before.record.integration_state
        );
        assert_eq!(saved.memory_revision, before.memory_revision + 1);
        assert_eq!(long_detail(&f).record, original_long.record);
        assert_error(
            confirm(
                &f.runtime,
                context(),
                generated.preview_id,
                None,
                "再次确认".into(),
            ),
            "MEMORY_PREVIEW_EXPIRED",
        );
        assert_eq!(short_detail(&f, 0), saved);
        assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM short_memories"), 3);
        server.finish();
    });
}

#[test]
fn long_preview_keeps_original_two_sources_despite_current_k_twenty_and_confirms_title_and_body() {
    tauri::async_runtime::block_on(async {
        let body = "事实".repeat(90);
        let server = Server::new(vec![Response::json(
            json!({"title":"重新整合的中性标题", "content":body}),
        )]);
        let f = fixture(&server.url);
        f.runtime.read(|s,_|{let revision=crate::provider_roles::revision(&s.connection)?;crate::provider_roles::save(&mut s.connection,"memory",Some("1"),"memory-preview-model",true,revision,None,None)?;Ok(())}).unwrap();
        seed_embedding(&f.runtime);
        assert_eq!(
            scalar(
                &f.runtime,
                "SELECT CAST(value AS INTEGER) FROM settings WHERE key='memory_merge_count'"
            ),
            20
        );
        let before = long_detail(&f);
        let generated = preview(f.runtime.clone(), context(), Kind::Long, "1".into())
            .await
            .unwrap();
        assert_eq!(long_detail(&f), before);
        assert_eq!(
            scalar(&f.runtime, "SELECT COUNT(*) FROM long_memory_embeddings"),
            1
        );
        assert_eq!(generated.title.as_deref(), Some("重新整合的中性标题"));
        assert_eq!(generated.content, body);
        let sent = server.requests.lock().unwrap()[0].clone();
        assert_eq!(sent["model"],"memory-preview-model");
        assert_eq!(sent["messages"].as_array().unwrap().len(), 2);
        assert!(sent["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains(FACT_RULE));
        let payload: serde_json::Value =
            serde_json::from_str(sent["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            payload,
            json!({"source_memories":["第1轮原始短期事实", "第2轮原始短期事实"]})
        );
        let saved = confirm(
            &f.runtime,
            context(),
            generated.preview_id,
            Some("人工标题 deltatitle".into()),
            "短".into(),
        )
        .unwrap();
        assert_eq!(saved.record.id, "1");
        assert_eq!(saved.record.title.as_deref(), Some("人工标题 deltatitle"));
        assert_eq!(saved.record.content, "短");
        assert_eq!(saved.record.source_count, Some(2));
        assert_eq!(
            saved.record.activated_by_round_id,
            before.record.activated_by_round_id
        );
        assert_eq!(
            saved.record.covered_through_round_id,
            before.record.covered_through_round_id
        );
        assert_eq!(saved.sources, before.sources);
        assert_eq!(saved.memory_revision, before.memory_revision + 1);
        assert_eq!(fts_count(&f.runtime, "deltatitle"), 1);
        assert_eq!(fts_count(&f.runtime, "alphatitle"), 0);
        assert_eq!(
            scalar(&f.runtime, "SELECT COUNT(*) FROM long_memory_embeddings"),
            0
        );
        assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM long_memories"), 1);
        server.finish();
    });
}

#[test]
fn confirmation_detects_changed_sources_and_original_messages_even_without_revision_bump() {
    tauri::async_runtime::block_on(async {
        let changes = [
            (Kind::Long, "UPDATE short_memories SET content='已变化的原来源' WHERE id=9"),
            (Kind::Long, "UPDATE long_memory_sources SET source_order=10 WHERE short_memory_id=9;
                UPDATE long_memory_sources SET source_order=1 WHERE short_memory_id=3;
                UPDATE long_memory_sources SET source_order=2 WHERE short_memory_id=9;"),
            (Kind::Short, "UPDATE messages SET content='用户原文已编辑' WHERE session_id=1 AND round_id=1 AND role='user'"),
            (Kind::Short, "UPDATE messages SET content='助手原文已编辑' WHERE session_id=1 AND round_id=1 AND role='assistant'"),
        ];
        let responses = changes
            .iter()
            .map(|(kind, _)| match kind {
                Kind::Long => {
                    Response::json(json!({"title":"中性事实标题", "content":"事实".repeat(90)}))
                }
                Kind::Short => Response::json(json!({"content":GENERATED_SHORT})),
            })
            .collect();
        let server = Server::new(responses);
        for (kind, sql) in changes {
            let f = fixture(&server.url);
            let id = if kind == Kind::Long {
                "1".into()
            } else {
                f.shorts[0].clone()
            };
            let before = get(&f.runtime, context(), kind, id.clone()).unwrap();
            let generated = preview(f.runtime.clone(), context(), kind, id.clone())
                .await
                .unwrap();
            f.runtime
                .read(|store, _| {
                    store.connection.execute_batch(sql)?;
                    Ok(())
                })
                .unwrap();
            let changed = get(&f.runtime, context(), kind, id.clone()).unwrap();
            assert_eq!(changed.memory_revision, before.memory_revision);
            assert_error(
                confirm(
                    &f.runtime,
                    context(),
                    generated.preview_id,
                    generated.title,
                    generated.content,
                ),
                "MEMORY_PREVIEW_EXPIRED",
            );
            assert_eq!(
                get(&f.runtime, context(), kind, id).unwrap().record,
                changed.record
            );
            assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM long_memories"), 1);
            assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM short_memories"), 3);
        }
        server.finish();
    });
}

#[test]
fn cancellation_and_invalid_model_outputs_leave_original_memory_and_no_confirmable_preview() {
    tauri::async_runtime::block_on(async {
        let mut delayed = Response::json(json!({"content":GENERATED_SHORT}));
        delayed.chunks[0].0 = 300;
        let server = Server::new(vec![delayed]);
        let f = fixture(&server.url);
        let before = short_detail(&f, 0);
        let runtime = f.runtime.clone();
        let id = f.shorts[0].clone();
        let task = tauri::async_runtime::spawn(async move {
            preview(runtime, context(), Kind::Short, id).await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while server.requests.lock().unwrap().is_empty() {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("假服务应已收到正在生成的预览请求");
        cancel(&f.runtime, context()).unwrap();
        assert_error(task.await.unwrap(), "CANCELLED");
        assert_eq!(short_detail(&f, 0), before);
        assert!(f.runtime.memory.preview.lock().unwrap().is_none());
        assert!(!f.runtime.requests.is_active("memory-preview").unwrap());
        server.finish();

        let truncated = Response {
            status: 200,
            mime: "application/json",
            chunks: vec![(0, serde_json::to_vec(&json!({"choices":[{"message":{"content":json!({"content":GENERATED_SHORT}).to_string()},"finish_reason":"length"}]})).unwrap())],
        };
        let server = Server::new(vec![
            Response::json(json!({"content":"太短"})),
            Response::json(json!({"content":"汉".repeat(101)})),
            Response::json(json!({"content":GENERATED_SHORT,"extra":"不允许的字段"})),
            truncated,
        ]);
        let f = fixture(&server.url);
        let before = short_detail(&f, 0);
        for _ in 0..4 {
            assert!(preview(
                f.runtime.clone(),
                context(),
                Kind::Short,
                f.shorts[0].clone()
            )
            .await
            .is_err());
            assert_eq!(short_detail(&f, 0), before);
            assert!(f.runtime.memory.preview.lock().unwrap().is_none());
            assert!(!f.runtime.requests.is_active("memory-preview").unwrap());
        }
        assert_eq!(scalar(&f.runtime, "SELECT COUNT(*) FROM short_memories"), 3);
        assert_eq!(long_detail(&f).record.title.as_deref(), Some(OLD_TITLE));
        server.finish();
    });
}
