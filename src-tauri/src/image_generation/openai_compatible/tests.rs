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
