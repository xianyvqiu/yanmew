use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::{Observation, WeatherSource};

fn invalid_response() -> AppError {
    AppError::new("WEATHER_INVALID_RESPONSE", "天气服务返回的数据不完整或无效")
}

// Open-Meteo 的 weather_code 使用其文档列出的 WMO 天气解释代码。
fn wmo_description(code: i64) -> Option<&'static str> {
    Some(match code {
        0 => "晴",
        1 => "大部晴朗",
        2 => "局部多云",
        3 => "阴",
        45 => "雾",
        48 => "雾凇",
        51 => "小毛毛雨",
        53 => "中等毛毛雨",
        55 => "浓毛毛雨",
        56 => "轻微冻毛毛雨",
        57 => "浓冻毛毛雨",
        61 => "小雨",
        63 => "中雨",
        65 => "大雨",
        66 => "小冻雨",
        67 => "大冻雨",
        71 => "小雪",
        73 => "中雪",
        75 => "大雪",
        77 => "米雪",
        80 => "小阵雨",
        81 => "中阵雨",
        82 => "强阵雨",
        85 => "小阵雪",
        86 => "大阵雪",
        95 => "雷暴",
        96 => "雷暴伴小冰雹",
        99 => "雷暴伴大冰雹",
        _ => return None,
    })
}

pub(super) fn open_meteo(value: &Value) -> AppResult<Observation> {
    if value.get("error").and_then(Value::as_bool) == Some(true) {
        return Err(invalid_response());
    }
    let current = value.get("current").ok_or_else(invalid_response)?;
    let timestamp = current
        .get("time")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_response)?;
    let observed_at = DateTime::<Utc>::from_timestamp(timestamp, 0).ok_or_else(invalid_response)?;
    let weather_code = current
        .get("weather_code")
        .and_then(Value::as_i64)
        .ok_or_else(invalid_response)?;
    let description = wmo_description(weather_code).ok_or_else(invalid_response)?;
    let temperature_celsius = match current.get("temperature_2m") {
        None | Some(Value::Null) => None,
        Some(temperature) => {
            let number = temperature.as_f64().filter(|number| number.is_finite());
            Some(number.ok_or_else(invalid_response)?)
        }
    };

    Ok(Observation {
        observed_at,
        description: description.to_owned(),
        temperature_celsius,
        source: WeatherSource::OpenMeteo,
    })
}

