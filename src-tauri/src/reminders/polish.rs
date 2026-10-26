//! 提醒文案只读接口；统一jobs生成并持久化，不由打开弹窗触发模型请求。
use crate::{error::{AppError,AppResult},runtime::Runtime};
use rusqlite::OptionalExtension;
use serde::Serialize;
pub use super::presentation::Service;
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Presentation {pub occurrence_id:String,pub persona_id:String,pub context_revision:u64,pub text:Option<String>}
pub async fn get(runtime:Runtime,id:i64,persona:String,revision:u64)->AppResult<Option<Presentation>> {
    tauri::async_runtime::spawn_blocking(move||runtime.read(|s,_|{
        let context=crate::persona::context(&s.connection,s.context_revision)?;
        if context.persona_id!=persona||context.context_revision!=revision{return Err(AppError::new("CONTEXT_CHANGED","提醒显示人设已改变"));}
        let text:Option<Option<String>>=s.connection.query_row("SELECT presentation_text FROM reminder_occurrences WHERE id=?1 AND delivery_state<>'missed'",[id],|r|r.get(0)).optional()?;
        Ok(text.map(|text|Presentation{occurrence_id:id.to_string(),persona_id:persona,context_revision:revision,text}))
    })).await.map_err(|_|AppError::storage())?
}
#[cfg(test)]mod tests;
