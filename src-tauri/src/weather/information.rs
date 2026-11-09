//! 公开气象资讯及派生缓存；不改聊天历史或数据库结构。
mod care;
pub(crate) async fn retry_care(runtime: Runtime, revision:u64, kind:&str, events:Arc<dyn crate::events::PetEventSink>)->AppResult<()> {
    if !matches!(kind,"warnings"|"typhoons"){return Err(AppError::new("INVALID_INPUT","请选择预警或台风关怀"));}
    let result=care::generate(&runtime,revision,kind=="warnings",kind=="typhoons",true,events.clone()).await;
    events.weather_changed();result
}
mod parse;
const SHANGHAI_WARNING_INDEX: &str = "https://d1.weather.com.cn/shweather/alarmIndex.json";
#[cfg(test)]
mod tests;
use crate::{
    error::{AppError, AppResult},
    location::Position,
    runtime::Runtime,
    state::Cancellation,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub name: String,
    pub province: String,
    pub city: String,
    pub district: String,
    #[serde(default)]
    pub country_code: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    pub fetched_at: DateTime<Utc>,
    pub feels_like: Option<f64>,
    pub humidity: Option<f64>,
    pub wind_speed: Option<f64>,
    pub wind_direction: Option<f64>,
    pub hourly: Vec<Hour>,
    pub daily: Vec<Day>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hour {
    pub time: i64,
    pub temperature: Option<f64>,
    pub rain: Option<f64>,
    pub code: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    pub date: String,
    pub low: Option<f64>,
    pub high: Option<f64>,
    pub rain: Option<f64>,
    pub uv: Option<f64>,
    pub sunrise: Option<i64>,
    pub sunset: Option<i64>,
    pub code: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub title: String,
    pub url: String,
    pub summary: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub id: String,
    pub title: String,
    pub description: String,
    pub level: String,
    pub issued_at: String,
    pub url: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub longitude: f64,
    pub latitude: f64,
    pub forecast: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Typhoon {
    pub id: String,
    pub name: String,
    pub intensity: String,
    pub published_at: DateTime<Utc>,
    pub wind_speed: Option<f64>,
    pub distance_km: f64,
    pub points: Vec<Point>,
    pub url: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Care {
    pub persona_id: String,
    pub name: String,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feed<T> {
    pub items: Vec<T>,
    pub checked_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub care: Option<Care>,
    pub attempted: BTreeMap<String, String>,
    #[serde(default)]
    pub care_status: BTreeMap<String, CareStatus>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CareStatus {pub event_key:String,pub state:String,pub error:Option<String>}
impl<T> Default for Feed<T> {
    fn default() -> Self {
        Self {
            items: vec![],
            checked_at: None,
            error: None,
            care: None,
            attempted: BTreeMap::new(),
            care_status: BTreeMap::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub position: Position,
    pub region: Option<Region>,
    pub forecast: Option<Forecast>,
    pub forecast_error: Option<String>,
    pub warnings: Feed<Warning>,
    pub typhoons: Feed<Typhoon>,
    pub links: Vec<Link>,
    pub search_error: Option<String>,
}

fn unavailable() -> AppError {
    AppError::new(
        "WEATHER_INFORMATION_UNAVAILABLE",
        "公开气象信息暂时未能获取，请稍后重试或配置和风天气兜底",
    )
}
struct Client {
    http: reqwest::Client,
}
impl Client {
    fn new() -> AppResult<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .user_agent("YanMew/0.1 (+https://github.com/xianyvqiu/yanmew)")
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(4))
                .timeout(Duration::from_secs(8))
                .build()
                .map_err(|_| unavailable())?,
        })
    }
    async fn text(&self, url: &str, cancel: &Cancellation) -> AppResult<String> {
        let mut response = cancel
            .wait(Duration::from_secs(10), async {
                let mut request = self.http.get(url);
                if url == SHANGHAI_WARNING_INDEX {
                    request = request.header(reqwest::header::REFERER, "https://www.weather.com.cn/shanghai/");
                }
                request.send().await.map_err(|_| unavailable())
            })
            .await?;
        if !response.status().is_success() {
            return Err(unavailable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = cancel
            .wait(Duration::from_secs(10), async {
                response.chunk().await.map_err(|_| unavailable())
            })
            .await?
        {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err(unavailable());
            }
            bytes.extend_from_slice(&chunk);
        }
        String::from_utf8(bytes).map_err(|_| unavailable())
    }
    async fn json(&self, url: &str, cancel: &Cancellation) -> AppResult<Value> {
        serde_json::from_str(&self.text(url, cancel).await?).map_err(|_| unavailable())
    }
    async fn region(&self, p: &Position, cancel: &Cancellation) -> AppResult<Region> {
        let url=format!("https://nominatim.openstreetmap.org/reverse?format=jsonv2&lat={:.3}&lon={:.3}&zoom=10&accept-language=zh-CN",p.latitude,p.longitude);
        parse::region(&self.json(&url, cancel).await?)
    }
    async fn forecast(
        &self,
        p: &Position,
        now: DateTime<Utc>,
        cancel: &Cancellation,
    ) -> AppResult<Forecast> {
        let url=format!("https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=apparent_temperature,relative_humidity_2m,wind_speed_10m,wind_direction_10m&hourly=temperature_2m,weather_code,precipitation_probability&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset,uv_index_max&forecast_days=7&timezone=auto&timeformat=unixtime",p.latitude,p.longitude);
        parse::forecast(&self.json(&url, cancel).await?, now)
    }
    async fn warnings(
        &self,
        region: Option<&Region>,
        now: DateTime<Utc>,
        cancel: &Cancellation,
    ) -> AppResult<Vec<Warning>> {
        let region = region.ok_or_else(unavailable)?;
        if region.province.is_empty() || region.country_code != "cn" {
            return Err(unavailable());
        }
        if parse::is_shanghai(region) {
            let supplement = async {
                let body = self.text(SHANGHAI_WARNING_INDEX, cancel).await?;
                parse::shanghai_warnings(&body, region)
            };
            // 两个来源任一失败，都不把部分查询当作完整的“最新空结果”。
            let (mut items, extra) = tokio::try_join!(self.nmc_warnings(region, now, cancel), supplement)?;
            for item in extra {
                if !items.iter().any(|old| parse::same_warning(old, &item)) { items.push(item); }
            }
            return Ok(items);
        }
        self.nmc_warnings(region, now, cancel).await
    }
    async fn nmc_warnings(&self, region: &Region, now: DateTime<Utc>, cancel: &Cancellation) -> AppResult<Vec<Warning>> {
        let mut items = vec![];
        for page in 1..=10 {
            let mut url =
                url::Url::parse("https://www.nmc.cn/rest/findAlarm").map_err(|_| unavailable())?;
            url.query_pairs_mut().extend_pairs([
                ("pageNo", page.to_string()),
                ("pageSize", "100".into()),
                ("province", region.province.clone()),
                ("signaltype", "".into()),
                ("signallevel", "".into()),
            ]);
            let data = self.json(url.as_str(), cancel).await?;
            let (records, total) = parse::warnings(&data, region, now)?;
            items.extend(records);
            if page >= total {
                items.sort_by(|a, b| b.issued_at.cmp(&a.issued_at));
                let mut seen = std::collections::HashSet::new();
                items.retain(|item| seen.insert(item.id.clone()));
                return Ok(items);
            }
        }
        Err(unavailable())
    }
    async fn typhoons(
        &self,
        p: &Position,
        now: DateTime<Utc>,
        cancel: &Cancellation,
    ) -> AppResult<Vec<Typhoon>> {
        let list = parse::jsonp(
            &self
                .text(
                    "https://typhoon.nmc.cn/weatherservice/typhoon/jsons/list_default",
                    cancel,
                )
                .await?,
        )?;
        let storms = list["typhoonList"].as_array().ok_or_else(unavailable)?;
        let active: Vec<_> = storms
            .iter()
            .filter(|row| row[7].as_str() == Some("start"))
            .collect();
        if active.len() > 10 {
            return Err(unavailable());
        }
        let mut result = vec![];
        for storm in active {
            let id = storm[0].as_u64().ok_or_else(unavailable)?;
            let value = parse::jsonp(
                &self
                    .text(
                        &format!("https://typhoon.nmc.cn/weatherservice/typhoon/jsons/view_{id}"),
                        cancel,
                    )
                    .await?,
            )?;
            if let Some(storm) = parse::typhoon(&value, p, now)? {
                result.push(storm);
            }
        }
        Ok(result)
    }
    async fn search(&self, region: Option<&Region>, cancel: &Cancellation) -> AppResult<Vec<Link>> {
        let region = region.ok_or_else(unavailable)?;
        let mut url = url::Url::parse("https://www.bing.com/search").map_err(|_| unavailable())?;
        url.query_pairs_mut().extend_pairs([
            ("format", "rss".to_string()),
            (
                "q",
                format!("{} (天气预报 OR 气象预警 OR 台风动态)", region.name),
            ),
        ]);
        parse::search(&self.text(url.as_str(), cancel).await?)
    }
}

fn cache_path(runtime: &Runtime) -> AppResult<std::path::PathBuf> {
    Ok(runtime
        .paths
        .as_ref()
        .map_err(Clone::clone)?
        .data_root
        .join("weather-information.json"))
}
fn save(runtime: &Runtime, value: &Dashboard) -> AppResult<()> {
    let path = cache_path(runtime)?;
    let temp = path.with_extension("tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec(value).map_err(|_| AppError::storage())?,
    )
    .map_err(|_| AppError::storage())?;
    std::fs::rename(temp, path).map_err(|_| AppError::storage())
}
pub(crate) fn load(runtime: &Runtime) -> AppResult<()> {
    let path = cache_path(runtime)?;
    if path.exists() {
        let bytes = std::fs::read(path).map_err(|_| AppError::storage())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(unavailable());
        }
        let mut data: Dashboard = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
        care::recover(&mut data);
        data.position.validate()?;
        data.links.retain(parse::is_weather_link);
        runtime.read(|s, _| {
            if s.weather.information.is_none() {
                s.weather.information = Some(data);
            }
            Ok(())
        })?;
    }
    Ok(())
}

pub(crate) async fn refresh(
    runtime: Runtime,
    revision: u64,
    events: Arc<dyn crate::events::PetEventSink>,
) -> AppResult<()> {
    let prepare = runtime.clone();
    let (mut value, lease, credentials) = tauri::async_runtime::spawn_blocking(move || {
        prepare.read(|s, _| {
            super::require_revision(s, revision)?;
            let position = s
                .weather
                .data
                .as_ref()
                .map(|d| d.position.clone())
                .ok_or_else(|| {
                    AppError::new("LOCATION_UNAVAILABLE", "尚未获得位置，请先刷新天气")
                })?;
            let context = crate::persona::context(&s.connection, s.context_revision)?;
            let lease = prepare.requests.start(
                "weather-information".into(),
                crate::error::parse_id(&context.persona_id)?,
            )?;
            let mut value = s
                .weather
                .information
                .clone()
                .filter(|d| parse::distance(&d.position, &position) < 10.)
                .unwrap_or(Dashboard {
                    position: position.clone(),
                    region: None,
                    forecast: None,
                    forecast_error: None,
                    warnings: Default::default(),
                    typhoons: Default::default(),
                    links: vec![],
                    search_error: None,
                });
            value.position = position;
            Ok((value, lease, super::fallback(&prepare, s)))
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    let now = runtime.clock.sample()?.utc;
    let client = Client::new()?;
    if value.region.is_none() {
        value.region = client.region(&value.position, &lease.cancel).await.ok();
    }
    if value
        .forecast
        .as_ref()
        .is_none_or(|f| now.signed_duration_since(f.fetched_at).num_seconds() >= 3600)
    {
        match client.forecast(&value.position, now, &lease.cancel).await {
            Ok(f) => {
                value.forecast = Some(f);
                value.forecast_error = None;
            }
            Err(e) => value.forecast_error = Some(e.message),
        }
    }
    let (mut warnings, typhoons, links) = tokio::join!(
        client.warnings(value.region.as_ref(), now, &lease.cancel),
        client.typhoons(&value.position, now, &lease.cancel),
        client.search(value.region.as_ref(), &lease.cancel)
    );
    if warnings.is_err() {
        if let Ok(Some(credentials)) = credentials {
            if let Ok(mut url) = url::Url::parse(&credentials.host) {
                if url.scheme() == "https"
                    && url.host_str().is_some()
                    && url.path() == "/"
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none()
                {
                    url.set_path(&format!(
                        "/weatheralert/v1/current/{:.2}/{:.2}",
                        value.position.latitude, value.position.longitude
                    ));
                    let fallback = async {
                        let mut response = client
                            .http
                            .get(url)
                            .header("X-QW-Api-Key", credentials.key)
                            .send()
                            .await
                            .map_err(|_| unavailable())?;
                        if !response.status().is_success() {
                            return Err(unavailable());
                        }
                        let mut bytes = vec![];
                        while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
                            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                                return Err(unavailable());
                            }
                            bytes.extend_from_slice(&chunk);
                        }
                        parse::qweather(
                            &serde_json::from_slice(&bytes).map_err(|_| unavailable())?,
                            now,
                        )
                    };
                    warnings = lease.cancel.wait(Duration::from_secs(10), fallback).await;
                }
            }
        }
    }
    if let Ok(items) = warnings.as_mut() {
        for warning in items.iter_mut().take(4) {
            if warning.url.starts_with("https://www.nmc.cn/publish/alarm/")
                && warning.description.starts_with("公开预警动态")
            {
                if let Some(old) = value.warnings.items.iter().find(|old| {
                    old.id == warning.id && !old.description.starts_with("公开预警动态")
                }) {
                    warning.description = old.description.clone();
                } else if let Ok(body) = client.text(&warning.url, &lease.cancel).await {
                    if let Some(content) = parse::warning_body(&body) {
                        warning.description = content;
                    }
                }
            }
        }
    }
    let warning_changed = update(&mut value.warnings, warnings, now, parse::warning_key);
    let typhoon_changed = update(&mut value.typhoons, typhoons, now, parse::typhoon_key);
    match links {
        Ok(links) => {
            value.links = links;
            value.search_error = None;
        }
        Err(e) => value.search_error = Some(e.message),
    }
    lease.cancel.check()?;
    let commit = runtime.clone();
    let mut copy = value.clone();
    tauri::async_runtime::spawn_blocking(move || {
        commit.read(|s, _| {
            super::require_revision(s, revision)?;
            lease.cancel.check()?;
            if s.weather
                .data
                .as_ref()
                .is_some_and(|d| !same_place(&copy, &d.position))
            {
                return Err(AppError::new(
                    "LOCATION_CHANGED",
                    "位置已变化，请重新获取气象资讯",
                ));
            }
            if let Some(current) = s.weather.information.as_ref() {
                retain_care(&mut copy.warnings, &current.warnings, parse::warning_key);
                retain_care(&mut copy.typhoons, &current.typhoons, parse::typhoon_key);
            }
            s.weather.information = Some(copy.clone());
            save(&commit, &copy)
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    // AI关怀独立于资料查询；先发布事实卡片，再生成一句话，不让模型失败覆盖天气。
    events.weather_changed();
    tauri::async_runtime::spawn(async move {
        let _ = care::generate(&runtime, revision, warning_changed, typhoon_changed, false, events.clone()).await;
        if runtime.jobs_cancel.check().is_ok() {
            events.weather_changed();
        }
    });
    Ok(())
}
fn update<T: Clone>(
    feed: &mut Feed<T>,
    result: AppResult<Vec<T>>,
    now: DateTime<Utc>,
    key: fn(&[T]) -> String,
) -> bool {
    match result {
        Ok(items) => {
            let changed = key(&feed.items) != key(&items);
            feed.items = items;
            feed.checked_at = Some(now);
            feed.error = None;
            if changed {
                feed.care = None;
                feed.care_status.clear();
            }
            changed
        }
        Err(error) => {
            feed.error = Some(error.message);
            false
        }
    }
}
fn retain_care<T: Clone>(next: &mut Feed<T>, current: &Feed<T>, key: fn(&[T]) -> String) {
    if key(&next.items) == key(&current.items) {
        next.care = current.care.clone().or_else(|| next.care.clone());
        next.attempted.extend(current.attempted.clone());
        next.care_status.extend(current.care_status.clone());
    }
}
pub(super) fn same_place(value: &Dashboard, position: &Position) -> bool {
    parse::distance(&value.position, position) < 10.
}

#[derive(Default)]
pub(crate) struct Worker {
    loaded: bool,
    last: Option<Instant>,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
}
impl Worker {
    pub(crate) fn tick(&mut self, runtime: &Runtime, events: Arc<dyn crate::events::PetEventSink>) {
        if runtime.paths.is_err()
            || runtime.jobs_cancel.check().is_err()
            || self.task.as_ref().is_some_and(|t| !t.inner().is_finished())
            || self
                .last
                .is_some_and(|t| t.elapsed() < Duration::from_secs(600))
        {
            return;
        }
        let runtime = runtime.clone();
        if !self.loaded {
            self.loaded = true;
            self.task = Some(tauri::async_runtime::spawn(async move {
                let init = runtime.clone();
                let _ = tauri::async_runtime::spawn_blocking(move || load(&init)).await;
                events.weather_changed();
            }));
            return;
        }
        let ready = runtime
            .read(|s, _| Ok(s.weather.data.is_some()))
            .unwrap_or(false);
        if !ready {
            return;
        }
        self.last = Some(Instant::now());
        self.task = Some(tauri::async_runtime::spawn(async move {
            if let Ok(revision) = runtime.read(|s, _| Ok(s.context_revision)) {
                let _ = refresh(runtime.clone(), revision, events.clone()).await;
            }
            if runtime.jobs_cancel.check().is_ok() {
                events.weather_changed();
            }
        }));
    }
}