pub(super) fn qweather(value: &Value) -> AppResult<Observation> {
    if value.get("code").and_then(Value::as_str) != Some("200") {
        return Err(invalid_response());
    }
    let now = value.get("now").ok_or_else(invalid_response)?;
    let observed_at = now
        .get("obsTime")
        .and_then(Value::as_str)
        .and_then(|time| {
            DateTime::parse_from_rfc3339(time)
                .or_else(|_| DateTime::parse_from_str(time, "%Y-%m-%dT%H:%M%:z"))
                .ok()
        })
        .map(|time| time.with_timezone(&Utc))
        .ok_or_else(invalid_response)?;
    // 和风直接提供天气文字；其图标代码与 WMO 代码互不通用。
    let description = now
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(invalid_response)?;
    let temperature_celsius = match now.get("temp") {
        None | Some(Value::Null) => None,
        Some(Value::String(temperature)) if temperature.trim().is_empty() => None,
        Some(Value::String(temperature)) => {
            let number = temperature
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite());
            Some(number.ok_or_else(invalid_response)?)
        }
        Some(_) => return Err(invalid_response()),
    };

    Ok(Observation {
        observed_at,
        description: description.to_owned(),
        temperature_celsius,
        source: WeatherSource::Qweather,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::{open_meteo, qweather, wmo_description};

    #[test]
    fn open_meteo_parses_valid_observation() {
        let observation = open_meteo(&json!({
            "current": {"time": 1_725_235_200, "weather_code": 63, "temperature_2m": 23.5}
        }))
        .unwrap();

        assert_eq!(
            observation.observed_at,
            Utc.timestamp_opt(1_725_235_200, 0).unwrap()
        );
        assert_eq!(observation.description, "中雨");
        assert_eq!(observation.temperature_celsius, Some(23.5));
        assert!(matches!(
            observation.source,
            super::WeatherSource::OpenMeteo
        ));
    }

    #[test]
    fn open_meteo_accepts_missing_temperature() {
        let observation = open_meteo(&json!({
            "current": {"time": 1_725_235_200, "weather_code": 0}
        }))
        .unwrap();
        assert_eq!(observation.temperature_celsius, None);

        let observation = open_meteo(&json!({
            "current": {"time": 1_725_235_200, "weather_code": 0, "temperature_2m": null}
        }))
        .unwrap();
        assert_eq!(observation.temperature_celsius, None);
    }

    #[test]
    fn open_meteo_rejects_invalid_fields_and_unknown_code() {
        for value in [
            json!({"current": {"time": "2024-09-02", "weather_code": 0}}),
            json!({"current": {"time": 9_999_999_999_999_999_i64, "weather_code": 0}}),
            json!({"current": {"time": 1_725_235_200, "weather_code": 100}}),
            json!({"current": {"time": 1_725_235_200, "weather_code": 0, "temperature_2m": "23"}}),
            json!({"error": true, "reason": "upstream failure"}),
        ] {
            assert_eq!(
                open_meteo(&value).err().unwrap().code,
                "WEATHER_INVALID_RESPONSE"
            );
        }
    }

    #[test]
    fn wmo_mapping_covers_documented_codes() {
        let documented = [
            0, 1, 2, 3, 45, 48, 51, 53, 55, 56, 57, 61, 63, 65, 66, 67, 71, 73, 75, 77, 80, 81, 82,
            85, 86, 95, 96, 99,
        ];
        for code in documented {
            assert!(wmo_description(code).is_some(), "未映射天气码 {code}");
        }
        assert_eq!(wmo_description(100), None);
    }

    #[test]
    fn qweather_parses_valid_observation() {
        let observation = qweather(&json!({
            "code": "200",
            "now": {"obsTime": "2024-09-02T08:00:00+08:00", "text": "多云", "temp": "24.5"}
        }))
        .unwrap();

        assert_eq!(
            observation.observed_at,
            Utc.timestamp_opt(1_725_235_200, 0).unwrap()
        );
        assert_eq!(observation.description, "多云");
        assert_eq!(observation.temperature_celsius, Some(24.5));
        assert!(matches!(observation.source, super::WeatherSource::Qweather));
    }

    #[test]
    fn qweather_accepts_official_minute_precision_time() {
        let observation = qweather(&json!({
            "code": "200",
            "now": {"obsTime": "2023-04-12T18:22+08:00", "text": "多云", "temp": "24"}
        }))
        .unwrap();

        assert_eq!(
            observation.observed_at,
            Utc.with_ymd_and_hms(2023, 4, 12, 10, 22, 0).unwrap()
        );
    }

    #[test]
    fn qweather_accepts_missing_or_empty_temperature() {
        for temperature in [None, Some(json!(null)), Some(json!(" "))] {
            let mut response = json!({
                "code": "200",
                "now": {"obsTime": "2024-09-02T08:00:00+08:00", "text": "晴"}
            });
            if let Some(temperature) = temperature {
                response["now"]["temp"] = temperature;
            }
            assert_eq!(qweather(&response).unwrap().temperature_celsius, None);
        }
    }

    #[test]
    fn qweather_rejects_invalid_fields_and_upstream_error() {
        for value in [
            json!({"code": "401", "now": {"obsTime": "2024-09-02T08:00:00+08:00", "text": "晴"}}),
            json!({"code": "200", "now": {"obsTime": "invalid", "text": "晴"}}),
            json!({"code": "200", "now": {"obsTime": "2024-09-02T08:00:00+08:00", "text": " "}}),
            json!({"code": "200", "now": {"obsTime": "2024-09-02T08:00:00+08:00", "text": "晴", "temp": 24}}),
            json!({"code": "200", "now": {"obsTime": "2024-09-02T08:00:00+08:00", "text": "晴", "temp": "NaN"}}),
        ] {
            assert_eq!(
                qweather(&value).err().unwrap().code,
                "WEATHER_INVALID_RESPONSE"
            );
        }
    }
}
