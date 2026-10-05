use super::*;

pub(crate) fn seed(runtime: &Runtime, session: i64, title: &str, content: &str) -> i64 {
    runtime.read(|store,_| {
        let round = crate::sessions::start_round(&mut store.connection,1,session,"原始问题")?;
        let round_id = crate::error::parse_id(&round.round.id)?;
        crate::sessions::complete_round(&mut store.connection,1,session,round_id,"原始回复",None)?;
        store.connection.execute("INSERT INTO long_memories(session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(?1,?2,?3,1,?4,'a','a')", params![session,title,content,round_id])?;
        Ok(store.connection.last_insert_rowid())
    }).unwrap()
}
pub(crate) fn online(runtime: &Runtime, url: &str) -> i64 {
    let id = runtime
        .read(|store, secrets| {
            let saved = embedding_providers::save(
                &mut store.connection,
                None,
                crate::providers::ProviderInput {
                    max_output_tokens: 32768,
                    context_window_override: None,
                    name: "向量服务".into(),
                    base_url: Some(url.into()),
                    model: "model".into(),
                    api_key: crate::crypto::SecretUpdate::Keep,
                },
                secrets,
            )?;
            let id = crate::error::parse_id(&saved.id)?;
            embedding_providers::select(&mut store.connection, id)?;
            Ok(id)
        })
        .unwrap();
    runtime.memory_index.reset().unwrap();
    id
}
fn vector(runtime: &Runtime, stamp: &Stamp) -> Vector {
    Vector {
        model_key: format!(
            "{}:2",
            embedding::identity(&runtime.embedding, stamp).unwrap()
        ),
        values: vec![1.0, 0.0],
    }
}
#[test]
fn cache_commit_rejects_changed_content_provider_epoch_and_deleted_memory() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    online(&runtime, "http://127.0.0.1:1/v1");
    let id = seed(&runtime, 1, "旧记忆", "用户在杭州旅行，带回桂花糕。");
    let task = next(&runtime).unwrap().unwrap();
    assert!(commit(&runtime, &task, vector(&runtime, &task.stamp)).unwrap());
    assert!(next(&runtime).unwrap().is_none());
    runtime
        .read(|s, _| {
            s.connection
                .execute("UPDATE long_memories SET title='新标题' WHERE id=?1", [id])?;
            Ok(())
        })
        .unwrap();
    assert!(!commit(&runtime, &task, vector(&runtime, &task.stamp)).unwrap());
    let updated = next(&runtime).unwrap().unwrap();
    runtime.memory_index.reset().unwrap();
    assert!(!commit(&runtime, &updated, vector(&runtime, &updated.stamp)).unwrap());
    let updated = next(&runtime).unwrap().unwrap();
    runtime
        .read(|s, _| {
            s.connection.execute(
                "UPDATE embedding_providers SET model='other' WHERE is_active=1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(!commit(&runtime, &updated, vector(&runtime, &updated.stamp)).unwrap());
    let latest = next(&runtime).unwrap().unwrap();
    runtime
        .read(|s, _| {
            s.connection
                .execute("DELETE FROM long_memories WHERE id=?1", [id])?;
            Ok(())
        })
        .unwrap();
    assert!(!commit(&runtime, &latest, vector(&runtime, &latest.stamp)).unwrap());
    assert_eq!(view(&runtime).unwrap().total, 0);
}
#[test]
fn unusable_vectors_are_not_considered_ready_and_current_session_is_first() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    online(&runtime, "http://127.0.0.1:1/v1");
    runtime.read(|s,_| {s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'其他','a','a')",[])?;Ok(())}).unwrap();
    seed(&runtime, 2, "其他会话", "正文");
    let id = seed(&runtime, 1, "当前会话", "正文");
    let task = next(&runtime).unwrap().unwrap();
    assert_eq!(task.item.id, id);
    assert!(commit(&runtime, &task, vector(&runtime, &task.stamp)).unwrap());
    runtime
        .read(|s, _| {
            s.connection.execute(
                "UPDATE long_memory_embeddings SET vector=zeroblob(8) WHERE long_memory_id=?1",
                [id],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(view(&runtime).unwrap().ready, 0);
    assert_eq!(next(&runtime).unwrap().unwrap().item.id, id);
}
