//! 端点与确切模型的能力证据；原生搜索只发送必要检索词，复用用户当前连接。
use crate::{error::{AppError,AppResult},providers::ConnectionTest,ai::client::AiClient,state::Cancellation};
use serde::Serialize;use serde_json::{Value,json};use std::{collections::HashMap,sync::{Mutex,OnceLock},time::{Instant,Duration}};
#[derive(Clone,Debug,Serialize)]#[serde(rename_all="camelCase")]
pub struct Capabilities{pub native_search_ready:bool,pub web_search:Option<bool>,pub reasoning:Option<bool>,pub source:String,pub reference:Option<String>}
#[derive(Clone,Copy,PartialEq,Eq)]pub enum Protocol{Responses,ChatSearch,Gemini,DeepSeekSearch}
#[derive(Clone)]struct Evidence{caps:Capabilities,protocol:Option<Protocol>,at:Instant}
static CACHE:OnceLock<Mutex<HashMap<(String,String),Evidence>>>=OnceLock::new();
fn key(base:&str,model:&str)->(String,String){(base.trim_end_matches('/').into(),model.into())}
fn known(base:&str,model:&str)->Evidence{
 let parsed=url::Url::parse(base).ok();let host=parsed.as_ref().and_then(|u|u.host_str().map(str::to_owned));
 let path=parsed.as_ref().map(|u|u.path().trim_end_matches('/')).unwrap_or("");
 let mut e=Evidence{caps:Capabilities{native_search_ready:false,web_search:None,reasoning:None,source:"unknown".into(),reference:None},protocol:None,at:Instant::now()};
 match host.as_deref(){
 Some("api.openai.com") if matches!(path,"/v1"|"/v1/chat/completions")=>{if ["gpt-5","gpt-5-mini","gpt-5.4","gpt-5.5","o4-mini"].contains(&model){e.caps.reasoning=Some(true);}if model=="gpt-5-search-api"{e.caps.web_search=Some(true);e.protocol=Some(Protocol::ChatSearch);e.caps.reference=Some("https://developers.openai.com/api/docs/guides/tools-web-search".into());}else if ["gpt-4.1","gpt-4.1-mini","gpt-5","gpt-5-mini","gpt-5.4","gpt-5.5","o4-mini"].contains(&model){e.protocol=Some(Protocol::Responses);}if e.protocol.is_some(){e.caps.reference=Some("https://developers.openai.com/api/docs/guides/tools-web-search".into());}},
 Some("api.deepseek.com") if matches!(path,""|"/v1"|"/chat/completions"|"/v1/chat/completions")=>{if ["deepseek-flash","deepseek-v4-flash","deepseek-v4-flash-vision-exp","deepseek-v4-pro"].contains(&model){e.protocol=Some(Protocol::DeepSeekSearch);e.caps.reasoning=Some(true);e.caps.reference=Some("https://api-docs.deepseek.com/guides/search/".into());}},
 Some("api.siliconflow.cn"|"api.siliconflow.com") if matches!(path,"/v1"|"/v1/chat/completions")=>{if ["deepseek-ai/DeepSeek-V4.1-Flash","deepseek-ai/DeepSeek-V4-Pro","deepseek-ai/DeepSeek-V4-Flash"].contains(&model){e.caps.reasoning=Some(true);e.caps.reference=Some("https://www.siliconflow.cn/models".into());}},
 Some("generativelanguage.googleapis.com") if path=="/v1beta/openai"=>{if ["gemini-2.5-pro","gemini-2.5-flash","gemini-2.5-flash-lite","gemini-3-flash-preview","gemini-3.1-pro-preview","gemini-3.8-flash","gemini-3.6-flash","gemini-3.5-flash-lite"].contains(&model){e.protocol=Some(Protocol::Gemini);e.caps.reasoning=Some(true);e.caps.reference=Some("https://ai.google.dev/gemini-api/docs/google-search".into());}},_=>{}}
 if e.protocol.is_some(){e.caps.web_search=Some(true);}if e.caps.web_search.is_some()||e.caps.reasoning.is_some(){e.caps.source="official_docs".into();}e
}
fn evidence(base:&str,model:&str)->Evidence{CACHE.get().and_then(|c|c.lock().ok()?.get(&key(base,model)).filter(|e|e.at.elapsed()<Duration::from_secs(86400)).cloned()).unwrap_or_else(||known(base,model))}
pub fn resolve(base:&str,model:&str)->Capabilities{let e=evidence(base,model);let mut caps=e.caps;caps.native_search_ready=caps.web_search==Some(true)&&e.protocol.is_some();caps}
pub fn protocol(base:&str,model:&str)->Option<Protocol>{let e=evidence(base,model);if e.caps.web_search==Some(true){e.protocol}else{None}}
pub fn remember(base:&str,model:&str,value:&Value){
 let item=if value["id"]==model{Some(value)}else{value["data"].as_array().and_then(|a|a.iter().find(|v|v["id"]==model))};let Some(item)=item else{return};
 let mut e=evidence(base,model);let web=item.pointer("/capabilities/server_tools/web_search/supported").or_else(||item.pointer("/capabilities/web_search/supported")).or_else(||item.pointer("/capabilities/web_search")).and_then(Value::as_bool);let reasoning=item.pointer("/capabilities/thinking/supported").or_else(||item.pointer("/capabilities/reasoning/supported")).or_else(||item.pointer("/capabilities/reasoning")).and_then(Value::as_bool).or_else(|| item.pointer("/effort/supported_levels").and_then(Value::as_array).filter(|a|a.iter().any(|v|v.as_str().is_some_and(|s|!matches!(s,"none"|"")))).map(|_|true)).or_else(||item["supported_parameters"].as_array().filter(|a|a.iter().any(|v|matches!(v.as_str(),Some("reasoning"|"reasoning_effort"|"enable_thinking")))).map(|_|true));
 if web.is_none()&&reasoning.is_none(){return}if let Some(v)=web{e.caps.web_search=Some(v);}if let Some(v)=reasoning{e.caps.reasoning=Some(v);}if web==Some(true){if let Some(p)=item.pointer("/capabilities/web_search/protocol").and_then(Value::as_str){e.protocol=match p{"responses"=>Some(Protocol::Responses),"chat_completions"=>Some(Protocol::ChatSearch),"gemini"=>Some(Protocol::Gemini),"deepseek_search"=>Some(Protocol::DeepSeekSearch),_=>e.protocol};}}
 e.caps.source="provider_metadata".into();e.caps.reference=Some(format!("{}/models",base.trim_end_matches('/')));e.at=Instant::now();put(base,model,e);
}
fn put(base:&str,model:&str,e:Evidence){if let Ok(mut c)=CACHE.get_or_init(Default::default).lock(){if c.len()>=128{c.retain(|_,v|v.at.elapsed()<Duration::from_secs(3600));}if c.len()>=128{c.clear();}c.insert(key(base,model),e);}}
pub fn observed_reasoning(base:&str,model:&str){let mut e=evidence(base,model);e.caps.reasoning=Some(true);e.caps.source="response_observed".into();e.at=Instant::now();put(base,model,e);}
/// 只读模型目录的短期节流，同时缓存失败，避免界面刷新造成连续请求。
pub fn claim_discovery(base:&str,model:&str)->bool {
 static ATTEMPTS:OnceLock<Mutex<HashMap<(String,String),Instant>>>=OnceLock::new();
 let Ok(mut attempts)=ATTEMPTS.get_or_init(Default::default).lock()else{return false};
 attempts.retain(|_,at|at.elapsed()<Duration::from_secs(300));
 if attempts.contains_key(&key(base,model)){return false}if attempts.len()>=128{return false}
 attempts.insert(key(base,model),Instant::now());true
}
fn failed(text:&str)->AppError{AppError::new("NATIVE_SEARCH_FAILED",text)}
pub async fn search(client:&AiClient,cfg:&ConnectionTest,query:&str,cancel:&Cancellation)->AppResult<(String,Vec<crate::web_search::Source>)>{
 let kind=protocol(&cfg.base_url,&cfg.model).ok_or_else(||failed("当前端点未确认支持可适配的原生搜索"))?;
 cancel.wait(Duration::from_secs(60),async{
 let base=cfg.base_url.trim_end_matches('/');let root=base.strip_suffix("/chat/completions").unwrap_or(base);
 let (endpoint,body)=match kind{
 Protocol::DeepSeekSearch=>{if query.chars().count()>128{return Err(failed("当前原生搜索每条检索词最多128个字符，请缩短检索词"));}let mut url=url::Url::parse(root).map_err(|_|failed("原生搜索连接地址无效"))?;url.set_path("/v1/tools/search");url.set_query(None);url.set_fragment(None);(url.to_string(),json!({"query":query,"count":5,"need_content":true}))},
 Protocol::Responses=>(format!("{root}/responses"),json!({"model":cfg.model,"input":query,"tools":[{"type":"web_search"}],"tool_choice":"required","max_output_tokens":cfg.max_output_tokens,"stream":false,"store":false})),
 Protocol::ChatSearch=>(format!("{root}/chat/completions"),json!({"model":cfg.model,"messages":[{"role":"user","content":query}],"web_search_options":{},"max_completion_tokens":cfg.max_output_tokens,"stream":false})),
 Protocol::Gemini=>{let mut url=url::Url::parse(root).map_err(|_|failed("原生搜索连接地址无效"))?;url.set_path("");url.set_query(None);url.set_fragment(None);url.path_segments_mut().map_err(|_|failed("原生搜索连接地址无效"))?.extend(["v1beta","models",&format!("{}:generateContent",cfg.model)]);(url.to_string(),json!({"contents":[{"parts":[{"text":query}]}],"tools":[{"google_search":{}}],"generationConfig":{"maxOutputTokens":cfg.max_output_tokens}}))}}
 ;let mut req=client.search_http().post(endpoint).json(&body);if let Some(key)=&cfg.api_key{req=if kind==Protocol::Gemini{req.header("x-goog-api-key",key)}else{req.bearer_auth(key)};}
 let mut response=req.send().await.map_err(|_|failed("原生搜索连接失败，未自动重试"))?;if !response.status().is_success(){return Err(failed(&format!("原生搜索返回HTTP {}，请检查模型、接口权限和额度；未自动重试",response.status().as_u16())));}
 let mut bytes=Vec::new();while let Some(chunk)=response.chunk().await.map_err(|_|failed("原生搜索响应读取失败"))?{if bytes.len()+chunk.len()>4*1024*1024{return Err(failed("原生搜索响应过大"));}bytes.extend_from_slice(&chunk);}let value:Value=serde_json::from_slice(&bytes).map_err(|_|failed("原生搜索响应格式无效"))?;cancel.check()?;
 parse(kind,&value)
 }).await
}
fn parse(kind:Protocol,value:&Value)->AppResult<(String,Vec<crate::web_search::Source>)>{
 if kind==Protocol::DeepSeekSearch {
  let results=value["results"].as_array().ok_or_else(||failed("原生搜索响应缺少结果列表"))?;
  let mut sources=Vec::<crate::web_search::Source>::new();let mut texts=Vec::new();
  for result in results.iter().take(10) {
   let Some(raw)=result["url"].as_str() else{continue};let Ok(url)=crate::web_search::source_url(raw)else{continue};
   if sources.iter().any(|s|s.url==url){continue}
   let title=result["title"].as_str().filter(|s|!s.trim().is_empty()).unwrap_or(&url);
   let excerpt=["summary","snippet","content"].iter().filter_map(|k|result[k].as_str()).find(|s|!s.trim().is_empty()).unwrap_or("");
   let excerpt:String=excerpt.chars().take(1500).collect();texts.push(format!("{title}\n{url}\n{excerpt}"));
   sources.push(crate::web_search::Source{title:title.chars().take(300).collect(),url,excerpt,published_at:result["date"].as_str().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d|d.to_rfc3339()),retrieved_at:chrono::Utc::now().to_rfc3339()});
  }
  return Ok((texts.join("\n\n").chars().take(10000).collect(),sources))
 }
 let mut texts=Vec::new();let mut links=Vec::<(String,String)>::new();let mut add=|v:&Value|{let node=v.get("url_citation").unwrap_or(v);if let Some(url)=node.get("url").or_else(||node.get("uri")).and_then(Value::as_str){links.push((node["title"].as_str().unwrap_or(url).into(),url.into()));}};
 match kind{
 Protocol::DeepSeekSearch=>unreachable!(),
 Protocol::Responses=>{if value["status"]!="completed"{return Err(failed("原生搜索未完整完成"));}for output in value["output"].as_array().into_iter().flatten(){if output["type"]=="message"{for part in output["content"].as_array().into_iter().flatten(){if part["type"]=="output_text"{if let Some(t)=part["text"].as_str(){texts.push(t.to_string());}for a in part["annotations"].as_array().into_iter().flatten(){add(a);}}}}for source in output.pointer("/action/sources").and_then(Value::as_array).into_iter().flatten(){add(source);}}},
 Protocol::ChatSearch=>{let m=&value["choices"][0];if m["finish_reason"]!="stop"{return Err(failed("原生搜索未完整完成"));}if let Some(t)=m["message"]["content"].as_str(){texts.push(t.into());}for a in m["message"]["annotations"].as_array().into_iter().flatten(){add(a);}},
 Protocol::Gemini=>{let c=&value["candidates"][0];if c["finishReason"]!="STOP"{return Err(failed("原生搜索未完整完成"));}for p in c["content"]["parts"].as_array().into_iter().flatten(){if p["thought"]!=true{if let Some(t)=p["text"].as_str(){texts.push(t.into());}}}for g in c["groundingMetadata"]["groundingChunks"].as_array().into_iter().flatten(){add(&g["web"]);}}}
 let text=texts.join("\n");if text.trim().is_empty(){return Err(failed("原生搜索没有返回可用内容"));}let mut sources=Vec::new();for(title,url)in links{let Ok(url)=crate::web_search::source_url(&url)else{continue};if sources.iter().any(|s:&crate::web_search::Source|s.url==url){continue}sources.push(crate::web_search::Source{title,url,excerpt:text.chars().take(1500).collect(),published_at:None,retrieved_at:chrono::Utc::now().to_rfc3339()});if sources.len()==10{break;}}
 if sources.is_empty(){return Err(failed("原生搜索未返回可核对来源，不能作为已联网结果"));}Ok((text.chars().take(10000).collect(),sources))
}

