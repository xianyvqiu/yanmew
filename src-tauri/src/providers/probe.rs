//! 没有聊天模型的连接只读探测模型目录，不发送生图请求。
use crate::{ai::client::AiClient,error::{AppError,AppResult},providers::ConnectionTest,state::Cancellation};
pub fn endpoint(base:&str)->AppResult<String>{
  if let Some(result)=crate::image_generation::openai_compatible::native::catalog(base){return result.map(|(url,_)|url);}
  let mut url=url::Url::parse(base).map_err(|_|AppError::validation("baseUrl","连接地址无效"))?;
  let path=url.path().trim_end_matches('/');
  let root=["/images/generations","/images/edits","/chat/completions"].into_iter().find_map(|suffix|path.strip_suffix(suffix)).unwrap_or(path);
  let path=if root.is_empty(){"/v1/models".into()}else{format!("{root}/models")};
  url.set_path(&path);url.set_query(None);url.set_fragment(None);Ok(url.into())
}
pub async fn test(client:&AiClient,config:&ConnectionTest)->AppResult<()>{
  let cancel=Cancellation::default();
  cancel.wait(std::time::Duration::from_secs(30),async{
    let mut request=client.search_http().get(endpoint(&config.base_url)?);
    if let Some(key)=&config.api_key{request=if crate::image_generation::openai_compatible::native::detect(&config.base_url)==Some(crate::image_generation::openai_compatible::native::Kind::Gemini){request.header("x-goog-api-key",key)}else{request.bearer_auth(key)};}
    let mut response=request.send().await.map_err(|error|{let detail=crate::ai::client::transport(error);AppError::new("CONNECTION_TEST_FAILED",format!("{}；未发送生图请求",detail.message))})?;
    if !response.status().is_success(){return Err(AppError::new("CONNECTION_TEST_FAILED",format!("模型目录检查失败：{}；请确认服务支持模型列表接口。未发送生图请求",crate::ai::client::http_failure_message(response.status().as_u16()))));}
    let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|error|{let detail=crate::ai::client::transport(error);AppError::new("CONNECTION_TEST_FAILED",format!("模型目录响应读取失败：{}；未发送生图请求",detail.message))})?{
      if bytes.len()+chunk.len()>2*1024*1024{return Err(AppError::new("CONNECTION_TEST_FAILED","模型目录响应过大"));}bytes.extend_from_slice(&chunk);
    }
    let value:serde_json::Value=serde_json::from_slice(&bytes).map_err(|_|AppError::new("CONNECTION_TEST_FAILED","服务未返回有效的JSON模型目录，无法确认连接"))?;
    if !value.get("data").is_some_and(serde_json::Value::is_array)&&!value.get("models").is_some_and(serde_json::Value::is_array){return Err(AppError::new("CONNECTION_TEST_FAILED","服务未返回可识别的模型目录，无法确认连接"));}
    Ok(())
  }).await
}
#[cfg(test)]mod tests{
 use super::*;use crate::ai::tests::client;use serde_json::json;use std::io::{Read,Write};
 #[test]fn image_generation_endpoint_maps_to_same_origin_models(){
  assert_eq!(endpoint("https://api.siliconflow.cn/v1/images/generations").unwrap(),"https://api.siliconflow.cn/v1/models");assert_eq!(endpoint("https://example.com/v1/").unwrap(),"https://example.com/v1/models");
 }
 #[test]fn catalog_http_failures_explain_auth_and_proxy_without_exposing_response(){tauri::async_runtime::block_on(async{
  for (status,expected) in [(401,"认证失败"),(407,"代理要求认证")] {
   let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
   let worker=std::thread::spawn(move||{
    let(mut socket,_)=listener.accept().unwrap();socket.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();let mut buffer=[0;8192];socket.read(&mut buffer).unwrap();
    let body="private-api-key and upstream details";write!(socket,"HTTP/1.1 {status} Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
   });
   let config=ConnectionTest{base_url:format!("http://{address}/v1"),model:"未配置".into(),api_key:None,max_output_tokens:32768,context_window_override:None};
   let error=test(&client(),&config).await.unwrap_err();
   assert_eq!(error.code,"CONNECTION_TEST_FAILED");assert!(error.message.contains(expected));assert!(error.message.contains("未发送生图请求"));assert!(!error.message.contains("private-api-key"));worker.join().unwrap();
  }
 });}
 #[test]fn image_only_connection_reads_catalog_once_and_rejects_non_catalog_response(){tauri::async_runtime::block_on(async{
  for (value,ok) in [(json!({"data":[{"id":"image-model"}]}),true),(json!({"unexpected":"not a model catalog"}),false)]{
   let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
   let worker=std::thread::spawn(move||{
    let(mut socket,_)=listener.accept().unwrap();socket.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();let mut bytes=Vec::new();let mut buffer=[0;4096];
    while !bytes.windows(4).any(|part|part==b"\r\n\r\n"){let n=socket.read(&mut buffer).unwrap();assert!(n>0);bytes.extend_from_slice(&buffer[..n]);assert!(bytes.len()<16384);}
    assert!(String::from_utf8_lossy(&bytes).starts_with("GET /v1/models HTTP/1.1"));
    let body=value.to_string();write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
   });
   let config=ConnectionTest{base_url:format!("http://{address}/v1/images/generations"),model:"未配置".into(),api_key:None,max_output_tokens:32768,context_window_override:None};
   assert_eq!(test(&client(),&config).await.is_ok(),ok);worker.join().unwrap();
  }
 });}
}
