//! 当前会话较旧长期记忆的 FTS5/向量混合检索，供 9.6 上下文及工具复用。
use crate::{
    embedding, embedding_providers,
    error::{AppError, AppResult},
    runtime::Runtime,
    state::Cancellation,
};
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;

const CANDIDATES: usize = 40;
const RESULTS: usize = 6;
const BODY_BUDGET: usize = 2400;
// BGE 中文小样本校准：相关改写最低约 0.60；日常无关内容低于 0.50。
const SEMANTIC_THRESHOLD: f32 = 0.60;
#[derive(Clone)]
struct Memory {
    id: i64,
    title: String,
    content: String,
    vector: Option<(String, usize, Vec<u8>, String)>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub id: String,
    pub title: String,
    pub content: String,
    pub matched_by: Vec<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub fallback_error: Option<AppError>,
}
fn terms(query: &str) -> Vec<String> {
    let mut result = Vec::new();
    for part in query.split(|c: char| !c.is_alphanumeric()) {
        let chars = part.chars().take(128).collect::<Vec<_>>();
        if chars.len() >= 3 {
            for window in chars.windows(3) {
                result.push(window.iter().collect());
            }
        } else if !chars.is_empty() {
            result.push(chars.iter().collect());
        }
    }
    result.sort();
    result.dedup();
    result.truncate(128);
    result
}
fn lexical(runtime: &Runtime, session: i64, recent: usize, query: &str) -> AppResult<Vec<i64>> {
    let terms = terms(query);
    if terms.is_empty() {
        return Ok(vec![]);
    }
    runtime.read(|store,_| {
        let full = terms.iter().filter(|t| t.chars().count() >= 3).map(|t| format!("\"{}\"",t.replace('"',"\"\""))).collect::<Vec<_>>().join(" OR ");
        let mut found = Vec::new();
        if !full.is_empty() {
            let mut statement = store.connection.prepare("SELECT rowid FROM long_memories_fts WHERE long_memories_fts MATCH ?1 AND session_id=?2 AND rowid NOT IN (SELECT id FROM long_memories WHERE session_id=?2 ORDER BY activated_by_round_id DESC,id DESC LIMIT ?3) ORDER BY bm25(long_memories_fts,2.0,1.0) LIMIT ?4")?;
            let rows = statement.query_map(params![full,session,recent,CANDIDATES],|r|r.get(0))?;
            found = rows.collect::<Result<Vec<i64>,_>>()?;
        }
        // 少于三字不能走 trigram MATCH；绑定参数并按字面子串匹配，不解释通配符。
        for term in terms.iter().filter(|t| t.chars().count() < 3) {
            let mut statement = store.connection.prepare("SELECT id FROM long_memories WHERE session_id=?1 AND (instr(title,?2)>0 OR instr(content,?2)>0) AND id NOT IN (SELECT id FROM long_memories WHERE session_id=?1 ORDER BY activated_by_round_id DESC,id DESC LIMIT ?3) ORDER BY activated_by_round_id DESC,id DESC LIMIT ?4")?;
            for id in statement.query_map(params![session,term,recent,CANDIDATES],|r|r.get::<_,i64>(0))? { let id = id?; if !found.contains(&id) { found.push(id); } }
        }
        found.truncate(CANDIDATES);
        Ok(found)
    })
}
fn snapshot(runtime: &Runtime, session: i64, recent: usize) -> AppResult<Vec<Memory>> {
    runtime.read(|store,_| {
        let mut statement = store.connection.prepare("SELECT l.id,l.title,l.content,e.model_key,e.dimensions,e.vector,e.content_hash FROM long_memories l LEFT JOIN long_memory_embeddings e ON e.long_memory_id=l.id AND e.session_id=l.session_id WHERE l.session_id=?1 AND l.id NOT IN (SELECT id FROM long_memories WHERE session_id=?1 ORDER BY activated_by_round_id DESC,id DESC LIMIT ?2) ORDER BY l.id DESC")?;
        let rows = statement.query_map(params![session,recent],|r| {
            let key: Option<String> = r.get(3)?;
            Ok(Memory {id:r.get(0)?,title:r.get(1)?,content:r.get(2)?,vector: if let Some(key)=key {Some((key,r.get(4)?,r.get(5)?,r.get(6)?))}else{None}})
        })?;
        Ok(rows.collect::<Result<_,_>>()?)
    })
}
pub async fn search(
    runtime: &Runtime,
    session: i64,
    query: &str,
    recent: usize,
    cancel: &Cancellation,
) -> AppResult<SearchResult> {
    search_impl(runtime, session, query, recent, cancel, true).await
}
/// 深层搜索只返回线索，不因待打开的正文较长而从候选中排除。
pub async fn search_metadata(runtime:&Runtime,session:i64,query:&str,cancel:&Cancellation)->AppResult<SearchResult> {
    search_impl(runtime,session,query,0,cancel,false).await
}
async fn search_impl(runtime:&Runtime,session:i64,query:&str,recent:usize,cancel:&Cancellation,include_body:bool)->AppResult<SearchResult> {
    if query.trim().is_empty() {
        return Ok(SearchResult {
            hits: vec![],
            fallback_error: None,
        });
    }
    if query.chars().count() > 4096 {
        return Err(AppError::validation("query", "记忆检索文字过长"));
    }
    cancel.check()?;
    let worker = runtime.clone();
    let memories = tauri::async_runtime::spawn_blocking(move || snapshot(&worker, session, recent))
        .await
        .map_err(|_| AppError::storage())??;
    if memories.is_empty() {
        return Ok(SearchResult {
            hits: vec![],
            fallback_error: None,
        });
    }
    let stamp = runtime.read(|store, _| embedding_providers::stamp(&store.connection, None))?;
    let worker = runtime.clone();
    let text = query.to_owned();
    let lexical = async move {
        tauri::async_runtime::spawn_blocking(move || lexical(&worker, session, recent, &text))
            .await
            .map_err(|_| AppError::storage())?
    };
    let semantic = embedding::generate(runtime, &stamp, query.to_owned(), true, cancel);
    let (keywords, vector) = tokio::join!(lexical, semantic);
    let keywords = keywords?;
    cancel.check()?;
    if runtime.read(|store, _| embedding_providers::stamp(&store.connection, None))? != stamp {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "Embedding 配置已切换，请重新检索",
        ));
    }
    let mut similar = Vec::new();
    let mut fallback_error = None;
    match vector {
        Ok(query) => {
            runtime.memory_index.observed(&query.model_key)?;
            for memory in &memories {
                if let Some((key, dimensions, bytes, hash)) = &memory.vector {
                    if *key != query.model_key
                        || *dimensions != query.values.len()
                        || *hash != embedding::content_hash(&memory.title, &memory.content)
                    {
                        continue;
                    }
                    if let Ok(values) = embedding::decode(bytes, *dimensions) {
                        let similarity = values
                            .iter()
                            .zip(&query.values)
                            .map(|(a, b)| a * b)
                            .sum::<f32>();
                        if similarity >= SEMANTIC_THRESHOLD {
                            similar.push((memory.id, similarity));
                        }
                    }
                }
            }
            similar.sort_by(|a, b| b.1.total_cmp(&a.1));
            similar.truncate(CANDIDATES);
        }
        Err(error) if error.code == "CANCELLED" => return Err(error),
        Err(error) => {
            if error.code != "EMBEDDING_TOO_LONG" {
                runtime.memory_index.report(error.clone());
            }
            fallback_error = Some(error);
        }
    }
    // RRF 不把 BM25 与余弦的不同量纲直接相加，先逐通道相关性筛选再融合。
    let mut scores: HashMap<i64, (f64, Vec<String>)> = HashMap::new();
    for (channel, ids) in [
        ("keyword", keywords),
        ("semantic", similar.into_iter().map(|(id, _)| id).collect()),
    ] {
        for (rank, id) in ids.into_iter().enumerate() {
            let score = scores.entry(id).or_default();
            score.0 += 1.0 / (60.0 + rank as f64 + 1.0);
            score.1.push(channel.into());
        }
    }
    let mut ranked = scores.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0).then_with(|| b.0.cmp(&a.0)));
    let mut used = 0;
    let mut hits = Vec::new();
    for (id, (_, matched_by)) in ranked {
        if hits.len() == RESULTS {
            break;
        }
        if let Some(memory) = memories.iter().find(|m| m.id == id) {
            let size = memory.title.chars().count() + if include_body { memory.content.chars().count() } else {0};
            if used + size > BODY_BUDGET {
                continue;
            }
            used += size;
            let unchanged = runtime.read(|store, _| {
                use rusqlite::OptionalExtension;
                let row = store
                    .connection
                    .query_row(
                        "SELECT title,content FROM long_memories WHERE id=?1 AND session_id=?2",
                        params![id, session],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    )
                    .optional()?;
                Ok(row.is_some_and(|(title, content)| {
                    title == memory.title && content == memory.content
                }))
            })?;
            if unchanged {
                hits.push(Hit {
                    id: id.to_string(),
                    title: memory.title.clone(),
                    content: if include_body {memory.content.clone()} else {String::new()},
                    matched_by,
                });
            }
        }
    }
    Ok(SearchResult {
        hits,
        fallback_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_index::tests::seed;
    #[test]
    fn lexical_scope_short_terms_escaping_edits_deletes_and_rebuild() {
        let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
        runtime.read(|s,_| {s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'其他','a','a')",[])?;Ok(())}).unwrap();
        let first = seed(&runtime, 1, "旅行游记", "去杭州西湖游玩，带回桂花糕。");
        let latest = seed(&runtime, 1, "最近旅行", "也去杭州西湖。");
        seed(&runtime, 2, "其他会话", "杭州西湖");
        assert_eq!(lexical(&runtime, 1, 1, "桂花糕").unwrap(), vec![first]);
        assert_eq!(lexical(&runtime, 1, 1, "杭州").unwrap(), vec![first]);
        assert_eq!(
            lexical(&runtime, 1, 0, "西湖 \" OR * NOT").unwrap().len(),
            2
        );
        runtime
            .read(|s, _| {
                s.connection.execute(
                    "UPDATE long_memories SET content='去北京故宫游玩' WHERE id=?1",
                    [first],
                )?;
                s.connection
                    .execute("DELETE FROM long_memories WHERE id=?1", [latest])?;
                s.connection.execute(
                    "INSERT INTO long_memories_fts(long_memories_fts) VALUES('rebuild')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(lexical(&runtime, 1, 0, "杭州").unwrap().is_empty());
        assert_eq!(lexical(&runtime, 1, 0, "北京故宫").unwrap(), vec![first]);
    }
    #[test]
    fn missing_local_resources_fall_back_without_changing_chat_or_pet() {
        let mut runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
        runtime.embedding = std::sync::Arc::new(embedding::Engine::new(Ok(
            std::path::PathBuf::from("missing-model-test"),
        )));
        let id = seed(&runtime, 1, "杭州旅行", "去了西湖并买了桂花糕。");
        let result = tauri::async_runtime::block_on(search(
            &runtime,
            1,
            "桂花糕",
            0,
            &Cancellation::default(),
        ))
        .unwrap();
        assert_eq!(result.hits[0].id, id.to_string());
        assert!(result.fallback_error.is_some());
        assert_eq!(
            runtime
                .read(|s, _| Ok(crate::providers::list(&s.connection)?[0].is_active))
                .unwrap(),
            true
        );
        assert!(tauri::async_runtime::block_on(search(
            &runtime,
            1,
            "新消息",
            1,
            &Cancellation::default()
        ))
        .unwrap()
        .hits
        .is_empty());
    }
    #[test]
    fn online_hybrid_fuses_deduplicates_and_falls_back_on_http_error() {
        use std::io::{Read, Write};
        for success in [true, false] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .unwrap();
                let mut bytes = [0; 8192];
                let count = stream.read(&mut bytes).unwrap();
                assert!(
                    String::from_utf8_lossy(&bytes[..count]).starts_with("POST /v1/embeddings ")
                );
                let (status, body) = if success {
                    ("200 OK", r#"{"data":[{"index":0,"embedding":[1,0]}]}"#)
                } else {
                    ("503 Service Unavailable", "secret upstream error")
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            });
            let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
            crate::memory_index::tests::online(&runtime, &format!("http://{address}/v1"));
            let id = seed(&runtime, 1, "杭州旧旅行", "去了西湖，带回桂花糕。");
            seed(&runtime, 1, "最新记忆", "不加入检索");
            let stamp = runtime
                .read(|s, _| embedding_providers::stamp(&s.connection, None))
                .unwrap();
            let key = format!(
                "{}:2",
                embedding::identity(&runtime.embedding, &stamp).unwrap()
            );
            runtime
                .read(|s, _| {
                    s.connection.execute(
                        "INSERT INTO long_memory_embeddings VALUES(?1,1,?2,2,?3,?4,'a')",
                        params![
                            id,
                            key,
                            embedding::encode(&[1.0, 0.0]),
                            embedding::content_hash("杭州旧旅行", "去了西湖，带回桂花糕。")
                        ],
                    )?;
                    Ok(())
                })
                .unwrap();
            let result = tauri::async_runtime::block_on(search(
                &runtime,
                1,
                "桂花糕",
                1,
                &Cancellation::default(),
            ))
            .unwrap();
            assert_eq!(result.hits.len(), 1);
            assert_eq!(result.hits[0].id, id.to_string());
            assert_eq!(result.hits[0].matched_by.len(), if success { 2 } else { 1 });
            assert_eq!(result.fallback_error.is_none(), success);
            server.join().unwrap();
        }
    }
}
