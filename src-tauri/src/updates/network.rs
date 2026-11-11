use super::*;
use reqwest::{Client, redirect::Policy};
use std::{fs, io::Write, time::Duration};

const MAX_PACKAGE: u64=1024*1024*1024;
const CONNECT_TIMEOUT: Duration=Duration::from_secs(30);
const METADATA_TIMEOUT: Duration=Duration::from_secs(45);
const RETRY_DELAY: Duration=Duration::from_secs(1);
#[derive(Deserialize)]
struct Asset { name:String, browser_download_url:String, size:u64, digest:Option<String> }
#[derive(Deserialize)]
struct Release { html_url:String, draft:bool, prerelease:bool, body:Option<String>, assets:Vec<Asset> }

pub fn client() -> Result<Client> {
    Client::builder().user_agent("YanMew-Updater/0.1.0")
        .connect_timeout(CONNECT_TIMEOUT).timeout(Duration::from_secs(1200))
        .retry(reqwest::retry::never())
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len()>5 || !allowed_host(attempt.url()) { attempt.stop() } else { attempt.follow() }
        })).build().map_err(|_|"无法初始化更新网络连接".into())
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum FailureKind { ConnectTimeout,Timeout,Dns,Certificate,Handshake,Proxy,Refused,Interrupted,Other }
#[derive(Clone,Copy,Debug)]
struct TransportFailure {kind:FailureKind,retryable:bool}
impl TransportFailure {
    fn message(self,operation:&str,retried:bool)->String {
        let reason=match self.kind {
            FailureKind::ConnectTimeout=>"建立连接超时",
            FailureKind::Timeout=>"等待响应超时",
            FailureKind::Dns=>"域名解析失败",
            FailureKind::Certificate=>"TLS/证书校验失败",
            FailureKind::Handshake=>"TLS握手失败或连接被提前关闭",
            FailureKind::Proxy=>"代理连接或认证失败",
            FailureKind::Refused=>"连接被拒绝",
            FailureKind::Interrupted=>"网络连接中断",
            FailureKind::Other=>"网络请求未完成",
        };
        let advice=match self.kind {
            FailureKind::Certificate=>"请检查系统时间与证书，或稍后重试。",
            FailureKind::Proxy=>"请检查代理连接及认证设置。",
            FailureKind::Dns=>"请检查DNS或稍后重试。",
            _=>"请稍后重试或查看正式发布页。",
        };
        format!("{operation}失败：{reason}。{}{advice}",if retried{"已自动重试1次；"}else{""})
    }
}
fn classify_transport(error:&(dyn std::error::Error+'static),mut timeout:bool,connecting:bool)->TransportFailure {
    let mut source=Some(error);
    let (mut dns,mut certificate,mut handshake,mut proxy,mut refused,mut interrupted)=(false,false,false,false,false,false);
    // 仅在内存中识别原因；不将原文、地址或代理凭据传入界面或日志。
    while let Some(cause)=source {
        if let Some(io)=cause.downcast_ref::<std::io::Error>() {
            dns|=matches!(io.raw_os_error(),Some(11001|11002|11003|11004));
            refused|=io.kind()==std::io::ErrorKind::ConnectionRefused;
            interrupted|=matches!(io.kind(),std::io::ErrorKind::UnexpectedEof|std::io::ErrorKind::ConnectionReset|std::io::ErrorKind::ConnectionAborted|std::io::ErrorKind::BrokenPipe);
            timeout|=io.kind()==std::io::ErrorKind::TimedOut;
        }
        let text=cause.to_string().to_ascii_lowercase();
        // native-tls可能只保留本地化说明，仍可从标准os error后缀识别错误码。
        let code=cause.downcast_ref::<std::io::Error>().and_then(|io|io.raw_os_error())
            .or_else(||text.rsplit_once("(os error ").and_then(|(_,suffix)|suffix.strip_suffix(')')).and_then(|value|value.parse::<i32>().ok()));
        if let Some(code)=code {
            dns|=matches!(code,11001|11002|11003|11004);refused|=code==10061;timeout|=code==10060;
            interrupted|=matches!(code,10053|10054|10058|109|232);
            certificate|=matches!(code as u32,0x80092010..=0x80092014|0x800b0101..=0x800b0114|0x80090322|0x80090325|0x80090327|0x80090328|0x80090349);
        }
        dns|=["dns error","failed to lookup address","no such host is known","name or service not known"].iter().any(|value|text.contains(value));
        certificate|=["certificate","cert_e_","crypt_e_","sec_e_untrusted_root","sec_e_wrong_principal","sec_e_cert_","0x80092013"].iter().any(|value|text.contains(value));
        handshake|=text.contains("handshake");
        proxy|=["proxy authentication","proxy connect","tunnel error","unsuccessful tunnel"].iter().any(|value|text.contains(value));
        interrupted|=["unexpected eof","connection reset","connection aborted","connection closed before message completed"].iter().any(|value|text.contains(value));
        source=cause.source();
    }
    let kind=if certificate{FailureKind::Certificate}else if proxy{FailureKind::Proxy}else if dns{FailureKind::Dns}
        else if refused{FailureKind::Refused}else if timeout{if connecting{FailureKind::ConnectTimeout}else{FailureKind::Timeout}}
        else if handshake{FailureKind::Handshake}else if interrupted{FailureKind::Interrupted}else{FailureKind::Other};
    let retryable=matches!(kind,FailureKind::ConnectTimeout|FailureKind::Timeout|FailureKind::Interrupted)
        ||kind==FailureKind::Handshake&&interrupted;
    TransportFailure{kind,retryable}
}
fn transport(error:&reqwest::Error)->TransportFailure {classify_transport(error,error.is_timeout(),error.is_connect())}
async fn send_metadata(request:reqwest::RequestBuilder,operation:&str,delay:Duration)->Result<reqwest::Response> {
    for attempt in 0..2 {
        let next=request.try_clone().ok_or_else(||format!("{operation}失败：无法准备更新请求"))?;
        match next.timeout(METADATA_TIMEOUT).send().await {
            Ok(response)=>return Ok(response),
            Err(error)=>{
                let failure=transport(&error);
                if attempt==0&&failure.retryable {tokio::time::sleep(delay).await;continue;}
                return Err(failure.message(operation,attempt>0));
            }
        }
    }
    unreachable!("更新请求最多两次")
}
fn allowed_host(url:&url::Url)->bool {
    url.scheme()=="https"&&url.username().is_empty()&&url.password().is_none()&&url.port().is_none()
        && matches!(url.host_str(),Some("api.github.com"|"github.com"|"release-assets.githubusercontent.com"|"objects.githubusercontent.com"))
}
fn release_url(value:&str,download:bool)->bool {
    let Ok(url)=url::Url::parse(value) else {return false;};
    allowed_host(&url)&&url.host_str()==Some("github.com")&&url.query().is_none()&&url.fragment().is_none()
        &&url.path().starts_with(if download {"/xianyvqiu/yanmew/releases/download/"}else{"/xianyvqiu/yanmew/releases/tag/"})
}
async fn bounded(mut response:reqwest::Response,max:u64)->Result<Vec<u8>> {
    if response.content_length().is_some_and(|n|n>max) {return Err("更新信息超过大小限制".into());}
    let mut bytes=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|error|transport(&error).message("读取更新信息",false))? {
        if bytes.len() as u64+chunk.len() as u64>max {return Err("更新信息超过大小限制".into());}
        bytes.extend_from_slice(&chunk);
    } Ok(bytes)
}
fn digest(asset:&Asset)->Result<&str> {
    asset.digest.as_deref().and_then(|s|s.strip_prefix("sha256:")).filter(|s|valid_hash(s))
        .ok_or_else(||"Release附件缺少有效SHA-256，暂不能安全自动更新，请查看发布页".into())
}
pub async fn latest(client:&Client)->Result<Option<Candidate>> {
    let request=client.get("https://api.github.com/repos/xianyvqiu/yanmew/releases/latest")
        .header("Accept","application/vnd.github+json").header("X-GitHub-Api-Version","2026-03-10");
    let response=send_metadata(request,"检查更新",RETRY_DELAY).await?;
    match response.status().as_u16() {
        404=>return Ok(None),
        403|429=>return Err("GitHub暂时限制访问，请稍后重试或打开发布页".into()),
        200=>{}, _=>return Err("无法读取GitHub发布信息，请稍后重试".into()),
    }
    let bytes=bounded(response,2*1024*1024).await?;
    let release:Release=serde_json::from_slice(&bytes).map_err(|_|"GitHub发布信息格式无效")?;
    if release.draft||release.prerelease||!release_url(&release.html_url,false) {return Err("发布信息不是本仓库正式版本".into());}
    let manifests:Vec<_>=release.assets.iter().filter(|a|a.name=="yanmew-update.json").collect();
    if manifests.len()!=1 {return Err("正式Release尚未提供更新清单，请查看发布页；没有修改本地程序".into());}
    let asset=manifests[0];
    if asset.size>65536||!release_url(&asset.browser_download_url,true) {return Err("更新清单地址或大小无效".into());}
    let response=send_metadata(client.get(&asset.browser_download_url),"读取更新清单",RETRY_DELAY).await?;
    if !response.status().is_success(){return Err("更新清单不可访问".into());}
    let bytes=bounded(response,65536).await?;
    if bytes.len() as u64!=asset.size||format!("{:x}",Sha256::digest(&bytes))!=digest(asset)?.to_ascii_lowercase() {return Err("更新清单校验失败".into());}
    candidate(&release,&bytes).map(Some)
}
fn candidate(release:&Release,bytes:&[u8])->Result<Candidate> {
    let manifest:ReleaseManifest=serde_json::from_slice(bytes).map_err(|_|"更新清单格式不支持")?;
    if manifest.schema_version!=1||!valid_build(&manifest.build_id)||!valid_hash(&manifest.sha256)
        ||semver::Version::parse(&manifest.version).is_err()||manifest.size==0||manifest.size>MAX_PACKAGE {
        return Err("更新清单版本或大小无效".into());
    }
    let assets:Vec<_>=release.assets.iter().filter(|a|a.name==manifest.asset_name).collect();
    if assets.len()!=1 {return Err("Release缺少对应便携更新包".into());}
    let asset=assets[0];
    if !release_url(&asset.browser_download_url,true)||asset.size!=manifest.size||digest(asset)?.to_ascii_lowercase()!=manifest.sha256.to_ascii_lowercase() {
        return Err("更新包的来源、大小或SHA-256不一致".into());
    }
    Ok(Candidate{manifest,url:asset.browser_download_url.clone(),release_url:release.html_url.clone(),notes:release.body.clone().unwrap_or_default().chars().take(12000).collect()})
}
pub async fn download(client:&Client,candidate:&Candidate,path:&Path,progress:impl Fn(u64,u64))->Result<()> {
    if !release_url(&candidate.url,true)||candidate.manifest.size>MAX_PACKAGE {return Err("更新下载地址或大小无效".into());}
    let mut response=client.get(&candidate.url).send().await.map_err(|error|transport(&error).message("下载更新包",false))?;
    if !response.status().is_success(){return Err("更新包不可访问，请稍后重试".into());}
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|_|"无法创建更新临时文件")?;
    let result=async {
        let mut received=0u64;let mut hash=Sha256::new();
        while let Some(chunk)=response.chunk().await.map_err(|error|transport(&error).message("下载更新包",false))? {
            received+=chunk.len() as u64;if received>candidate.manifest.size {return Err("下载大小超过清单".into());}
            file.write_all(&chunk).map_err(|_|"更新包写入失败，请检查磁盘空间")?;hash.update(&chunk);progress(received,candidate.manifest.size);
        }
        file.sync_all().map_err(|_|"更新包保存失败")?;
        if received!=candidate.manifest.size||format!("{:x}",hash.finalize())!=candidate.manifest.sha256.to_ascii_lowercase(){return Err("更新包不完整或校验失败；没有修改旧程序".into());}
        Ok(())
    }.await;
    drop(file);if result.is_err(){let _=fs::remove_file(path);}result
}

