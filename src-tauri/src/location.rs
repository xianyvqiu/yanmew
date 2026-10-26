use crate::{
    error::{AppError, AppResult},
    platform::contracts::LocationSource,
    state::Cancellation,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
    pub located_at: DateTime<Utc>,
}
impl Position {
    pub fn validate(&self) -> AppResult<()> {
        if !self.latitude.is_finite()
            || !self.longitude.is_finite()
            || !(-90.0..=90.0).contains(&self.latitude)
            || !(-180.0..=180.0).contains(&self.longitude)
        {
            return Err(AppError::new("LOCATION_UNAVAILABLE", "系统未返回有效位置"));
        }
        Ok(())
    }
}

/// 授权及定位共用有界等待；平台实现负责在 future 被丢弃时取消原生操作。
pub async fn locate(source: &dyn LocationSource, cancel: &Cancellation) -> AppResult<Position> {
    locate_with_timeout(source, cancel, Duration::from_secs(30)).await
}
async fn locate_with_timeout(
    source: &dyn LocationSource,
    cancel: &Cancellation,
    timeout: Duration,
) -> AppResult<Position> {
    cancel.check()?;
    let position = cancel.wait(timeout, source.locate()).await.map_err(|e| {
        if e.code == "AI_TIMEOUT" {
            AppError::new("LOCATION_TIMEOUT", "定位超时，请检查系统位置服务后重试")
        } else {
            e
        }
    })?;
    position.validate()?;
    Ok(position)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Waiting;
    impl LocationSource for Waiting {
        fn locate(
            &self,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Position>> + Send + '_>>
        {
            Box::pin(std::future::pending())
        }
    }
    #[test]
    fn location_timeout_is_not_a_provider_error() {
        tauri::async_runtime::block_on(async {
            let error =
                locate_with_timeout(&Waiting, &Cancellation::default(), Duration::from_millis(5))
                    .await
                    .unwrap_err();
            assert_eq!(error.code, "LOCATION_TIMEOUT");
        });
    }
    #[test]
    fn coordinates_must_be_finite_and_in_range() {
        for (latitude, longitude) in [(f64::NAN, 10.0), (91.0, 10.0), (20.0, 181.0)] {
            assert!(Position {
                latitude,
                longitude,
                located_at: Utc::now()
            }
            .validate()
            .is_err());
        }
        // (0,0) 若为系统实际结果是合法坐标；不可在定位失败时伪造该坐标。
        assert!(Position {
            latitude: 0.0,
            longitude: 0.0,
            located_at: Utc::now()
        }
        .validate()
        .is_ok());
    }
}
