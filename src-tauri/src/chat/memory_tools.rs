//! 深层回忆只读工具；所有参数都不含会话 ID，后端固定请求快照。
use super::*;
use crate::ai::provider::ToolCall;
use rusqlite::params;
use serde_json::{json, Value};

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{"name":"search_long_memories","description":"混合检索当前会话全部长期记忆，返回少量 ID、标题与来源通道；需要正文再打开。","parameters":{"type":"object","properties":{"query":{"type":"string","minLength":1,"maxLength":4096}},"required":["query"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"open_long_memory","description":"只读打开当前会话指定长期记忆的完整标题、正文和来源数。","parameters":{"type":"object","properties":{"memory_id":{"type":"string","pattern":"^[1-9][0-9]*$"}},"required":["memory_id"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"open_long_memory_sources","description":"按来源顺序分页读取长期记忆的来源短期记忆及原始回合 ID。","parameters":{"type":"object","properties":{"memory_id":{"type":"string","pattern":"^[1-9][0-9]*$"},"limit":{"type":"integer","minimum":1,"maximum":50,"default":20},"offset":{"type":"integer","minimum":0,"default":0}},"required":["memory_id"],"additionalProperties":false}}}),
        json!({"type":"function","function":{"name":"open_chat_round","description":"只读打开当前会话一个原始聊天回合，包含用户消息和已保存 AI 回复。","parameters":{"type":"object","properties":{"round_id":{"type":"string","pattern":"^[1-9][0-9]*$"}},"required":["round_id"],"additionalProperties":false}}}),
    ]
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    query: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Open {
    memory_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sources {
    memory_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Round {
    round_id: String,
}
fn arguments<T: serde::de::DeserializeOwned>(call: &ToolCall) -> AppResult<T> {
    serde_json::from_value(call.arguments.clone())
        .map_err(|_| AppError::validation("arguments", "记忆工具参数无效"))
}
pub(super) async fn execute(
    runtime: &Runtime,
    lease: &Arc<RequestLease>,
    persona: i64,
    session: i64,
    revision: u64,
    call: &ToolCall,
) -> AppResult<Value> {
    let check = |store: &crate::runtime::Store| {
        lease.require_scope(&runtime.requests, &format!("chat:{session}"), persona)?;
        runtime.jobs_cancel.check()?;
        coordinator::require_context(store, persona, session, revision)
    };
    runtime.read(|s, _| check(s))?;
    if call.name == "search_long_memories" {
        let args: Search = arguments(call)?;
        let result =
            crate::memory_search::search_metadata(runtime, session, &args.query, &lease.cancel).await?;
        runtime.read(|s, _| check(s))?;
        return Ok(
            json!({"items":result.hits.into_iter().map(|hit|json!({"memory_id":hit.id,"title":hit.title,"matched_by":hit.matched_by})).collect::<Vec<_>>(),"semantic_available":result.fallback_error.is_none()}),
        );
    }
    runtime.read(|store,_| {
        check(store)?;
        let c=&store.connection;
        match call.name.as_str() {
            "open_long_memory" => {
                let args:Open=arguments(call)?;let id=crate::error::parse_id(&args.memory_id)?;
                c.query_row("SELECT title,content,source_count,activated_by_round_id FROM long_memories WHERE id=?1 AND session_id=?2",params![id,session],|r|Ok(json!({"memory_id":args.memory_id,"title":r.get::<_,String>(0)?,"content":r.get::<_,String>(1)?,"source_count":r.get::<_,i64>(2)?,"activated_by_round_id":r.get::<_,i64>(3)?.to_string()}))).optional()?.ok_or_else(AppError::not_found)
            }
            "open_long_memory_sources" => {
                let args:Sources=arguments(call)?;let id=crate::error::parse_id(&args.memory_id)?;
                let limit=args.limit.unwrap_or(20);let offset=args.offset.unwrap_or(0);
                if !(1..=50).contains(&limit) {return Err(AppError::validation("limit","来源分页数量须在 1–50 之间"));}
                let exists:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM long_memories WHERE id=?1 AND session_id=?2)",params![id,session],|r|r.get(0))?;
                if !exists {return Err(AppError::not_found());}
                let mut stmt=c.prepare("SELECT s.id,s.round_id,s.content,l.source_order FROM long_memory_sources l JOIN short_memories s ON s.id=l.short_memory_id WHERE l.long_memory_id=?1 AND s.session_id=?2 ORDER BY l.source_order LIMIT ?3 OFFSET ?4")?;
                let items=stmt.query_map(params![id,session,limit+1,offset],|r|Ok(json!({"short_memory_id":r.get::<_,i64>(0)?.to_string(),"round_id":r.get::<_,i64>(1)?.to_string(),"content":r.get::<_,String>(2)?,"source_order":r.get::<_,i64>(3)?})))?.collect::<Result<Vec<_>,_>>()?;
                let next=if items.len()>limit as usize {Some(offset.saturating_add(limit))} else {None};
                Ok(json!({"memory_id":args.memory_id,"items":items.into_iter().take(limit as usize).collect::<Vec<_>>(),"next_offset":next}))
            }
            "open_chat_round" => {
                let args:Round=arguments(call)?;let id=crate::error::parse_id(&args.round_id)?;
                let snapshot=crate::sessions::round_snapshot(c,persona,session,id)?;
                serde_json::to_value(snapshot).map_err(|_|AppError::storage())
            }
            _=>Err(AppError::new("UNKNOWN_TOOL","未注册此回忆工具")),
        }
    })
}
