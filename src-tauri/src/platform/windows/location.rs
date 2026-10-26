use crate::{
    error::{AppError, AppResult},
    location::Position,
    platform::contracts::LocationSource,
};

pub struct WindowsLocation(pub tauri::WebviewWindow);

#[cfg(windows)]
fn native_error(error: windows::core::Error) -> AppError {
    if matches!(error.code().0 as u32, 0x800705B4 | 0x80070102) {
        AppError::new("LOCATION_TIMEOUT", "系统定位超时，请稍后重试")
    } else if error.code().0 == 0x80070005u32 as i32 {
        AppError::new(
            "LOCATION_DENIED",
            "定位权限被拒绝，请在 Windows 位置隐私设置中允许桌面应用访问位置",
        )
    } else {
        AppError::new(
            "LOCATION_UNAVAILABLE",
            "系统定位不可用，请检查 Windows 位置服务",
        )
    }
}

/// 超时或取消丢弃 future 时，同时终止 WinRT 操作。
#[cfg(windows)]
struct CancelOnDrop(Box<dyn Fn() + Send>);
#[cfg(windows)]
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        (self.0)();
    }
}

impl LocationSource for WindowsLocation {
    fn locate(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Position>> + Send + '_>> {
        Box::pin(async move {
            #[cfg(windows)]
            {
                use windows::Devices::Enumeration::{
                    DeviceAccessInformation, DeviceAccessStatus, DeviceClass,
                };
                use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator};
                use windows::Foundation::TimeSpan;
                // 只读查询不会弹出授权框。已有授权时不再受 RequestAccessAsync 的前台限制。
                let allowed = DeviceAccessInformation::CreateFromDeviceClass(DeviceClass::Location)
                    .and_then(|access| access.CurrentStatus())
                    .is_ok_and(|status| status == DeviceAccessStatus::Allowed);
                if !allowed {
                    let (send, receive) = tokio::sync::oneshot::channel();
                    let window = self.0.clone();
                    self.0
                        .run_on_main_thread(move || {
                            if send.is_closed() {
                                return;
                            }
                            let result = if window.is_focused().unwrap_or(false) {
                                Geolocator::RequestAccessAsync().map_err(native_error)
                            } else {
                                Err(AppError::new(
                                    "LOCATION_FOREGROUND_REQUIRED",
                                    "请在当前前台窗口中刷新定位",
                                ))
                            };
                            if let Err(Ok(operation)) = send.send(result) {
                                let _ = operation.Cancel();
                            }
                        })
                        .map_err(|_| AppError::new("LOCATION_UNAVAILABLE", "无法启动系统定位"))?;
                    let access = receive
                        .await
                        .map_err(|_| AppError::new("LOCATION_UNAVAILABLE", "定位窗口已关闭"))??;
                    let operation = access.clone();
                    let _access_guard = CancelOnDrop(Box::new(move || {
                        let _ = operation.Cancel();
                    }));
                    // windows 0.61 的异步句柄未启用 Future 适配；短间隔异步等待不占用 UI 线程。
                    while access.Status().map_err(native_error)?.0 == 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    }
                    let status = access.GetResults().map_err(native_error)?;
                    if status != GeolocationAccessStatus::Allowed {
                        if status != GeolocationAccessStatus::Denied {
                            return Err(AppError::new(
                                "LOCATION_UNAVAILABLE",
                                "系统未确认定位授权状态，请检查位置服务后重试",
                            ));
                        }
                        return Err(AppError::new(
                            "LOCATION_DENIED",
                            "未获得定位权限，请检查 Windows 位置隐私设置",
                        ));
                    }
                }
                let locator = Geolocator::new().map_err(native_error)?;
                let position = locator
                    .GetGeopositionAsyncWithAgeAndTimeout(
                        TimeSpan {
                            Duration: 60 * 10_000_000,
                        },
                        TimeSpan {
                            Duration: 20 * 10_000_000,
                        },
                    )
                    .map_err(native_error)?;
                let operation = position.clone();
                let _position_guard = CancelOnDrop(Box::new(move || {
                    let _ = operation.Cancel();
                }));
                while position.Status().map_err(native_error)?.0 == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                }
                let coordinate = position
                    .GetResults()
                    .map_err(native_error)?
                    .Coordinate()
                    .map_err(native_error)?;
                let seconds = coordinate.Timestamp().map_err(native_error)?.UniversalTime
                    / 10_000_000
                    - 11_644_473_600;
                let coordinates = coordinate
                    .Point()
                    .map_err(native_error)?
                    .Position()
                    .map_err(native_error)?;
                Ok(Position {
                    latitude: coordinates.Latitude,
                    longitude: coordinates.Longitude,
                    located_at: chrono::DateTime::from_timestamp(seconds, 0)
                        .ok_or_else(|| AppError::new("LOCATION_UNAVAILABLE", "定位时间无效"))?,
                })
            }
            #[cfg(not(windows))]
            {
                Err(AppError::new(
                    "LOCATION_UNAVAILABLE",
                    "当前平台尚未接入定位",
                ))
            }
        })
    }
}
