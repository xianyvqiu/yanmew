use super::*;
use chrono::TimeZone;
use scraper::Html;
fn s(v: &Value) -> String {
    v.as_str().map(|s| s.trim().to_string()).unwrap_or_else(|| {
        if v.is_number() {
            v.to_string()
        } else {
            String::new()
        }
    })
}
fn number(v: &Value) -> Option<f64> {
    v.as_f64().filter(|v| v.is_finite())
}
pub(super) fn region(v: &Value) -> AppResult<Region> {
    let a = &v["address"];
    let pick = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| a[*k].as_str().filter(|s| !s.is_empty()))
            .unwrap_or("")
            .to_string()
    };
    let province = pick(&["state", "province"]);
    let city = pick(&["city", "town", "municipality", "county"]);
    let district = pick(&["city_district", "district", "county"]);
    let mut names = vec![];
    for name in [&province, &city, &district] {
        if !name.is_empty() && !names.contains(name) {
            names.push(name.clone());
        }
    }
    if names.is_empty() {
        return Err(unavailable());
    }
    Ok(Region {
        name: names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(" · "),
        province,
        city,
        district,
        country_code: s(&a["country_code"]),
    })
}
pub(super) fn forecast(v: &Value, now: DateTime<Utc>) -> AppResult<Forecast> {
    let h = &v["hourly"];
    let d = &v["daily"];
    let c = &v["current"];
    let times = h["time"].as_array().ok_or_else(unavailable)?;
    let dates = d["time"].as_array().ok_or_else(unavailable)?;
    if times.len() > 400 || dates.len() > 16 || times.is_empty() || dates.is_empty() {
        return Err(unavailable());
    }
    let offset = v["utc_offset_seconds"].as_i64().unwrap_or(0);
    let mut hourly = vec![];
    for (i, t) in times.iter().enumerate() {
        let time = t.as_i64().ok_or_else(unavailable)?;
        if time >= now.timestamp() - 1800 && hourly.len() < 24 {
            hourly.push(Hour {
                time,
                temperature: number(&h["temperature_2m"][i]),
                rain: number(&h["precipitation_probability"][i]),
                code: h["weather_code"][i].as_i64().unwrap_or(-1),
            });
        }
    }
    let mut daily = vec![];
    for (i, t) in dates.iter().take(7).enumerate() {
        let time = t.as_i64().ok_or_else(unavailable)?;
        daily.push(Day {
            date: Utc
                .timestamp_opt(time + offset, 0)
                .single()
                .ok_or_else(unavailable)?
                .format("%Y-%m-%d")
                .to_string(),
            low: number(&d["temperature_2m_min"][i]),
            high: number(&d["temperature_2m_max"][i]),
            rain: number(&d["precipitation_probability_max"][i]),
            uv: number(&d["uv_index_max"][i]),
            sunrise: d["sunrise"][i].as_i64(),
            sunset: d["sunset"][i].as_i64(),
            code: d["weather_code"][i].as_i64().unwrap_or(-1),
        });
    }
    if hourly.is_empty() || daily.first().is_none_or(|d|d.date!=(now+chrono::Duration::seconds(offset)).format("%Y-%m-%d").to_string()) {return Err(unavailable());}
    Ok(Forecast {
        fetched_at: now,
        feels_like: number(&c["apparent_temperature"]),
        humidity: number(&c["relative_humidity_2m"]),
        wind_speed: number(&c["wind_speed_10m"]),
        wind_direction: number(&c["wind_direction_10m"]),
        hourly,
        daily,
    })
}
fn level(title: &str) -> String {
    for (word, key) in [
        ("红色", "red"),
        ("橙色", "orange"),
        ("黄色", "yellow"),
        ("蓝色", "blue"),
    ] {
        if title.contains(word) {
            return key.into();
        }
    }
    "unknown".into()
}
fn local(title: &str, region: &Region) -> bool {
    // 省级信号影响范围较广，保留并标注来源；地方信号限定到本城市/区县。
    [&region.city, &region.district]
        .iter()
        .filter(|s| s.chars().count() >= 2)
        .any(|name| title.contains(name.as_str()))
        || (!region.province.is_empty()
            && title.starts_with(&format!(
                "{}气象台",
                region.province.trim_end_matches(['省', '市'])
            )))
        || title.starts_with(&format!("{}气象台", region.province))
}
pub(super) fn warnings(
    v: &Value,
    region: &Region,
    now: DateTime<Utc>,
) -> AppResult<(Vec<Warning>, u64)> {
    if v["code"].as_i64() != Some(0) {
        return Err(unavailable());
    }
    let page = &v["data"]["page"];
    let list = page["list"].as_array().ok_or_else(unavailable)?;
    let mut items = vec![];
    for row in list {
        let title = s(&row["title"]);
        let issued = s(&row["issuetime"]);
        let date = chrono::NaiveDateTime::parse_from_str(&issued, "%Y/%m/%d %H:%M")
            .map_err(|_| unavailable())?;
        let stamp = date.and_utc() - chrono::Duration::hours(8);
        if now.signed_duration_since(stamp).num_hours() > 24
            || stamp > now + chrono::Duration::minutes(10)
            || !local(&title, region)
            || title.contains("解除")
        {
            continue;
        }
        let path = s(&row["url"]);
        if !path.starts_with("/publish/alarm/") || path.contains("..") || path.contains(['?', '#'])
        {
            return Err(unavailable());
        }
        items.push(Warning {
            id: s(&row["alertid"]),
            level: level(&title),
            title,
            description: "公开预警动态；最新生效状态、影响范围与防御指南请查看官方原文。".into(),
            issued_at: issued,
            url: format!("https://www.nmc.cn{path}"),
            source: "中央气象台 · 公开预警信号".into(),
        });
    }
    Ok((
        items,
        page["totalPage"].as_u64().ok_or_else(unavailable)?.max(1),
    ))
}
pub(super) fn qweather(v: &Value, now: DateTime<Utc>) -> AppResult<Vec<Warning>> {
    let rows = v["alerts"].as_array().ok_or_else(unavailable)?;
    let mut result = vec![];
    for row in rows.iter().take(50) {
        let expires =
            DateTime::parse_from_rfc3339(row["expireTime"].as_str().ok_or_else(unavailable)?)
                .map_err(|_| unavailable())?;
        let effective =
            DateTime::parse_from_rfc3339(row["effectiveTime"].as_str().ok_or_else(unavailable)?)
                .map_err(|_| unavailable())?;
        if expires <= now || effective > now {
            continue;
        }
        let mut link = "https://www.nmc.cn/publish/alarm.html".to_string();
        if let Some(url) = row["web"]
            .as_str()
            .and_then(|u| url::Url::parse(u).ok())
            .filter(|u| {
                u.scheme() == "https"
                    && u.host_str().is_some()
                    && u.username().is_empty()
                    && u.password().is_none()
            })
        {
            link = url.to_string();
        }
        result.push(Warning {
            id: s(&row["id"]),
            title: s(&row["headline"]),
            description: s(&row["description"]),
            level: row["color"]["code"].as_str().unwrap_or("unknown").into(),
            issued_at: s(&row["issuedTime"]),
            url: link,
            source: format!("{} · 和风天气", s(&row["senderName"])),
        });
    }
    Ok(result)
}
pub(super) fn jsonp(body: &str) -> AppResult<Value> {
    let (prefix, body) = body.trim().split_once('(').ok_or_else(unavailable)?;
    if !prefix.starts_with("typhoon_jsons_")
        || !prefix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(unavailable());
    }
    serde_json::from_str(
        body.trim()
            .trim_start_matches('(')
            .trim_end_matches([';', ')']),
    )
    .map_err(|_| unavailable())
}
pub(super) fn distance(a: &Position, b: &Position) -> f64 {
    let dlat = (a.latitude - b.latitude).to_radians();
    let dlon = (a.longitude - b.longitude).to_radians();
    let n = (dlat / 2.).sin().powi(2)
        + a.latitude.to_radians().cos() * b.latitude.to_radians().cos() * (dlon / 2.).sin().powi(2);
    6371. * 2. * n.clamp(0., 1.).sqrt().asin()
}
pub(super) fn typhoon(
    v: &Value,
    position: &Position,
    now: DateTime<Utc>,
) -> AppResult<Option<Typhoon>> {
    let row = &v["typhoon"];
    let records = row[8].as_array().ok_or_else(unavailable)?;
    let last = records.last().ok_or_else(unavailable)?;
    let stamp = Utc
        .timestamp_millis_opt(last[2].as_i64().ok_or_else(unavailable)?)
        .single()
        .ok_or_else(unavailable)?;
    if now.signed_duration_since(stamp).num_hours() > 24
        || stamp > now + chrono::Duration::minutes(15)
    {
        return Err(unavailable());
    }
    let mut points: Vec<Point> = records
        .iter()
        .rev()
        .take(16)
        .rev()
        .map(|p| {
            Ok(Point {
                longitude: number(&p[4]).ok_or_else(unavailable)?,
                latitude: number(&p[5]).ok_or_else(unavailable)?,
                forecast: false,
            })
        })
        .collect::<AppResult<_>>()?;
    let forecasts = last[11]["BABJ"].as_array().ok_or_else(unavailable)?;
    let mut nearest = f64::MAX;
    for p in forecasts {
        let lon = number(&p[2]).ok_or_else(unavailable)?;
        let lat = number(&p[3]).ok_or_else(unavailable)?;
        let location = Position {
            longitude: lon,
            latitude: lat,
            located_at: now,
        };
        location.validate()?;
        nearest = nearest.min(distance(position, &location));
        points.push(Point {
            longitude: lon,
            latitude: lat,
            forecast: true,
        });
    }
    let current = points
        .iter()
        .rev()
        .find(|p| !p.forecast)
        .ok_or_else(unavailable)?;
    nearest = nearest.min(distance(
        position,
        &Position {
            latitude: current.latitude,
            longitude: current.longitude,
            located_at: now,
        },
    ));
    if nearest > 600. {
        return Ok(None);
    }
    Ok(Some(Typhoon {
        id: row[0].as_u64().ok_or_else(unavailable)?.to_string(),
        name: format!("{} · {}", s(&row[4]), s(&row[2])),
        intensity: s(&last[3]),
        published_at: stamp,
        wind_speed: number(&last[7]),
        distance_km: nearest,
        points,
        url: "https://typhoon.nmc.cn/".into(),
    }))
}
pub(super) fn is_weather_link(link: &Link) -> bool {
    let text = format!("{} {}", link.title, link.summary).to_lowercase();
    ["天气", "气象", "台风", "热带风暴", "热带气旋", "暴雨", "降雨", "雷电", "雷暴", "强对流", "大风", "寒潮", "高温", "降雪", "暴雪", "降温", "冰雹", "大雾", "霜冻", "沙尘", "洪涝", "防汛", "梅雨", "冷空气", "weather", "meteorolog", "typhoon", "cyclone", "thunderstorm"]
        .iter().any(|topic| text.contains(topic))
}
pub(super) fn search(body: &str) -> AppResult<Vec<Link>> {
    if !body.contains("<rss") || !body.contains("<channel>") {
        return Err(unavailable());
    }
    let text = |raw: &str, tag: &str| -> String {
        let start = format!("<{tag}>");
        let end = format!("</{tag}>");
        let value = raw
            .split(&start)
            .nth(1)
            .and_then(|s| s.split(&end).next())
            .unwrap_or("");
        Html::parse_fragment(value)
            .root_element()
            .text()
            .collect::<String>()
    };
    let mut links = vec![];
    for raw in body.split("<item>").skip(1).take(40) {
        let link = text(raw, "link");
        let Ok(url) = url::Url::parse(&link) else {
            continue;
        };
        if !matches!(url.scheme(), "https" | "http")
            || url.username() != ""
            || url.password().is_some()
        {
            continue;
        }
        let title = text(raw, "title");
        if !title.is_empty() {
            let result = Link {
                title: title.chars().take(160).collect(),
                url: url.to_string(),
                summary: text(raw, "description").chars().take(300).collect(),
            };
            if is_weather_link(&result) && !links.iter().any(|item: &Link| item.url == result.url) {
                links.push(result);
                if links.len() == 8 { break; }
            }
        }
    }
    Ok(links)
}
pub(super) fn warning_key(items: &[Warning]) -> String {
    let mut keys: Vec<_> = items
        .iter()
        .map(|w| {
            format!(
                "{}:{}",
                w.title
                    .replace("继续发布", "")
                    .replace("发布", "")
                    .replace("更新", "")
                    .replace("预警信号", "")
                    .chars()
                    .filter(|c| !c.is_ascii_digit() && !c.is_whitespace())
                    .collect::<String>(),
                w.level
            )
        })
        .collect();
    keys.sort();
    keys.join("|")
}

pub(super) fn warning_body(body: &str) -> Option<String> {
    let html = Html::parse_document(body);
    let selector = scraper::Selector::parse("#alarmtext").ok()?;
    let content = html
        .select(&selector)
        .map(|node| {
            node.text()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    (content.chars().count() > 15).then(|| content.chars().take(3000).collect())
}
pub(super) fn typhoon_key(items: &[Typhoon]) -> String {
    let mut keys: Vec<_> = items
        .iter()
        .map(|t| {
            format!(
                "{}:{}:{}",
                t.id,
                t.intensity,
                if t.distance_km < 200. {
                    "near"
                } else {
                    "watch"
                }
            )
        })
        .collect();
    keys.sort();
    keys.join("|")
}
