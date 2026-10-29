//! 聊天表情包：按需披露元数据，只能使用当前人设已绑定的全局库。
use super::*;
use crate::ai::provider::ToolCall;
use rusqlite::{params,OptionalExtension};
use serde_json::{json,Value};
pub const RULES:&str="你可适度、自行决定是否发送聊天表情包；它与切换桌宠表情是两个功能。需要发送时，先list_chat_sticker_tags了解标签，再search_chat_stickers检索，按介绍选择后send_chat_sticker。无需每次回复查询或穷举，也无需向用户解释这些步骤。";
pub fn definitions()->Vec<Value> {
    let make=|name:&str,description:&str,properties:Value,required:Vec<&str>|json!({"type":"function","function":{"name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":required,"additionalProperties":false}}});
    vec![
        make("list_chat_sticker_tags","分页查询当前人设绑定表情包库的可用标签。",json!({"query":{"type":"string"},"page":{"type":"integer","minimum":1}}),vec![]),
        make("search_chat_stickers","分页检索当前人设绑定库中的聊天表情包标题、介绍和标签；多个tagIds必须全部符合。结果不包含图片内容。",json!({"query":{"type":"string"},"tagIds":{"type":"array","items":{"type":"string"}},"page":{"type":"integer","minimum":1}}),vec![]),
        make("send_chat_sticker","把检索到的表情包发到本轮消息，保留GIF动画。",json!({"stickerId":{"type":"string"}}),vec!["stickerId"]),
    ]
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Search {#[serde(default)]query:String,#[serde(default)]page:Option<u32>,#[serde(default)]tag_ids:Vec<String>}
pub fn read(runtime:&Runtime,persona:i64,session:i64,revision:u64,call:&ToolCall)->AppResult<Value> {
    let a:Search=serde_json::from_value(call.arguments.clone()).map_err(|_|AppError::validation("arguments","表情包检索参数无效"))?;
    runtime.read(|s,_|{coordinator::require_context(s,persona,session,revision)?;
        crate::stickers::query(&s.connection,crate::stickers::Query{entity:if call.name=="list_chat_sticker_tags"{"tags"}else{"stickers"}.into(),query:a.query,page:a.page.unwrap_or(1),library_id:None,persona_id:Some(persona.to_string()),tag_ids:a.tag_ids})
    })
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Send {sticker_id:String}
type Snapshot=(String,String,String,String,i64);
fn snapshot(c:&rusqlite::Connection,persona:i64,id:i64)->AppResult<Snapshot> {
    c.query_row("SELECT s.title,s.description,s.filename,s.image_file,s.revision FROM chat_stickers s JOIN persona_sticker_libraries b ON b.library_id=s.library_id WHERE b.persona_id=?1 AND s.id=?2",params![persona,id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?.ok_or_else(||AppError::new("NOT_FOUND","表情包不存在或当前人设未绑定这个库"))
}
pub fn send(runtime:&Runtime,lease:&RequestLease,persona:i64,session:i64,revision:u64,call:&ToolCall)->AppResult<Value> {
    let a:Send=serde_json::from_value(call.arguments.clone()).map_err(|_|AppError::validation("stickerId","请选择检索到的表情包"))?;
    let id=crate::error::parse_id(&a.sticker_id)?;
    let original=runtime.read(|s,_|{lease.require_conversation_scope(&runtime.requests,session,persona)?;coordinator::require_context(s,persona,session,revision)?;snapshot(&s.connection,persona,id)})?;
    let paths=runtime.paths.as_ref().map_err(Clone::clone)?;
    let bytes=std::fs::read(crate::chat_images::managed_file(paths,&original.3)?).map_err(|_|AppError::storage())?;
    use base64::Engine;
    let mut staged=crate::chat_images::stage(paths,vec![crate::chat_images::Input{filename:original.2.clone(),data_url:format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))}])?;
    let result=runtime.read(|s,_|{
        lease.require_conversation_scope(&runtime.requests,session,persona)?;coordinator::require_context(s,persona,session,revision)?;lease.cancel.check()?;
        if snapshot(&s.connection,persona,id)?!=original{return Err(AppError::new("CONFLICT","表情包已改变，请重新检索后再发送"));}
        let active=s.chat_requests.get(&session).filter(|a|a.request_id==lease.id.to_string()).ok_or_else(||AppError::new("CONTEXT_CHANGED","本次聊天已结束，未发送表情包"))?;
        let round=crate::error::parse_id(&active.round_id)?;let content=if active.partial_content.trim().is_empty(){String::new()}else{active.partial_content.clone()};
        let now=chrono::Utc::now().to_rfc3339();let tx=s.connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        crate::chat_images::publish_staged(&staged.images)?;
        tx.execute("INSERT INTO messages(session_id,round_id,role,content,created_at) VALUES(?1,?2,'assistant',?3,?4) ON CONFLICT(round_id,role) DO NOTHING",params![session,round,content,now])?;
        let message:i64=tx.query_row("SELECT id FROM messages WHERE round_id=?1 AND role='assistant'",[round],|r|r.get(0))?;
        crate::chat_images::insert(&tx,message,&staged.images,&now)?;
        tx.execute("UPDATE message_images SET source_kind='sticker',caption=?1 WHERE message_id=?2 AND image_file=?3",params![format!("聊天表情包：{}；{}",original.0,original.1),message,staged.images[0].image_file])?;
        lease.cancel.check()?;tx.commit()?;Ok(json!({"status":"sent","title":original.0,"messageId":message.to_string(),"display":"表情包已加入本轮聊天消息，无需另发路径或Markdown图片链接"}))
    });
    if result.is_ok(){staged.committed=true;}result
}
