//! 当前聊天请求可用的只读旅行游记工具。
use crate::{
    ai::provider::ToolCall,
    error::{AppError, AppResult},
    runtime::{Runtime, Store},
    state::RequestLease,
};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

pub const LIST_TOOL: &str = "list_travel_journals";
pub const SEARCH_TOOL: &str = "search_travel_journals";
pub const OPEN_TOOL: &str = "open_travel_journal";
const DEFAULT_LIMIT: u32 = 20;
const MAX_LIMIT: u32 = 50;

pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{
            "name":LIST_TOOL,
            "description":"分页查看当前会话的旅行游记，只返回游记 ID、标题和生成时间。",
            "parameters":{"type":"object","properties":{
                "limit":{"type":"integer","minimum":1,"maximum":50,"default":20},
                "offset":{"type":"integer","minimum":0,"default":0}
            },"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name":SEARCH_TOOL,
            "description":"在当前会话的旅行游记标题和正文中搜索关键词，只返回游记 ID、标题和生成时间。",
            "parameters":{"type":"object","properties":{
                "query":{"type":"string","minLength":1},
                "limit":{"type":"integer","minimum":1,"maximum":50,"default":20},
                "offset":{"type":"integer","minimum":0,"default":0}
            },"required":["query"],"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name":OPEN_TOOL,
            "description":"读取当前会话中指定旅行游记的完整标题和正文。",
            "parameters":{"type":"object","properties":{
                "journal_id":{"type":"string","pattern":"^[1-9][0-9]*$"}
            },"required":["journal_id"],"additionalProperties":false}
        }}),
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageArguments {
    limit: Option<u32>,
    offset: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArguments {
    query: String,
    limit: Option<u32>,
    offset: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenArguments {
    journal_id: String,
}

fn invalid_arguments() -> AppError {
    AppError::validation("arguments", "旅行游记工具参数无效")
}

fn page(arguments: &Value) -> AppResult<(u32, i64)> {
    let args: PageArguments =
        serde_json::from_value(arguments.clone()).map_err(|_| invalid_arguments())?;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let offset = args.offset.unwrap_or(0);
    if !(1..=MAX_LIMIT).contains(&limit) || offset > i64::MAX as u64 {
        return Err(invalid_arguments());
    }
    Ok((limit, offset as i64))
}

fn search_page(arguments: &Value) -> AppResult<(String, u32, i64)> {
    let args: SearchArguments =
        serde_json::from_value(arguments.clone()).map_err(|_| invalid_arguments())?;
    let query = args.query.trim();
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT);
    let offset = args.offset.unwrap_or(0);
    if query.is_empty()
        || query.chars().count() > 200
        || !(1..=MAX_LIMIT).contains(&limit)
        || offset > i64::MAX as u64
    {
        return Err(invalid_arguments());
    }
    Ok((query.to_owned(), limit, offset as i64))
}

/// 列表与搜索仅投影展示元数据，正文只由 open 工具按需读取。
fn metadata_query(
    store: &Store,
    session_id: i64,
    query: Option<&str>,
    limit: u32,
    offset: i64,
) -> AppResult<Vec<Value>> {
    let mut statement = store.connection.prepare(
        "SELECT id,title,created_at FROM travel_journals
         WHERE session_id=?1 AND (?2 IS NULL OR instr(title,?2)>0 OR instr(content,?2)>0)
         ORDER BY created_at DESC,id DESC LIMIT ?3 OFFSET ?4",
    )?;
    let rows = statement.query_map(rusqlite::params![session_id, query, i64::from(limit), offset], |row| {
        Ok(json!({"journal_id": row.get::<_, i64>(0)?.to_string(), "title": row.get::<_, String>(1)?, "created_at": row.get::<_, String>(2)?}))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// 每次查询都重新校验租约和固定上下文，避免迟到工具读取切换后的会话。
pub struct TravelJournalTools {
    runtime: Runtime,
    request: Arc<RequestLease>,
    persona_id: i64,
    session_id: i64,
    context_revision: u64,
}

impl TravelJournalTools {
    pub fn new(
        runtime: &Runtime,
        request: &Arc<RequestLease>,
        persona_id: i64,
        session_id: i64,
        context_revision: u64,
    ) -> AppResult<Self> {
        let handler = Self {
            runtime: runtime.clone(),
            request: request.clone(),
            persona_id,
            session_id,
            context_revision,
        };
        runtime.read(|store, _| handler.require_context(store))?;
        Ok(handler)
    }

    fn require_context(&self, store: &Store) -> AppResult<()> {
        self.request.require_conversation_scope(
            &self.runtime.requests,
            self.session_id,
            self.persona_id,
        )?;
        self.runtime.jobs_cancel.check()?;
        let context = crate::persona::context(&store.connection, store.context_revision)?;
        if context.persona_id != self.persona_id.to_string()
            || context.session_id.as_deref() != Some(self.session_id.to_string().as_str())
            || context.context_revision != self.context_revision
        {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前人设或会话已经改变，不能继续读取旅行游记",
            ));
        }
        crate::sessions::get(&store.connection, self.persona_id, self.session_id)?;
        Ok(())
    }

    /// 只读查询在后台阻塞执行器中调用；详情每次从数据库读取，不缓存已删除记录。
    pub fn execute(&mut self, call: &ToolCall) -> AppResult<Value> {
        if call.id.trim().is_empty()
            || !matches!(call.name.as_str(), LIST_TOOL | SEARCH_TOOL | OPEN_TOOL)
        {
            return Err(AppError::validation(
                "toolCall",
                "旅行游记工具名称或调用 ID 无效",
            ));
        }
        self.runtime.read(|store, _| {
            self.require_context(store)?;
            let result = match call.name.as_str() {
                LIST_TOOL => {
                    let (limit, offset) = page(&call.arguments)?;
                    json!({"journals": metadata_query(store, self.session_id, None, limit, offset)?})
                }
                SEARCH_TOOL => {
                    let (query, limit, offset) = search_page(&call.arguments)?;
                    json!({"journals": metadata_query(store, self.session_id, Some(&query), limit, offset)?})
                }
                OPEN_TOOL => {
                    let args: OpenArguments = serde_json::from_value(call.arguments.clone()).map_err(|_| invalid_arguments())?;
                    let journal_id = crate::error::parse_id(&args.journal_id)?;
                    let journal = store.connection.query_row(
                        "SELECT id,title,content,created_at,updated_at FROM travel_journals WHERE id=?1 AND session_id=?2",
                        rusqlite::params![journal_id, self.session_id],
                        |row| Ok(json!({
                            "journal_id": row.get::<_, i64>(0)?.to_string(),
                            "title": row.get::<_, String>(1)?,
                            "content": row.get::<_, String>(2)?,
                            "created_at": row.get::<_, String>(3)?,
                            "updated_at": row.get::<_, String>(4)?
                        })),
                    ).optional()?.ok_or_else(AppError::not_found)?;
                    json!({"journal": journal})
                }
                _ => unreachable!("工具名称已在入口校验"),
            };
            Ok(json!({
                "request_id": self.request.id.to_string(),
                "tool_call_id": call.id,
                "result": result
            }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Runtime, Arc<RequestLease>, TravelJournalTools) {
        let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
        let request = Arc::new(runtime.requests.start("chat:1".into(), 1).unwrap());
        let handler = TravelJournalTools::new(&runtime, &request, 1, 1, 0).unwrap();
        (runtime, request, handler)
    }

    fn call(id: &str, name: &str, arguments: Value) -> ToolCall {
        ToolCall {
            index: 0,
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    fn add_journal(
        runtime: &Runtime,
        session_id: i64,
        title: &str,
        content: &str,
        created_at: &str,
    ) -> i64 {
        runtime.read(|store, _| {
            store.connection.execute(
                "INSERT INTO travels(session_id,status,started_at,planned_return_at,actual_return_at,midway_letter_at,
                    midway_letter_state,return_letter_state,gift_state,journal_state,gift_kind,facts_state,destination,
                    activities_json,facts_version,facts_ready_at,gift_snapshot_json,created_at)
                 VALUES(?1,'returned','2026-01-01T00:00:00Z','2026-01-02T00:00:00Z','2026-01-02T00:00:00Z',
                    '2026-01-01T12:00:00Z','done','done','done','done','text','ready','测试地点','[{\"elapsed_minutes\":1,\"description\":\"散步\"}]',
                    1,'2026-01-02T00:00:00Z','{\"schema_version\":1,\"kind\":\"text\"}','2026-01-01T00:00:00Z')",
                [session_id],
            )?;
            let travel_id = store.connection.last_insert_rowid();
            store.connection.execute(
                "INSERT INTO travel_journals(session_id,travel_id,title,content,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)",
                rusqlite::params![session_id, travel_id, title, content, created_at],
            )?;
            Ok(store.connection.last_insert_rowid())
        }).unwrap()
    }

    #[test]
    fn list_is_session_scoped_and_paginates_metadata() {
        let (runtime, _, mut handler) = setup();
        runtime.read(|store, _| {
            store.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'另一会话','a','a')", [])?;
            Ok(())
        }).unwrap();
        add_journal(&runtime, 1, "第一篇", "正文甲", "2026-01-01T00:00:00Z");
        add_journal(&runtime, 1, "第二篇", "正文乙", "2026-01-02T00:00:00Z");
        add_journal(&runtime, 2, "隔离游记", "不应出现", "2026-01-03T00:00:00Z");

        let first = handler
            .execute(&call("p1", LIST_TOOL, json!({"limit":1})))
            .unwrap();
        assert_eq!(first["result"]["journals"].as_array().unwrap().len(), 1);
        assert_eq!(first["result"]["journals"][0]["title"], "第二篇");
        assert!(first["result"]["journals"][0].get("content").is_none());
        let second = handler
            .execute(&call("p2", LIST_TOOL, json!({"limit":1,"offset":1})))
            .unwrap();
        assert_eq!(second["result"]["journals"][0]["title"], "第一篇");
        let all = handler.execute(&call("all", LIST_TOOL, json!({}))).unwrap();
        assert_eq!(all["result"]["journals"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn search_matches_title_or_body_and_open_returns_full_current_body() {
        let (runtime, _, mut handler) = setup();
        let id = add_journal(
            &runtime,
            1,
            "海边旅行",
            "发现蓝色贝壳",
            "2026-02-03T00:00:00Z",
        );
        add_journal(&runtime, 1, "山间散步", "看见松林", "2026-02-04T00:00:00Z");
        let title_hit = handler
            .execute(&call("s1", SEARCH_TOOL, json!({"query":"海边"})))
            .unwrap();
        assert_eq!(title_hit["result"]["journals"].as_array().unwrap().len(), 1);
        let body_hit = handler
            .execute(&call("s2", SEARCH_TOOL, json!({"query":"蓝色贝壳"})))
            .unwrap();
        assert_eq!(body_hit["result"]["journals"][0]["title"], "海边旅行");
        let opened = handler
            .execute(&call("o1", OPEN_TOOL, json!({"journal_id":id.to_string()})))
            .unwrap();
        assert_eq!(opened["result"]["journal"]["content"], "发现蓝色贝壳");
    }

    #[test]
    fn deleted_journal_is_not_restored_from_a_cached_result() {
        let (runtime, _, mut handler) = setup();
        let id = add_journal(&runtime, 1, "会被删除", "正文", "2026-03-01T00:00:00Z");
        runtime
            .read(|store, _| {
                store
                    .connection
                    .execute("DELETE FROM travel_journals WHERE id=?1", [id])?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            handler
                .execute(&call(
                    "gone",
                    OPEN_TOOL,
                    json!({"journal_id":id.to_string()})
                ))
                .unwrap_err()
                .code,
            "NOT_FOUND"
        );
        let list = handler
            .execute(&call("after-delete", LIST_TOOL, json!({})))
            .unwrap();
        assert!(list["result"]["journals"].as_array().unwrap().is_empty());
    }

    #[test]
    fn edited_and_deleted_journals_update_tools_and_next_five_title_context() {
        let (runtime, _, mut handler) = setup();
        let mut ids = Vec::new();
        for day in 1..=6 {
            ids.push(add_journal(&runtime, 1, &format!("历史标题{day}"), "不应自动携带的完整正文", &format!("2026-03-{day:02}T00:00:00Z")));
        }
        let titles = || runtime.read(|store, _| {
            let mut context = crate::chat::BackgroundContext::snapshot(store, &runtime, 1, 1, "人设", "输入", None, None)?;
            Ok(context.request(vec![], &[])?.0.messages.into_iter().filter(|message| message.content.starts_with("旅行游记标题 ")).map(|message| message.content).collect::<Vec<_>>())
        }).unwrap();
        let before = titles();
        assert_eq!(before.len(), 5);
        assert!(!before.iter().any(|title| title.contains("历史标题1")));
        assert!(before.iter().all(|title| !title.contains("不应自动携带的完整正文")));
        let id = ids[5];
        runtime.read(|store, _| {
            let journal = crate::travel_journals::get(&store.connection, 1, id)?;
            crate::travel_journals::update(&mut store.connection, 1, journal.travel_id.parse().unwrap(), id,
                crate::models::TravelJournalExpected { title: journal.title, content: journal.content, updated_at: journal.updated_at },
                "修改后的标题", "修改后的正文", chrono::Utc::now())?;
            Ok(())
        }).unwrap();
        assert!(titles().iter().any(|title| title.contains("修改后的标题")));
        let found = handler.execute(&call("edited-search", SEARCH_TOOL, json!({"query":"修改后的正文"}))).unwrap();
        assert_eq!(found["result"]["journals"][0]["title"], "修改后的标题");
        let opened = handler.execute(&call("edited-open", OPEN_TOOL, json!({"journal_id":id.to_string()}))).unwrap();
        assert_eq!(opened["result"]["journal"]["content"], "修改后的正文");
        runtime.read(|store, _| {
            let journal = crate::travel_journals::get(&store.connection, 1, id)?;
            crate::travel_journals::delete(&mut store.connection, 1, journal.travel_id.parse().unwrap(), id)?;
            Ok(())
        }).unwrap();
        let after = titles();
        assert_eq!(after.len(), 5);
        assert!(!after.iter().any(|title| title.contains("修改后的标题")));
        assert!(after.iter().any(|title| title.contains("历史标题1")));
        assert_eq!(handler.execute(&call("deleted-open", OPEN_TOOL, json!({"journal_id":id.to_string()}))).unwrap_err().code, "NOT_FOUND");
    }

    #[test]
    fn cancellation_context_changes_and_injected_arguments_are_rejected() {
        let (_runtime, request, mut handler) = setup();
        assert!(handler
            .execute(&call("inject", LIST_TOOL, json!({"session_id":2})))
            .is_err());
        assert_eq!(
            handler
                .execute(&call("bad-id", OPEN_TOOL, json!({"journal_id":"01"})))
                .unwrap_err()
                .code,
            "VALIDATION_ERROR"
        );
        assert!(handler
            .execute(&call("bad-limit", LIST_TOOL, json!({"limit":51})))
            .is_err());
        request.cancel.cancel();
        assert_eq!(
            handler
                .execute(&call("cancelled", LIST_TOOL, json!({})))
                .unwrap_err()
                .code,
            "CANCELLED"
        );

        let (runtime, _, mut changed) = setup();
        runtime
            .read(|store, _| {
                store.context_revision += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            changed
                .execute(&call("stale", LIST_TOOL, json!({})))
                .unwrap_err()
                .code,
            "CONTEXT_CHANGED"
        );
    }
}