#[cfg(test)] mod tests {
    use super::*;
    use std::{io::{Read,Write},net::TcpListener,thread,time::Instant};
    fn fixture(replies:Vec<(&'static [u8],Duration)>)->(String,thread::JoinHandle<usize>) {
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();
        let address=format!("http://{}",listener.local_addr().unwrap());
        let server=thread::spawn(move||{
            let count=replies.len();let mut workers=Vec::new();
            for (reply,delay) in replies {
                let deadline=Instant::now()+Duration::from_secs(3);
                let mut stream=loop {match listener.accept(){
                    Ok((stream,_))=>break stream,
                    Err(error) if error.kind()==std::io::ErrorKind::WouldBlock&&Instant::now()<deadline=>thread::sleep(Duration::from_millis(5)),
                    Err(error)=>panic!("未收到预期的测试请求：{error}"),
                }};
                workers.push(thread::spawn(move||{
                    stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                    let mut request=Vec::new();let mut byte=[0u8;1];
                    while request.len()<8192&&!request.ends_with(b"\r\n\r\n") {
                        if stream.read(&mut byte).unwrap_or(0)==0{return;}
                        request.push(byte[0]);
                    }
                    thread::sleep(delay);let _=stream.write_all(reply);
                }));
            }
            for worker in workers{worker.join().unwrap();}count
        });(address,server)
    }
    fn local_client()->Client {Client::builder().no_proxy().retry(reqwest::retry::never()).build().unwrap()}
    fn run<T>(future:impl std::future::Future<Output=T>)->T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
    }
    #[derive(Debug)] struct Wrapped(std::io::Error);
    impl std::fmt::Display for Wrapped {fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{write!(f,"request failed")}}
    impl std::error::Error for Wrapped {fn source(&self)->Option<&(dyn std::error::Error+'static)>{Some(&self.0)}}
    #[test] fn transport_causes_are_distinct_without_exposing_credentials(){
        for (cause,kind,retryable) in [
            ("unexpected EOF during handshake",FailureKind::Handshake,true),
            ("TLS handshake protocol unsupported",FailureKind::Handshake,false),
            ("certificate verify failed during handshake",FailureKind::Certificate,false),
            ("CRYPT_E_REVOCATION_OFFLINE",FailureKind::Certificate,false),
            ("unsuccessful tunnel: proxy authentication required",FailureKind::Proxy,false),
            ("dns error: no such host is known",FailureKind::Dns,false),
            ("connection closed before message completed",FailureKind::Interrupted,true),
            ("unrecognized failure",FailureKind::Other,false),
        ] {
            let error=Wrapped(std::io::Error::other(format!("{cause}; https://proxy-user:private-secret@example.test")));
            let failure=classify_transport(&error,false,true);assert_eq!(failure.kind,kind);assert_eq!(failure.retryable,retryable);
            let message=failure.message("检查更新",false);
            for private in ["private-secret","proxy-user","example.test"]{assert!(!message.contains(private));}
        }
        let refused=Wrapped(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        let failure=classify_transport(&refused,false,true);assert_eq!(failure.kind,FailureKind::Refused);assert!(!failure.retryable);
        let dns=Wrapped(std::io::Error::from_raw_os_error(11001));assert_eq!(classify_transport(&dns,false,true).kind,FailureKind::Dns);
    }
    #[test] fn connection_and_response_timeouts_are_classified_and_certificate_errors_are_not_retried(){
        let error=Wrapped(std::io::Error::other("operation timed out"));
        for (connecting,kind) in [(true,FailureKind::ConnectTimeout),(false,FailureKind::Timeout)] {
            let failure=classify_transport(&error,true,connecting);assert_eq!(failure.kind,kind);assert!(failure.retryable);
        }
        let certificate=Wrapped(std::io::Error::other("certificate retrieval timed out"));
        let failure=classify_transport(&certificate,true,true);assert_eq!(failure.kind,FailureKind::Certificate);assert!(!failure.retryable);
    }
    #[test] fn localized_windows_errors_keep_their_categories_without_leaking_details(){
        for (code,kind,retryable) in [
            (10053,FailureKind::Interrupted,true),(10054,FailureKind::Interrupted,true),
            (10060,FailureKind::ConnectTimeout,true),(10061,FailureKind::Refused,false),
            (11001,FailureKind::Dns,false),(0x80092013u32 as i32,FailureKind::Certificate,false),
            (0x800b0109u32 as i32,FailureKind::Certificate,false),
        ] {
            let error=Wrapped(std::io::Error::other(format!("private-secret：本地化连接错误。 (os error {code})")));
            let failure=classify_transport(&error,false,true);assert_eq!(failure.kind,kind);assert_eq!(failure.retryable,retryable);
            assert!(!failure.message("检查更新",false).contains("private-secret"));
        }
    }
    #[test] fn interrupted_metadata_request_retries_once_and_recovers(){
        let (url,server)=fixture(vec![(b"",Duration::ZERO),(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",Duration::ZERO)]);
        let response=run(send_metadata(local_client().get(url),"检查更新",Duration::ZERO)).unwrap();
        assert_eq!(response.status(),404);assert_eq!(server.join().unwrap(),2);
    }
    #[test] fn interrupted_metadata_request_stops_after_two_attempts(){
        let (url,server)=fixture(vec![(b"",Duration::ZERO),(b"",Duration::ZERO)]);
        let error=run(send_metadata(local_client().get(url),"检查更新",Duration::ZERO)).unwrap_err();
        assert!(error.contains("网络连接中断"),"{error}");assert!(error.contains("已自动重试1次"));assert_eq!(server.join().unwrap(),2);
    }
    #[test] fn metadata_response_timeout_retries_once_and_recovers(){
        let reply=b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (url,server)=fixture(vec![(reply,Duration::from_millis(350)),(reply,Duration::ZERO)]);
        let http=Client::builder().no_proxy().retry(reqwest::retry::never()).read_timeout(Duration::from_millis(100)).build().unwrap();
        let response=run(send_metadata(http.get(url),"检查更新",RETRY_DELAY)).unwrap();
        assert_eq!(response.status(),404);assert_eq!(server.join().unwrap(),2);
    }
    #[test] fn http_statuses_are_not_treated_as_transport_failures_or_retried(){
        for (reply,status) in [
            (&b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"[..],404),
            (&b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n"[..],403),
            (&b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n"[..],429),
            (&b"HTTP/1.1 500 Server Error\r\nContent-Length: 0\r\n\r\n"[..],500),
        ] {
            let (url,server)=fixture(vec![(reply,Duration::ZERO)]);
            let response=run(send_metadata(local_client().get(url),"检查更新",Duration::ZERO)).unwrap();
            assert_eq!(response.status(),status);assert_eq!(server.join().unwrap(),1);
        }
    }
    #[test] fn proxy_authentication_failure_is_not_retried(){
        let (url,server)=fixture(vec![(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",Duration::ZERO)]);
        let http=Client::builder().no_proxy().proxy(reqwest::Proxy::https(url).unwrap()).retry(reqwest::retry::never()).build().unwrap();
        let error=run(send_metadata(http.get("https://api.github.com/repos/xianyvqiu/yanmew/releases/latest"),"检查更新",Duration::ZERO)).unwrap_err();
        assert!(error.contains("代理连接或认证失败"),"{error}");assert!(!error.contains("已自动重试"));assert_eq!(server.join().unwrap(),1);
    }
    #[test] fn interrupted_metadata_body_reports_failure_without_redownloading(){
        let (url,server)=fixture(vec![(b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\nshort",Duration::ZERO)]);
        let error=run(async{let response=send_metadata(local_client().get(url),"检查更新",Duration::ZERO).await?;bounded(response,64).await}).unwrap_err();
        assert!(error.contains("读取更新信息失败"),"{error}");assert_eq!(server.join().unwrap(),1);
    }
    #[test] fn rejects_other_repositories_credentials_and_untrusted_redirects(){
        for value in ["https://evil.test/xianyvqiu/yanmew/releases/download/v/a.zip","https://github.com/other/repo/releases/download/v/a.zip","http://github.com/xianyvqiu/yanmew/releases/download/v/a.zip","https://x@github.com/xianyvqiu/yanmew/releases/download/v/a.zip"] {assert!(!release_url(value,true));}
        assert!(!allowed_host(&url::Url::parse("https://localhost/a").unwrap()));
        assert!(allowed_host(&url::Url::parse("https://release-assets.githubusercontent.com/a").unwrap()));
    }
    #[test] fn requires_matching_release_asset_digest_and_size(){
        let hash="a".repeat(64);let manifest=ReleaseManifest{schema_version:1,version:"0.1.0".into(),build_id:"20261110-170000".into(),asset_name:"portable.zip".into(),size:4,sha256:hash.clone()};
        let mut release=Release{html_url:format!("{REPOSITORY}/releases/tag/test"),draft:false,prerelease:false,body:None,assets:vec![Asset{name:"portable.zip".into(),browser_download_url:format!("{REPOSITORY}/releases/download/test/portable.zip"),size:4,digest:Some(format!("sha256:{hash}"))}]};
        let bytes=serde_json::to_vec(&manifest).unwrap();assert!(candidate(&release,&bytes).is_ok());
        release.assets[0].digest=None;assert!(candidate(&release,&bytes).is_err());
        release.assets[0].digest=Some(format!("sha256:{hash}"));release.assets[0].size=5;assert!(candidate(&release,&bytes).is_err());
    }
    #[test] #[ignore="需显式运行的公开GitHub只读网络检查"]
    fn live_release_probe(){
        let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let result=runtime.block_on(async{latest(&client().unwrap()).await});
        match result{Ok(Some(candidate))=>println!("公开正式版本：{} {}",candidate.manifest.version,candidate.manifest.build_id),Ok(None)=>println!("暂无可公开访问的正式Release；没有下载或修改程序"),Err(error)=>panic!("原生更新网络检查失败：{error}")}
    }
}
