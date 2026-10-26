//! 信件及游记的Skill按需只读；复用任务租约、总时限和原最终输出校验。
use crate::{
    ai::{client::AiClient, provider::{Choice, Finish, GenerateRequest, Message}},
    error::{AppError, AppResult},
    platform::contracts::SecretProtector,
    providers::ConnectionTest,
    runtime::{Runtime, Store},
    state::RequestLease,
};
use serde_json::{json, Value};
use std::collections::HashMap;

pub(crate) async fn complete(
    runtime:&Runtime, client:&AiClient, target:&ConnectionTest, input:&GenerateRequest,
    lease:&RequestLease, persona:i64,
    check:impl Fn(&mut Store,&dyn SecretProtector)->AppResult<()>+Send+Sync,
) -> AppResult<Choice> {
    lease.cancel.wait(client.total_timeout(),async {
        let rounds=runtime.read(|s,secrets| {
            check(s,secrets)?;
            crate::config::get(&s.connection)?["tool_call_rounds"].parse::<usize>().map_err(|_|AppError::storage())
        })?;
        let mut request=input.clone();
        request.messages.push(Message::new("system",super::tools::READ_RULES));
        let mut observed:HashMap<String,(String,Value,Value)>=HashMap::new();
        for round in 0..=rounds {
            lease.cancel.check()?;
            runtime.read(|s,secrets|check(s,secrets))?;
            request.tools=if round<rounds {super::tools::read_definitions()} else {vec![]};
            if round==rounds {request.messages.push(Message::new("system","本次Skill查询轮数已达上限，不再调用工具；只依据已取得资料，按原任务格式返回最终结果。"));}
            let choice=client.complete(target,&request,&lease.cancel).await?;
            runtime.read(|s,secrets|check(s,secrets))?;
            if choice.finish!=Finish::ToolCalls {return Ok(choice);}
            if round==rounds {return Err(AppError::new("TOOL_LIMIT_REACHED","已达到后台Skill查询轮数上限，任务未完成"));}
            request.messages.push(Message::tool_request(&choice));
            for call in &choice.tools {
                if call.id.trim().is_empty(){return Err(AppError::validation("toolCall","Skill查询调用ID不能为空"));}
                let value=if let Some((name,args,value))=observed.get(&call.id) {
                    if name!=&call.name||args!=&call.arguments {return Err(AppError::new("TOOL_CALL_CONFLICT","Skill查询调用ID参数发生变化"));}
                    value.clone()
                } else {
                    let result=runtime.read(|s,secrets| {
                        check(s,secrets)?;lease.cancel.check()?;
                        super::tools::read_bound(&s.connection,persona,call)
                    });
                    let value=match result {
                        Ok(result)=>json!({"ok":true,"result":result}),
                        Err(error) if matches!(error.code,"CANCELLED"|"CONTEXT_CHANGED")=>return Err(error),
                        Err(error)=>json!({"ok":false,"error":error}),
                    };
                    observed.insert(call.id.clone(),(call.name.clone(),call.arguments.clone(),value.clone()));value
                };
                request.messages.push(Message::tool_result(&call.id,value));
            }
        }
        unreachable!()
    }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tests::{client,Response,Server};
    fn tools(calls:&[(&str,&str,Value)])->Response {
        Response{status:200,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&json!({"choices":[{"index":0,"message":{"content":"","reasoning_content":"只读查询","tool_calls":calls.iter().map(|(id,name,args)|json!({"id":id,"type":"function","function":{"name":name,"arguments":args.to_string()}})).collect::<Vec<_>>()},"finish_reason":"tool_calls"}]})).unwrap())]}
    }
    fn fixture(url:&str)->(Runtime,ConnectionTest,RequestLease,GenerateRequest) {
        let rt=crate::runtime::test_runtime(url);
        let (target,base)=rt.read(|s,secrets| {
            for (title,body,bound) in [(super::super::BASE_TITLE,"基础必读内容",true),("旅行写作","按需才读取的细节",true),("未绑定","绝不泄露的正文",false)] {
                super::super::save(&mut s.connection,super::super::Input{id:None,revision:None,title:title.into(),summary:"目录简介".into(),body:body.into()})?;
                if bound {super::super::bind(&s.connection,s.connection.last_insert_rowid(),1,true)?;}
            }
            Ok((crate::providers::connection_test_config(&s.connection,1,secrets)?,super::super::context(&s.connection,1)?))
        }).unwrap();
        let lease=rt.requests.start("background-test".into(),1).unwrap();
        (rt,target,lease,GenerateRequest{messages:vec![Message::new("system",base),Message::new("user","按任务要求输出JSON")],max_tokens:1024,tools:vec![],json:true})
    }
    #[test]
    fn background_progressively_reads_bound_skills_and_preserves_tool_protocol() {
        tauri::async_runtime::block_on(async {
            let server=Server::new(vec![tools(&[("list","list_skills",json!({"query":"旅行"}))]),tools(&[("read","get_skill",json!({"skill_id":"2"}))]),Response::json(json!({"content":"完成","expression":"(^_^)"}))]);
            let (rt,target,lease,request)=fixture(&server.url);
            let result=complete(&rt,&client(),&target,&request,&lease,1,|_,_|lease.require_scope(&rt.requests,"background-test",1)).await.unwrap();
            assert_eq!(result.finish,Finish::Stop);
            let requests=server.requests.lock().unwrap();assert_eq!(requests.len(),3);
            assert!(requests[0]["messages"].to_string().contains("基础必读内容"));
            assert!(!requests[0]["messages"].to_string().contains("按需才读取的细节"));
            assert!(!requests[1]["messages"].to_string().contains("按需才读取的细节"));
            assert!(requests[2]["messages"].to_string().contains("按需才读取的细节"));
            assert!(!requests.iter().any(|r|r["messages"].to_string().contains("绝不泄露的正文")));
            let messages=requests[2]["messages"].as_array().unwrap();
            assert!(messages.iter().any(|m|m["role"]=="assistant"&&m["tool_calls"][0]["id"]=="read"&&m["reasoning_content"]=="只读查询"));
            assert!(messages.iter().any(|m|m["tool_call_id"]=="read"));
            assert!(requests[0]["tools"].as_array().unwrap().iter().all(|t|matches!(t["function"]["name"].as_str(),Some("list_skills"|"get_skill"))));
            drop(requests);server.finish();
        });
    }
    #[test]
    fn background_refuses_writes_and_unbound_reads_without_changing_database() {
        tauri::async_runtime::block_on(async {
            let server=Server::new(vec![tools(&[("write","create_skill",json!({"title":"不应创建","body":"禁止","summary":""})),("foreign","get_skill",json!({"skill_id":"3"}))]),Response::json(json!({"content":"已按现有资料完成"}))]);
            let (rt,target,lease,request)=fixture(&server.url);
            complete(&rt,&client(),&target,&request,&lease,1,|_,_|lease.require_scope(&rt.requests,"background-test",1)).await.unwrap();
            assert_eq!(rt.read(|s,_|Ok(super::super::list(&s.connection)?.len())).unwrap(),3);
            let requests=server.requests.lock().unwrap();let messages=requests[1]["messages"].as_array().unwrap();
            let denied=messages.iter().filter(|m|m["role"]=="tool").map(|m|serde_json::from_str::<Value>(m["content"].as_str().unwrap()).unwrap()).collect::<Vec<_>>();
            assert_eq!(denied.len(),2);assert!(denied.iter().all(|v|v["error"]["code"]=="PERMISSION_DENIED"));
            assert!(!requests[1]["messages"].to_string().contains("绝不泄露的正文"));drop(requests);server.finish();
        });
    }
    #[test]
    fn background_stops_when_read_rounds_are_exhausted() {
        tauri::async_runtime::block_on(async {
            let server=Server::new(vec![tools(&[("first","list_skills",json!({}))]),tools(&[("extra","get_skill",json!({"skill_id":"2"}))])]);
            let (rt,target,lease,request)=fixture(&server.url);
            rt.read(|s,_|crate::config::save(&mut s.connection,[("tool_call_rounds".into(),"1".into())].into_iter().collect())).unwrap();
            assert!(complete(&rt,&client(),&target,&request,&lease,1,|_,_|Ok(())).await.is_err());
            assert!(server.requests.lock().unwrap()[1].get("tools").is_none());server.finish();
        });
    }
}