#[cfg(test)]mod tests{use super::*;
 #[test]fn capabilities_need_exact_endpoint_and_model_or_explicit_metadata(){
 assert_eq!(resolve("https://api.deepseek.com/v1","deepseek-flash").web_search,Some(true));assert_eq!(resolve("https://api.deepseek.com/v1","deepseek-v4-pro").web_search,Some(true));assert_eq!(resolve("https://proxy.invalid/v1","deepseek-v4-pro").web_search,None);assert_eq!(resolve("https://api.siliconflow.cn/v1","deepseek-ai/DeepSeek-V4.1-Flash").reasoning,Some(true));assert_eq!(resolve("https://api.siliconflow.cn/v1","deepseek-ai/DeepSeek-V4.1-Flash").web_search,None);
 remember("https://metadata.invalid/v1","exact",&json!({"id":"exact","capabilities":{"web_search":{"supported":true,"protocol":"responses"},"reasoning":false}}));let caps=resolve("https://metadata.invalid/v1","exact");assert_eq!(caps.web_search,Some(true));assert_eq!(caps.reasoning,Some(false));assert_eq!(resolve("https://metadata.invalid/v1","exact-other").web_search,None);assert!(protocol("https://metadata.invalid/v1","exact").is_some());
 observed_reasoning("https://observed.invalid/v1","model");assert_eq!(resolve("https://observed.invalid/v1","model").reasoning,Some(true));
 }
 #[test]fn metadata_effort_is_reasoning_evidence_without_inventing_search_support(){
 remember("https://effort.invalid/v1","deepseek-test",&json!({"data":[{"id":"deepseek-test","effort":{"supported_levels":["low","high","max"]}}]}));
 let caps=resolve("https://effort.invalid/v1","deepseek-test");assert_eq!(caps.reasoning,Some(true));assert_eq!(caps.web_search,None);assert!(!caps.native_search_ready);
 remember("https://params.invalid/v1","reason",&json!({"id":"reason","supported_parameters":["reasoning_effort"]}));assert_eq!(resolve("https://params.invalid/v1","reason").reasoning,Some(true));
 assert!(claim_discovery("https://once.invalid/v1","one"));assert!(!claim_discovery("https://once.invalid/v1","one"));
 }
 #[test]fn deepseek_structured_search_preserves_source_specific_text_dates_and_no_result(){
 let (text,sources)=parse(Protocol::DeepSeekSearch,&json!({"results":[{"title":"一","url":"https://example.com/one","summary":"第一篇摘要","date":"2026-10-27T00:00:00Z"},{"title":"二","url":"https://example.com/two","summary":"","snippet":"第二篇摘要"},{"title":"重复","url":"https://example.com/one"},{"url":"file:///secret"}]})).unwrap();
 assert!(text.contains("第一篇摘要"));assert_eq!(sources.len(),2);assert_eq!(sources[0].excerpt,"第一篇摘要");assert_eq!(sources[1].excerpt,"第二篇摘要");assert!(sources[0].published_at.is_some());
 assert!(parse(Protocol::DeepSeekSearch,&json!({"results":[]})).unwrap().1.is_empty());assert!(parse(Protocol::DeepSeekSearch,&json!({"error":"bad"})).is_err());
 }
 #[test]fn native_responses_and_gemini_require_completion_and_sources(){
 let result=parse(Protocol::Responses,&json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"事实","annotations":[{"type":"url_citation","url":"https://example.com/a","title":"来源"}]}]}]})).unwrap();assert_eq!(result.0,"事实");assert_eq!(result.1.len(),1);
 assert!(parse(Protocol::Responses,&json!({"status":"incomplete","output":[]})).is_err());
 assert!(parse(Protocol::Responses,&json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"无依据"}]}]})).is_err());
 let g=parse(Protocol::Gemini,&json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"秘密推理","thought":true},{"text":"答案"}]},"groundingMetadata":{"groundingChunks":[{"web":{"uri":"https://example.com/g","title":"网页"}}]}}]})).unwrap();assert_eq!(g.0,"答案");assert_eq!(g.1.len(),1);
 }
}

