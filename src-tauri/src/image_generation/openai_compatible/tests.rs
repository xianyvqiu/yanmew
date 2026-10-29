use super::*;
use crate::ai::tests::{client,Response,Server};
fn response(status:u16,value:Value)->Response {Response {status,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&value).unwrap())]}}
fn snapshot(url:String)->Snapshot {Snapshot {provider:crate::image_generation::Provider {id:"1".into(),name:"测试".into(),protocol:"openai-compatible".into(),base_url:url,model:"configured-image-model".into()},revision:1,key:"image-only-key".into()}}
#[tokio::test]
async fn sends_one_image_with_selected_model_and_transparent_parameters() {
    let encoded=base64::engine::general_purpose::STANDARD.encode([1,2,3]);
    let server=Server::new_at(vec![response(200,json!({"data":[{"b64_json":encoded}]}))],"/v1/images/generations");
    let bytes=generate(&client(),&snapshot(server.url.clone()),"蝴蝶结","我想戴一朵小蝴蝶结",&Cancellation::default()).await.unwrap();
    assert_eq!(bytes,vec![1,2,3]);
    let requests=server.requests.lock().unwrap();assert_eq!(requests.len(),1);
    assert_eq!(requests[0]["model"],"configured-image-model");assert_eq!(requests[0]["n"],1);assert_eq!(requests[0]["size"],"1024x1024");assert_eq!(requests[0]["background"],"transparent");assert_eq!(requests[0]["output_format"],"png");
    assert!(requests[0].get("api_key").is_none());drop(requests);server.finish();
}
#[tokio::test]
async fn api_failure_does_not_retry_and_only_first_item_is_used() {
    let server=Server::new_at(vec![response(503,json!({"error":"failed"}))],"/v1/images/generations");
    assert!(generate(&client(),&snapshot(server.url.clone()),"星星","亮闪闪",&Cancellation::default()).await.is_err());
    assert_eq!(server.requests.lock().unwrap().len(),1);server.finish();
    let valid=base64::engine::general_purpose::STANDARD.encode([1]);
    assert!(parse(json!({"data":[{"b64_json":"!"},{"b64_json":valid}]}),&Cancellation::default()).await.is_err());
    assert!(parse(json!({"data":[]}),&Cancellation::default()).await.is_err());
}
#[tokio::test]
async fn cancel_during_response_drops_late_result() {
    let server=Server::new_at(vec![Response {status:200,mime:"application/json",chunks:vec![(200,serde_json::to_vec(&json!({"data":[{"b64_json":"AQ=="}]})).unwrap())]}],"/v1/images/generations");
    let cancel=Cancellation::default();let abort=cancel.clone();
    let timer=tokio::spawn(async move {tokio::time::sleep(Duration::from_millis(40)).await;abort.cancel();});
    assert_eq!(generate(&client(),&snapshot(server.url.clone()),"星星","亮闪闪",&cancel).await.unwrap_err().code,"CANCELLED");
    timer.await.unwrap();server.finish();
}
#[tokio::test]
async fn image_links_reject_private_addresses_protocols_credentials_and_bad_encoding() {
    for url in ["http://example.com/image.png","file:///tmp/image","https://localhost/image","https://127.0.0.1/image","https://[::1]/image","https://192.168.1.1/image","https://name:pass@example.com/image"] {
        assert!(parse(json!({"data":[{"url":url}]}),&Cancellation::default()).await.is_err());
    }
    assert!(public_url("https://images.example/image.png?signature=signed").is_ok());
}

