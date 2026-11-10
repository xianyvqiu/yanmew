//! 社区发布目录只读服务；不安装插件，不接触钱包、存档或模型配置。
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashSet, time::Duration};

const CATALOG_URL: &str = "https://raw.githubusercontent.com/xianyvqiu/yanmew-community-games/main/catalog.json";
const RAW_PREFIX: &str = "/xianyvqiu/yanmew-community-games/";
const DOWNLOAD_PREFIX: &str = "/xianyvqiu/yanmew-community-games/releases/download/";
const MAX_CATALOG: usize = 1024 * 1024;
const MAX_README: usize = 256 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub api_version: String,
    pub repository: String,
    pub readme_url: String,
    pub readme_sha256: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub published_at: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Catalog { schema_version: u32, games: Vec<Game> }
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Readme { pub markdown: String, pub base_url: String }

fn unavailable() -> AppError {
    AppError::new("COMMUNITY_UNAVAILABLE", "暂时无法读取社区小游戏，请检查网络或稍后重试")
}
fn invalid() -> AppError {
    AppError::new("COMMUNITY_INVALID_CATALOG", "社区发布目录格式无效，请稍后刷新或反馈给维护者")
}
fn hash(value: &str) -> bool { value.len() == 64 && value.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) }
fn text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control)
}
fn https(value: &str, host: &str) -> AppResult<url::Url> {
    let url = url::Url::parse(value).map_err(|_| invalid())?;
    if value.len() > 2048 || url.scheme() != "https" || url.host_str() != Some(host)
        || url.port_or_known_default() != Some(443) || !url.username().is_empty()
        || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err(invalid());
    }
    Ok(url)
}
fn validate(game: &Game) -> AppResult<()> {
    if game.id.len() > 128 || game.id.split('.').count() < 2 || game.id.starts_with("com.yanmew.game.")
        || game.id.split('.').any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'))
        || !text(&game.name,100) || !text(&game.description,2000) || !text(&game.author,100)
        || !text(&game.version,128) || game.api_version != "1.0.0" || semver::Version::parse(&game.version).is_err()
        || !hash(&game.sha256) || !hash(&game.readme_sha256) || game.size_bytes == 0
        || game.size_bytes > crate::game_plugins::package::MAX_PACKAGE as u64
        || chrono::DateTime::parse_from_rfc3339(&game.published_at).is_err() { return Err(invalid()); }
    let repository = https(&game.repository,"github.com")?;
    if repository.path_segments().map(|parts| parts.filter(|s| !s.is_empty()).count()) != Some(2) { return Err(invalid()); }
    let readme = https(&game.readme_url,"raw.githubusercontent.com")?;
    let path: Vec<_> = readme.path().strip_prefix(RAW_PREFIX).ok_or_else(invalid)?.split('/').collect();
    if path.len()!=4 || path[0].len()!=40 || !path[0].bytes().all(|c|c.is_ascii_hexdigit())
        || path[1]!="games" || path[2]!=game.id || path[3]!="README.md" { return Err(invalid()); }
    let download=https(&game.download_url,"github.com")?;
    if !download.path().starts_with(DOWNLOAD_PREFIX) || !download.path().ends_with(".yanmew-game")
        || download.path().strip_prefix(DOWNLOAD_PREFIX).is_none_or(|p| !p.contains('/')) { return Err(invalid()); }
    Ok(())
}
fn parse_catalog(bytes: &[u8]) -> AppResult<Vec<Game>> {
    if bytes.len()>MAX_CATALOG { return Err(invalid()); }
    let catalog: Catalog=serde_json::from_slice(bytes).map_err(|_|invalid())?;
    if catalog.schema_version!=1 || catalog.games.len()>500 { return Err(invalid()); }
    let mut ids=HashSet::new();
    for game in &catalog.games { validate(game)?; if !ids.insert(&game.id) { return Err(invalid()); } }
    Ok(catalog.games)
}
fn decode_readme(bytes: Vec<u8>, expected: &str) -> AppResult<String> {
    if bytes.len()>MAX_README || format!("{:x}",Sha256::digest(&bytes))!=expected {
        return Err(AppError::new("COMMUNITY_README_CHANGED","小游戏介绍校验未通过，请刷新社区列表后重试"));
    }
    String::from_utf8(bytes).map_err(|_|invalid())
}
struct Client { http: reqwest::Client }
impl Client {
    fn new() -> AppResult<Self> {
        let http=reqwest::Client::builder().user_agent("YanMew/0.1 community browser")
            .redirect(reqwest::redirect::Policy::none()).connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15)).build().map_err(|_|unavailable())?;
        Ok(Self{http})
    }
    async fn bytes(&self,url:&str,limit:usize)->AppResult<Vec<u8>> {
        let mut response=self.http.get(url).send().await.map_err(|_|unavailable())?;
        if !response.status().is_success() || response.content_length().is_some_and(|n|n>limit as u64) { return Err(unavailable()); }
        let mut bytes=Vec::new();
        while let Some(chunk)=response.chunk().await.map_err(|_|unavailable())? {
            if bytes.len()+chunk.len()>limit { return Err(unavailable()); } bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    async fn catalog(&self)->AppResult<Vec<Game>> { parse_catalog(&self.bytes(CATALOG_URL,MAX_CATALOG).await?) }
    async fn find(&self,id:&str,version:&str)->AppResult<Game> {
        let game=self.catalog().await?.into_iter().find(|g|g.id==id)
            .ok_or_else(||AppError::new("COMMUNITY_GAME_NOT_FOUND","该小游戏已下架或不存在，请刷新列表"))?;
        if game.version!=version { return Err(AppError::new("COMMUNITY_GAME_CHANGED","小游戏版本已更新，请刷新列表后重试")); }
        Ok(game)
    }
}
pub async fn list()->AppResult<Vec<Game>> { Client::new()?.catalog().await }
pub async fn readme(id:&str,version:&str)->AppResult<Readme> {
    let client=Client::new()?;let game=client.find(id,version).await?;
    let bytes=client.bytes(&game.readme_url,MAX_README).await?;
    Ok(Readme{markdown:decode_readme(bytes,&game.readme_sha256)?,base_url:game.readme_url})
}
pub async fn download(id:&str,version:&str)->AppResult<String> { Ok(Client::new()?.find(id,version).await?.download_url) }

#[cfg(test)]
mod tests;