#[cfg(test)]mod network_tests{use super::*;use crate::ai::tests::{Server,Response,client};
 #[test]fn deepseek_raw_search_uses_current_key_without_a_second_model_request(){tauri::async_runtime::block_on(async{
 let response=json!({"results":[{"title":"天气资料","url":"https://example.com/weather","summary":"当地预报","date":null}]});
 let server=Server::new_at(vec![Response{status:200,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&response).unwrap())]}],"/v1/tools/search");
 remember(&server.url,"deepseek-fixture",&json!({"id":"deepseek-fixture","capabilities":{"web_search":{"supported":true,"protocol":"deepseek_search"}}}));
 let cfg=ConnectionTest{base_url:server.url.clone(),model:"deepseek-fixture".into(),api_key:Some("fixture-key".into()),max_output_tokens:32768,context_window_override:None};
 let result=search(&client(),&cfg,"当地天气",&Cancellation::default()).await.unwrap();assert_eq!(result.1.len(),1);
 let requests=server.requests.lock().unwrap();assert_eq!(requests.len(),1);assert_eq!(requests[0]["query"],"当地天气");assert_eq!(requests[0]["count"],5);assert_eq!(requests[0]["need_content"],true);assert!(requests[0].get("model").is_none());drop(requests);server.finish();
 });}
 #[test]fn native_search_uses_same_connection_and_reuses_call_without_external_key(){tauri::async_runtime::block_on(async{
 let response=json!({"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":"检索后的事实","annotations":[{"type":"url_citation","url":"https://example.com/source","title":"来源"}]}]}]});
 let server=Server::new_at(vec![Response{status:200,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&response).unwrap())]}],"/v1/responses");
 remember(&server.url,"native-test",&json!({"id":"native-test","capabilities":{"web_search":{"supported":true,"protocol":"responses"}}}));
 let runtime=crate::runtime::test_runtime(&server.url);runtime.read(|s,_|{s.connection.execute("UPDATE provider_models SET model='native-test',max_output_tokens=6144 WHERE purpose='chat'",[])?;s.connection.execute("UPDATE providers SET api_key_encrypted='fake-key' WHERE id=1",[])?;Ok(())}).unwrap();let cfg=ConnectionTest{base_url:server.url.clone(),model:"native-test".into(),api_key:Some("fake-key".into()),max_output_tokens:6144,context_window_override:None};
 let mut search=crate::web_search::SearchSession::new(&runtime,"用户搜索天气").with_native(cfg);assert!(search.available());let call=crate::ai::provider::ToolCall{index:0,id:"native-once".into(),name:"search_web".into(),arguments:json!({"query":"天气"})};let a=search.execute(&client(),&runtime,&call,&Cancellation::default()).await.unwrap();let b=search.execute(&client(),&runtime,&call,&Cancellation::default()).await.unwrap();assert_eq!(a,b);assert_eq!(a["engine"],"model_native");assert_eq!(server.requests.lock().unwrap().len(),1);let requests=server.requests.lock().unwrap();assert_eq!(requests[0]["input"],"天气");assert_eq!(requests[0]["max_output_tokens"],6144);assert!(requests[0].get("api_key").is_none());drop(requests);server.finish();
 });}
}
