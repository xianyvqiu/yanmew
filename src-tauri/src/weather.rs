//! 天气是带时效的外部资料；失败不改变聊天、钱包或桌宠状态。
mod parse;
#[cfg(test)]
mod tests;
use crate::{
    error::{AppError, AppResult},
    location::Position,
    platform::contracts::LocationSource,
    runtime::Runtime,
    state::Cancellation,
};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum WeatherSource {
    OpenMeteo,
    Qweather,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub observed_at: DateTime<Utc>,
    pub description: String,
    pub temperature_celsius: Option<f64>,
    pub source: WeatherSource,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherData {
    pub position: Position,
    pub observation: Observation,
    pub fetched_at: DateTime<Utc>,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    Available,
    Cached,
    Stale,
    Unavailable,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherView {
    pub context_revision: u64,
    pub availability: Availability,
    pub data: Option<WeatherData>,
    pub error: Option<AppError>,
}
/// 运行期派生缓存不属于用户历史；重新启动后重新定位，配置仍存于 SQLite。
#[derive(Default)]
pub struct Cache {
    data: Option<WeatherData>,
    error: Option<AppError>,
}
impl Cache {
    pub fn view(&self, revision: u64, now: DateTime<Utc>) -> WeatherView {
        let availability = match &self.data {
            None => Availability::Unavailable,
            Some(data) if !is_fresh(data, now) => Availability::Stale,
            Some(_) if self.error.is_some() => Availability::Cached,
            Some(_) => Availability::Available,
        };
        WeatherView {
            context_revision: revision,
            availability,
            data: self.data.clone(),
            error: self.error.clone(),
        }
    }
    fn finish(&mut self, result: AppResult<WeatherData>) {
        match result {
            Ok(data) => {
                self.data = Some(data);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}
fn is_fresh(data: &WeatherData, now: DateTime<Utc>) -> bool {
    let fetched_age = now.signed_duration_since(data.fetched_at).num_seconds();
    let observed_age = now
        .signed_duration_since(data.observation.observed_at)
        .num_seconds();
    (0..=1800).contains(&fetched_age) && (0..=3600).contains(&observed_age)
}

// 凭据仅留在后端短生命周期请求快照中，不派生 Debug/Serialize。
struct Fallback {
    host: String,
    key: String,
}
fn fallback(runtime: &Runtime, store: &crate::runtime::Store) -> AppResult<Option<Fallback>> {
    let settings = crate::config::get_weather(&store.connection)?;
    let Some(host) = settings.api_host.filter(|_| settings.has_api_key) else {
        return Ok(None);
    };
    let encrypted: Option<String> = store
        .connection
        .query_row(
            "SELECT value FROM settings WHERE key='weather_qweather_api_key_encrypted'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(encrypted
        .map(|key| {
            runtime
                .secrets
                .unprotect(&key)
                .map(|key| Fallback { host, key })
        })
        .transpose()?)
}

struct Client {
    http: reqwest::Client,
    primary: String,
    #[cfg(test)]
    fallback_endpoint: Option<String>,
}
impl Client {
    fn new() -> AppResult<Self> {
        Ok(Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|_| AppError::new("WEATHER_UNAVAILABLE", "无法启动天气服务"))?,
            primary: "https://api.open-meteo.com/v1/forecast".into(),
            #[cfg(test)]
            fallback_endpoint: None,
        })
    }
    async fn json(
        &self,
        request: reqwest::RequestBuilder,
        cancel: &Cancellation,
    ) -> AppResult<serde_json::Value> {
        cancel
            .wait(Duration::from_secs(10), async {
                let mut response = request
                    .send()
                    .await
                    .map_err(|_| AppError::new("WEATHER_NETWORK", "天气服务连接失败或超时"))?;
                if !response.status().is_success() {
                    return Err(AppError::new(
                        "WEATHER_SERVICE_ERROR",
                        "天气服务暂时不可用，请检查天气配置或稍后重试",
                    ));
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response
                    .chunk()
                    .await
                    .map_err(|_| AppError::new("WEATHER_NETWORK", "天气数据接收失败"))?
                {
                    if bytes.len() + chunk.len() > 256 * 1024 {
                        return Err(AppError::new(
                            "WEATHER_INVALID_RESPONSE",
                            "天气服务返回的数据过大",
                        ));
                    }
                    bytes.extend_from_slice(&chunk);
                }
                serde_json::from_slice(&bytes).map_err(|_| {
                    AppError::new("WEATHER_INVALID_RESPONSE", "天气服务返回的数据无效")
                })
            })
            .await
            .map_err(|e| {
                if e.code == "AI_TIMEOUT" {
                    AppError::new("WEATHER_TIMEOUT", "天气请求超时")
                } else {
                    e
                }
            })
    }
    async fn fetch(
        &self,
        position: &Position,
        fallback: AppResult<Option<Fallback>>,
        cancel: &Cancellation,
    ) -> AppResult<Observation> {
        position.validate()?;
        let mut primary_url = url::Url::parse(&self.primary)
            .map_err(|_| AppError::new("WEATHER_CONFIGURATION", "天气主机配置无效"))?;
        primary_url.query_pairs_mut().extend_pairs([
            ("latitude", position.latitude.to_string()),
            ("longitude", position.longitude.to_string()),
            ("current", "temperature_2m,weather_code".into()),
            ("temperature_unit", "celsius".into()),
            ("timeformat", "unixtime".into()),
            ("timezone", "GMT".into()),
        ]);
        let request = self.http.get(primary_url);
        let primary = self
            .json(request, cancel)
            .await
            .and_then(|v| parse::open_meteo(&v));
        match primary {
            Ok(value) => return Ok(value),
            Err(e) if e.code == "CANCELLED" => return Err(e),
            _ => {}
        }
        cancel.check()?;
        let Some(fallback) = fallback? else {
            return primary;
        };
        let host = url::Url::parse(&fallback.host)
            .map_err(|_| AppError::new("WEATHER_CONFIGURATION", "和风天气主机配置无效"))?;
        if host.scheme() != "https"
            || host.host_str().is_none()
            || host.path() != "/"
            || !host.username().is_empty()
            || host.password().is_some()
            || host.query().is_some()
            || host.fragment().is_some()
        {
            return Err(AppError::new(
                "WEATHER_CONFIGURATION",
                "和风天气主机必须为不含路径和账号的 HTTPS 地址",
            ));
        }
        let mut url = host
            .join("v7/weather/now")
            .map_err(|_| AppError::new("WEATHER_CONFIGURATION", "和风天气主机配置无效"))?;
        #[cfg(test)]
        if let Some(endpoint) = &self.fallback_endpoint {
            url = url::Url::parse(endpoint).unwrap();
        }
        url.query_pairs_mut().extend_pairs([
            (
                "location",
                format!("{:.2},{:.2}", position.longitude, position.latitude),
            ),
            ("lang", "zh".into()),
        ]);
        let request = self.http.get(url).header("X-QW-Api-Key", &fallback.key);
        self.json(request, cancel)
            .await
            .and_then(|v| parse::qweather(&v))
    }
}

/// 显式刷新才访问定位/网络；聊天可直接读缓存，不等待本函数。
pub async fn refresh(
    runtime: Runtime,
    revision: u64,
    source: &dyn LocationSource,
) -> AppResult<WeatherView> {
    let prepare = runtime.clone();
    let (lease, credentials) = tauri::async_runtime::spawn_blocking(move || {
        prepare.read(|s, _| {
            require_revision(s, revision)?;
            let context = crate::persona::context(&s.connection, s.context_revision)?;
            let lease = prepare.requests.start(
                "weather".into(),
                crate::error::parse_id(&context.persona_id)?,
            )?;
            Ok((lease, fallback(&prepare, s)))
        })
    })
    .await
    .map_err(|_| AppError::storage())??;
    let result = async {
        let position = crate::location::locate(source, &lease.cancel).await?;
        let observation = Client::new()?
            .fetch(&position, credentials, &lease.cancel)
            .await?;
        Ok(WeatherData {
            position,
            observation,
            fetched_at: runtime.clock.sample()?.utc,
        })
    }
    .await;
    tauri::async_runtime::spawn_blocking(move || {
        runtime.read(|s, _| {
            require_revision(s, revision)?;
            lease.cancel.check()?;
            s.weather.finish(result);
            Ok(s.weather.view(revision, runtime.clock.sample()?.utc))
        })
    })
    .await
    .map_err(|_| AppError::storage())?
}
fn require_revision(store: &crate::runtime::Store, revision: u64) -> AppResult<()> {
    if store.context_revision != revision {
        return Err(AppError::new(
            "STALE_CONTEXT",
            "当前人设或会话已切换，请刷新后重试",
        ));
    }
    Ok(())
}
