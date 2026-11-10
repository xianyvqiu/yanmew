use super::*;
use reqwest::{Client, redirect::Policy};
use std::{fs, io::Write, time::Duration};

const MAX_PACKAGE: u64=1024*1024*1024;
#[derive(Deserialize)]
struct Asset { name:String, browser_download_url:String, size:u64, digest:Option<String> }
#[derive(Deserialize)]
struct Release { html_url:String, draft:bool, prerelease:bool, body:Option<String>, assets:Vec<Asset> }

pub fn client() -> Result<Client> {
    Client::builder().user_agent("YanMew-Updater/0.1.0")
        .connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(1200))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len()>5 || !allowed_host(attempt.url()) { attempt.stop() } else { attempt.follow() }
        })).build().map_err(|_|"无法初始化更新网络连接".into())
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
    while let Some(chunk)=response.chunk().await.map_err(|_|"更新信息下载中断，请检查网络后重试")? {
        if bytes.len() as u64+chunk.len() as u64>max {return Err("更新信息超过大小限制".into());}
        bytes.extend_from_slice(&chunk);
    } Ok(bytes)
}
fn digest(asset:&Asset)->Result<&str> {
    asset.digest.as_deref().and_then(|s|s.strip_prefix("sha256:")).filter(|s|valid_hash(s))
        .ok_or_else(||"Release附件缺少有效SHA-256，暂不能安全自动更新，请查看发布页".into())
}
pub async fn latest(client:&Client)->Result<Option<Candidate>> {
    let response=client.get("https://api.github.com/repos/xianyvqiu/yanmew/releases/latest")
        .header("Accept","application/vnd.github+json").header("X-GitHub-Api-Version","2026-03-10")
        .timeout(Duration::from_secs(30)).send().await.map_err(|_|"检查更新失败，请检查网络或代理后重试")?;
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
    let response=client.get(&asset.browser_download_url).timeout(Duration::from_secs(30)).send().await.map_err(|_|"更新清单下载失败")?;
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
    let mut response=client.get(&candidate.url).send().await.map_err(|_|"更新下载失败，请检查网络后重试")?;
    if !response.status().is_success(){return Err("更新包不可访问，请稍后重试".into());}
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|_|"无法创建更新临时文件")?;
    let result=async {
        let mut received=0u64;let mut hash=Sha256::new();
        while let Some(chunk)=response.chunk().await.map_err(|_|"更新包下载中断")? {
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