#[tokio::test]
async fn reference_edit_sends_source_image_to_edits_without_text_only_fallback(){
 use std::io::{Read,Write};
 let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let url=format!("http://{}/v1",listener.local_addr().unwrap());
 let server=std::thread::spawn(move||{let (mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();let mut raw=vec![];let mut buf=[0u8;4096];let end=loop{let n=stream.read(&mut buf).unwrap();assert!(n>0);raw.extend_from_slice(&buf[..n]);if let Some(i)=raw.windows(4).position(|s|s==b"\r\n\r\n"){break i+4;}};let header=String::from_utf8_lossy(&raw[..end]).to_lowercase();assert!(header.starts_with("post /v1/images/edits "));assert!(header.contains("multipart/form-data; boundary=yanmew-"));let len:usize=header.lines().find_map(|s|s.strip_prefix("content-length:")).unwrap().trim().parse().unwrap();while raw.len()<end+len{let n=stream.read(&mut buf).unwrap();assert!(n>0);raw.extend_from_slice(&buf[..n]);}let payload=String::from_utf8_lossy(&raw[end..]);assert!(payload.contains("name=\"image\"; filename=\"reference.png\""));assert!(payload.contains("reference-image-content"));assert!(payload.contains("挥手"));assert!(payload.contains("name=\"background\"\r\n\r\ntransparent"));assert!(payload.contains("configured-image-model"));let body=r#"{"data":[{"b64_json":"AQID"}]}"#;write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();});
 let bytes=edit(&client(),&snapshot(url),b"reference-image-content","开心挥手",&Cancellation::default()).await.unwrap();assert_eq!(bytes,vec![1,2,3]);server.join().unwrap();
}

#[test]
fn endpoint_and_siliconflow_body_are_compatible_without_changing_connection() {
 for base in ["https://api.siliconflow.cn/v1","https://api.siliconflow.cn/v1/images/generations","https://api.siliconflow.cn/v1/images/generations/","https://api.siliconflow.cn"] {assert_eq!(endpoint(base,"generations"),"https://api.siliconflow.cn/v1/images/generations");}
 let mut cfg=snapshot("https://api.siliconflow.cn/v1/images/generations".into());cfg.provider.model="Tongyi-MAI/Z-Image-Turbo".into();
 let body=generation_body(&cfg,"画一只猫",false);assert_eq!(body,json!({"model":"Tongyi-MAI/Z-Image-Turbo","prompt":"画一只猫","image_size":"1024x1024"}));
 assert!(body.get("background").is_none());assert!(body.get("size").is_none());
}
#[tokio::test]
async fn generic_picture_uses_full_endpoint_and_accepts_images_envelope() {
 let server=Server::new_at(vec![response(200,json!({"images":[{"b64_json":"AQID"}]}))],"/v1/images/generations");
 let cfg=snapshot(format!("{}/images/generations",server.url));
 let bytes=generate_picture(&client(),&cfg,"一只猫在花园里",&Cancellation::default()).await.unwrap();assert_eq!(bytes,vec![1,2,3]);
 let requests=server.requests.lock().unwrap();assert_eq!(requests.len(),1);assert_eq!(requests[0]["prompt"],"一只猫在花园里");assert!(requests[0].get("background").is_none());drop(requests);server.finish();
}

#[test]
fn siliconflow_reference_request_uses_json_image_and_model_specific_fields() {
 let mut cfg=snapshot("https://api.siliconflow.cn/v1/images/generations".into());cfg.provider.model="Qwen/Qwen-Image-Edit-2509".into();
 let request=edit_request(&client(),&cfg,b"reference-image-content","保持当前人设，挥手",true).unwrap().build().unwrap();
 assert_eq!(request.url().as_str(),"https://api.siliconflow.cn/v1/images/generations");
 assert_eq!(request.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),"application/json");
 let value:Value=serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap();
 assert_eq!(value["model"],cfg.provider.model);assert_eq!(value["image"],format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(b"reference-image-content")));
 assert_eq!(value["prompt"],"保持当前人设，挥手");assert!(value.get("image_size").is_none());assert!(value.get("background").is_none());assert!(value.get("output_format").is_none());
 cfg.provider.model="Kwai-Kolors/Kolors".into();assert_eq!(siliconflow_edit_body(&cfg,b"image","挥手")["image_size"],"1024x1024");
}
#[tokio::test]
async fn siliconflow_reference_requests_reach_service_and_only_service_failure_is_reported() {
 for model in ["Tongyi-MAI/Z-Image-Turbo", "future-image-model"] {
  for status in [200,400] {
   let mut cfg=snapshot("https://api.siliconflow.cn/v1".into());cfg.provider.model=model.into();
   let http=client();
   let mut request=edit_request(&http,&cfg,b"reference","挥手",true).unwrap().build().unwrap();
   assert_eq!(request.url().as_str(),"https://api.siliconflow.cn/v1/images/generations");
   let server=Server::new_at(vec![response(status,if status==200{json!({"images":[{"b64_json":"AQID"}]})}else{json!({"error":"provider rejected the request"})})],"/v1/images/generations");
   // Keep the production request body and headers, but send only to this local fixture.
   *request.url_mut()=Url::parse(&format!("{}/images/generations",server.url)).unwrap();
   let response=http.search_http().execute(request).await.unwrap();
   if status==200 {
    let bytes=body(response,MAX_RESPONSE).await.unwrap();
    assert_eq!(parse_for(&cfg,serde_json::from_slice(&bytes).unwrap(),&Cancellation::default()).await.unwrap(),vec![1,2,3]);
   } else {
    let error=edit_failure(response.status().as_u16());
    assert!(error.message.contains("HTTP 400"));assert!(!error.message.contains("本次未发送"));
   }
   let requests=server.requests.lock().unwrap();assert_eq!(requests.len(),1);assert_eq!(requests[0]["model"],model);
   assert_eq!(requests[0]["image"],format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(b"reference")));
   drop(requests);server.finish();
   assert_eq!(generation_body(&cfg,"画猫",false)["model"],model);
  }
 }
}
#[tokio::test]
async fn reference_edit_failure_reports_http_reason_and_does_not_retry() {
 use std::io::{Read,Write};
 let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let url=format!("http://{}/v1",listener.local_addr().unwrap());
 let worker=std::thread::spawn(move||{let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut raw=Vec::new();let mut buf=[0u8;4096];let end=loop{let n=stream.read(&mut buf).unwrap();assert!(n>0);raw.extend_from_slice(&buf[..n]);if let Some(end)=raw.windows(4).position(|x|x==b"\r\n\r\n"){break end+4;}};let headers=String::from_utf8_lossy(&raw[..end]);let len:usize=headers.lines().find_map(|x|x.to_lowercase().strip_prefix("content-length:").map(|x|x.trim().parse().unwrap())).unwrap();while raw.len()<end+len{let n=stream.read(&mut buf).unwrap();assert!(n>0);raw.extend_from_slice(&buf[..n]);}stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();});
 let err=edit(&client(),&snapshot(url),b"reference","挥手",&Cancellation::default()).await.unwrap_err();assert!(err.message.contains("HTTP 404"));assert!(err.message.contains("图像编辑接口"));worker.join().unwrap();
 assert!(edit_failure(401).message.contains("Key"));assert!(edit_failure(429).message.contains("限流"));
}

#[test]
fn chat_reference_request_keeps_background_and_still_attaches_image() {
 let request=edit_request(&client(),&snapshot("http://127.0.0.1:1/v1".into()),b"reference-image","自己在海边",false).unwrap().build().unwrap();
 assert!(request.url().path().ends_with("/images/edits"));let payload=String::from_utf8_lossy(request.body().unwrap().as_bytes().unwrap());
 assert!(payload.contains("reference-image"));assert!(payload.contains("自己在海边"));assert!(!payload.contains("name=\"background\""));assert!(!payload.contains("name=\"output_format\""));
}
