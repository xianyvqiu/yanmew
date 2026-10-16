//! 对完整会话集合查询，按真实时间和主键稳定排序，再返回一页。
use crate::{error::{AppError,AppResult},runtime::Store};
use rusqlite::params;
use serde::{Deserialize,Serialize};
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Context {pub persona_id:String,pub session_id:Option<String>,pub context_revision:u64}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Query {pub kind:String,pub context:Context,pub search:String,pub order:String,pub page:u32}
#[derive(Serialize)]
pub struct Page {pub items:Vec<serde_json::Value>,pub total:u64,pub page:u32,pub pages:u32}
pub fn browse(store:&Store,input:Query)->AppResult<Page> {
    let global=input.kind=="dates";
    let current=crate::persona::context(&store.connection,store.context_revision)?;
    if !global && (current.persona_id!=input.context.persona_id||current.session_id!=input.context.session_id||current.context_revision!=input.context.context_revision){return Err(AppError::new("CONTEXT_CHANGED","当前人设或会话已变化，请重新读取列表"));}
    let session=if global {0} else {let Some(session)=input.context.session_id.as_deref()else{return Ok(Page{items:vec![],total:0,page:1,pages:1})};crate::error::parse_id(session)?};
    let (table,content,time)=match input.kind.as_str() {
        "travels"=>("travels","COALESCE(destination,'')||' '||COALESCE(activities_json,'')","started_at"),
        "letters"=>("letters","content","created_at"),
        "journals"=>("travel_journals","title||' '||content","created_at"),
        "accessories"=>("accessories","name||' '||description||' '||display_text","created_at"),
        "souvenirs"=>("souvenirs","content||' '||note","created_at"),
        "dates"=>("custom_festivals","name||' '||blessing||' '||COALESCE(note,'')","created_at"),
        _=>return Err(AppError::validation("kind","不支持的卡片类型")),
    };
    let order=match input.order.as_str(){"newest"=>"DESC","oldest"=>"ASC",_=>return Err(AppError::validation("order","请选择时间正序或倒序"))};
    if input.page==0{return Err(AppError::validation("page","页码从1开始"));}
    let c=&store.connection;let filter=format!("{} AND instr(lower({content}),lower(?2))>0",if global {"?1=0"}else{"session_id=?1"});
    let total:u64=c.query_row(&format!("SELECT count(*) FROM {table} WHERE {filter}"),params![session,input.search.trim()],|r|r.get(0))?;
    let pages=u32::try_from(total.div_ceil(12).max(1)).map_err(|_|AppError::storage())?;let page=input.page.min(pages);
    let mut q=c.prepare(&format!("SELECT id FROM {table} WHERE {filter} ORDER BY {time} {order},id {order} LIMIT 12 OFFSET ?3"))?;
    let ids=q.query_map(params![session,input.search.trim(),i64::from(page-1)*12],|r|r.get::<_,i64>(0))?.collect::<Result<Vec<_>,_>>()?;
    let mut items=Vec::new();
    for id in ids {
        let value=match input.kind.as_str() {
            "travels"=>serde_json::to_value(crate::travel::queries::get(c,session,id)?),
            "letters"=>serde_json::to_value(crate::letters::get(c,session,id)?),
            "journals"=>serde_json::to_value(crate::travel_journals::get(c,session,id)?),
            "accessories"=>serde_json::to_value(crate::accessories::get(c,session,id)?),
            "souvenirs"=>serde_json::to_value(crate::souvenirs::get(c,session,id)?),
            "dates"=>serde_json::to_value(crate::festivals::custom::get(c,id,chrono::Local::now().date_naive())?),
            _=>unreachable!(),
        }.map_err(|_|AppError::storage())?;items.push(value);
    }
    Ok(Page{items,total,page,pages})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_dates_browse_without_session_and_remain_after_session_deletion() {
        let runtime=crate::runtime::test_runtime("http://127.0.0.1:1/v1");
        runtime.read(|store,_| {
            crate::festivals::custom::create(&mut store.connection,crate::festivals::custom::Input {
                kind:"birthday".into(),name:"全局生日".into(),calendar_type:"solar".into(),month:2,day:29,
                is_leap_month:false,blessing:"生日祝福".into(),note:None,
            },"user",chrono::Local::now().date_naive(),chrono::Utc::now())?;
            let query=||Query {kind:"dates".into(),context:Context {persona_id:"1".into(),session_id:None,context_revision:0},search:"生日".into(),order:"newest".into(),page:1};
            let before=browse(store,query())?;
            assert_eq!(before.total,1);assert!(before.items[0].get("sessionId").is_none());
            store.connection.execute("DELETE FROM sessions",[])?;
            assert_eq!(browse(store,query())?.items,before.items);
            Ok(())
        }).unwrap();
    }
    #[test]
    fn settings_redesign_search_finds_records_outside_first_page_and_respects_scope() {
        let runtime=crate::runtime::test_runtime("http://127.0.0.1:1/v1");
        runtime.read(|store,_| {
            for index in 0..25 {store.connection.execute("INSERT INTO accessories(session_id,name,display_text,description,cost,source,created_at) VALUES(1,?1,'⭐','描述',0,'crafted',?2)",params![if index==0{"目标饰品".to_string()}else{format!("饰品{index}")},format!("2026-10-{:02}T00:00:00Z",index+1)])?;}
            let context=||Context{persona_id:"1".into(),session_id:Some("1".into()),context_revision:0};
            let page=browse(store,Query{kind:"accessories".into(),context:context(),search:"".into(),order:"newest".into(),page:1})?;
            assert_eq!(page.items.len(),12);assert_eq!(page.total,25);assert_eq!(page.pages,3);
            let found=browse(store,Query{kind:"accessories".into(),context:context(),search:"目标".into(),order:"newest".into(),page:99})?;
            assert_eq!(found.total,1);assert_eq!(found.page,1);assert_eq!(found.items[0]["name"],"目标饰品");
            let mut wrong=context();wrong.session_id=Some("999".into());assert!(browse(store,Query{kind:"accessories".into(),context:wrong,search:"".into(),order:"newest".into(),page:1}).is_err());Ok(())
        }).unwrap();
    }
}
