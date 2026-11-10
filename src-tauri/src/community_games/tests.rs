use super::*;
use serde_json::{json,Value};
fn game()->Value {
    json!({"id":"org.example.puzzle","name":"拼图","description":"本地拼图游戏","author":"作者","version":"1.0.0","apiVersion":"1.0.0","repository":"https://github.com/example/puzzle","readmeUrl":format!("https://raw.githubusercontent.com/xianyvqiu/yanmew-community-games/{}/games/org.example.puzzle/README.md","a".repeat(40)),"readmeSha256":"a".repeat(64),"downloadUrl":"https://github.com/xianyvqiu/yanmew-community-games/releases/download/org.example.puzzle%2Fv1.0.0/org.example.puzzle-1.0.0.yanmew-game","sha256":"b".repeat(64),"sizeBytes":1200,"publishedAt":"2026-11-09T00:00:00Z"})
}
fn catalog(games:Vec<Value>)->Vec<u8> {serde_json::to_vec(&json!({"schemaVersion":1,"games":games})).unwrap()}
#[test]
fn empty_and_reviewed_catalogs_are_valid_but_duplicate_ids_are_not() {
    assert!(parse_catalog(&catalog(vec![])).unwrap().is_empty());
    assert_eq!(parse_catalog(&catalog(vec![game()])).unwrap()[0].name,"拼图");
    assert!(parse_catalog(&catalog(vec![game(),game()])).is_err());
    assert!(parse_catalog(br#"{"schemaVersion":2,"games":[]}"#).is_err());
    assert!(parse_catalog(br#"{"schemaVersion":1,"games":[],"script":"run"}"#).is_err());
}
#[test]
fn catalog_rejects_untrusted_urls_credentials_unpinned_readmes_and_invalid_metadata() {
    for (field,value) in [
        ("downloadUrl",json!("https://evil.example/game.yanmew-game")),
        ("downloadUrl",json!("https://github.com/other/repo/releases/download/v1/game.yanmew-game")),
        ("downloadUrl",json!("https://github.com/xianyvqiu/yanmew-community-games/releases/download/v1/game.exe")),
        ("downloadUrl",json!("https://user:password@github.com/xianyvqiu/yanmew-community-games/releases/download/v1/game.yanmew-game")),
        ("readmeUrl",json!("https://raw.githubusercontent.com/xianyvqiu/yanmew-community-games/main/games/org.example.puzzle/README.md")),
        ("readmeUrl",json!(format!("https://raw.githubusercontent.com/other/repo/{}/games/org.example.puzzle/README.md","a".repeat(40)))),
        ("sha256",json!("invalid")),("sizeBytes",json!(67_108_865)),("version",json!("latest")),
        ("apiVersion",json!("2.0.0")),("name",json!("title\nscript")),("publishedAt",json!("yesterday")),
        ("id",json!("../evil")),("id",json!("com.yanmew.game.fake")),
    ] {let mut row=game();row[field]=value;assert!(parse_catalog(&catalog(vec![row])).is_err(),"{field}");}
}
#[test]
fn readme_must_match_published_digest_and_utf8() {
    let content=b"# Game\n".to_vec();let digest=format!("{:x}",Sha256::digest(&content));
    assert_eq!(decode_readme(content.clone(),&digest).unwrap(),"# Game\n");
    assert!(decode_readme(content,&"0".repeat(64)).is_err());
    let bytes=vec![255];let digest=format!("{:x}",Sha256::digest(&bytes));assert!(decode_readme(bytes,&digest).is_err());
}
#[test]
fn public_reader_rejects_http_errors_and_oversize_responses() {
    use std::io::{Read,Write};
    tauri::async_runtime::block_on(async {
        for (status,body,limit,success) in [("200 OK","hello",16,true),("200 OK","too large",3,false),("302 Found","moved",16,false),("404 Not Found","missing",16,false)] {
            let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
            let worker=std::thread::spawn(move||{let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut buffer=[0;4096];stream.read(&mut buffer).unwrap();write!(stream,"HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();});
            let http=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).build().unwrap();
            let client=Client{api:http.clone(),http};
            assert_eq!(client.bytes(&format!("http://{address}/"),limit).await.is_ok(),success);worker.join().unwrap();
        }
    });
}
#[test]
#[ignore="手动核对已发布的公开社区目录，不安装或下载插件"]
fn published_community_catalog_read_only_smoke() {
    let games=tauri::async_runtime::block_on(list()).unwrap();println!("community catalog verified: {} published games",games.len());
}
#[test]
fn official_readme_route_keeps_the_reviewed_repository_path_and_revision() {
    let valid:Game=serde_json::from_value(game()).unwrap();
    assert_eq!(readme_api_url(&valid).unwrap(),format!("{CONTENTS_API_PREFIX}games/org.example.puzzle/README.md?ref={}","a".repeat(40)));
    for url in ["https://raw.githubusercontent.com/other/repo/main/README.md","https://raw.githubusercontent.com/xianyvqiu/yanmew-community-games/main/games/org.example.puzzle/README.md","https://token@raw.githubusercontent.com/xianyvqiu/yanmew-community-games/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/games/org.example.puzzle/README.md"] {
        let mut invalid=valid.clone();invalid.readme_url=url.into();assert!(readme_api_url(&invalid).is_err());
    }
}
#[test]
fn raw_success_does_not_require_the_api_route() {
    use std::io::{Read,Write};
    tauri::async_runtime::block_on(async {
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
        let worker=std::thread::spawn(move||{let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut buffer=[0;4096];stream.read(&mut buffer).unwrap();write!(stream,"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nraw").unwrap();});
        let http=reqwest::Client::builder().no_proxy().build().unwrap();let client=Client{api:http.clone(),http};
        assert_eq!(client.github_bytes(&format!("http://{address}/raw"),"http://127.0.0.1:1/unavailable",16).await.unwrap(),b"raw");worker.join().unwrap();
    });
}
#[test]
fn api_fallback_is_anonymous_raw_content_and_preserves_limits_and_digest() {
    use std::io::{Read,Write};
    tauri::async_runtime::block_on(async {
        for (limit,success) in [(32,true),(3,false)] {
            let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
            let worker=std::thread::spawn(move||{
                for (index,(status,body)) in [("503 Service Unavailable","down"),("200 OK","# Game\n")].iter().enumerate() {
                    let(mut stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut buffer=[0;4096];let count=stream.read(&mut buffer).unwrap();let request=String::from_utf8_lossy(&buffer[..count]).to_lowercase();
                    assert!(!request.contains("authorization:"));
                    if index==1 {assert!(request.starts_with("get /contents/readme?ref=fixed "));assert!(request.contains("accept: application/vnd.github.raw+json"));}
                    write!(stream,"HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
                }
            });
            let http=reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).build().unwrap();let client=Client{api:http.clone(),http};
            let result=client.github_bytes(&format!("http://{address}/raw"),&format!("http://{address}/contents/readme?ref=fixed"),limit).await;
            assert_eq!(result.is_ok(),success);
            if let Ok(bytes)=result {assert_eq!(decode_readme(bytes.clone(),&format!("{:x}",Sha256::digest(&bytes))).unwrap(),"# Game\n");assert!(decode_readme(bytes,&"0".repeat(64)).is_err());}
            worker.join().unwrap();
        }
    });
}
