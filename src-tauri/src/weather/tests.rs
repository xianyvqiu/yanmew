use super::*;
use std::sync::atomic::AtomicUsize;
use crate::platform::contracts::LocationSource;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn position() -> Position {
    Position {
        latitude: 31.2,
        longitude: 121.5,
        located_at: Utc::now(),
    }
}
fn data(now: DateTime<Utc>) -> WeatherData {
    WeatherData {
        position: position(),
        observation: Observation {
            observed_at: now,
            description: "晴".into(),
            temperature_celsius: None,
            source: WeatherSource::OpenMeteo,
        },
        fetched_at: now,
    }
}
// 只接收一条本地请求，测试不依赖互联网或真实位置。
fn server(body: &str, status: u16) -> (String, std::thread::JoinHandle<String>) {
    server_bytes(body.as_bytes().to_vec(), status, "")
}
fn server_bytes(
    body: Vec<u8>,
    status: u16,
    encoding: &str,
) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let encoding = encoding.to_string();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = vec![0; 8192];
        let n = stream.read(&mut request).unwrap();
        write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\n{encoding}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
        stream.write_all(&body).unwrap();
        String::from_utf8_lossy(&request[..n]).into_owned()
    });
    (format!("http://{address}"), handle)
}

#[test]
fn primary_response_and_missing_temperature() {
    tauri::async_runtime::block_on(async {
        let (url, server) = server(r#"{"current":{"time":1725235200,"weather_code":0}}"#, 200);
        let mut client = Client::new().unwrap();
        client.primary = url;
        let result = client
            .fetch(
                &position(),
                Err(AppError::new("BROKEN_SECRET", "测试")),
                &Cancellation::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.source, WeatherSource::OpenMeteo);
        assert_eq!(result.temperature_celsius, None);
        let request = server.join().unwrap();
        assert!(request.contains("timeformat=unixtime"));
        assert!(request.contains("latitude=31.2"));
        assert!(!request.contains("Api-Key"));
    });
}

#[test]
fn primary_failure_without_credentials_and_invalid_host() {
    tauri::async_runtime::block_on(async {
        for fallback in [
            None,
            Some(Fallback {
                host: "http://unsafe.test".into(),
                key: "secret".into(),
            }),
        ] {
            let configured = fallback.is_some();
            let (url, server) = server("{}", 503);
            let mut client = Client::new().unwrap();
            client.primary = url;
            let result = client
                .fetch(&position(), Ok(fallback), &Cancellation::default())
                .await
                .unwrap_err();
            assert_eq!(
                result.code,
                if configured {
                    "WEATHER_CONFIGURATION"
                } else {
                    "WEATHER_SERVICE_ERROR"
                }
            );
            server.join().unwrap();
            assert!(!result.message.contains("secret"));
        }
    });
}

#[test]
fn cache_never_relabels_old_observation_as_current() {
    let now = Utc::now();
    let mut cache = Cache::default();
    assert_eq!(cache.view(0, now).availability, Availability::Unavailable);
    cache.finish(Ok(data(now)));
    assert_eq!(cache.view(0, now).availability, Availability::Available);
    cache.finish(Err(AppError::new("WEATHER_NETWORK", "失败")));
    assert_eq!(cache.view(0, now).availability, Availability::Cached);
    assert_eq!(
        cache
            .view(0, now + chrono::Duration::minutes(31))
            .availability,
        Availability::Stale
    );
    let mut old = data(now);
    old.observation.observed_at -= chrono::Duration::hours(2);
    cache.finish(Ok(old));
    assert_eq!(cache.view(0, now).availability, Availability::Stale);
    assert_eq!(
        cache
            .view(0, now - chrono::Duration::minutes(1))
            .availability,
        Availability::Stale
    );
}

#[test]
fn fallback_success_and_both_services_failed() {
    tauri::async_runtime::block_on(async {
        for status in [200, 503] {
            let (primary, first) = server("{}", 503);
            let (fallback_endpoint, second) = server(
                r#"{"code":"200","now":{"obsTime":"2026-09-28T10:25+08:00","text":"阴","temp":"19"}}"#,
                status,
            );
            let mut client = Client::new().unwrap();
            client.primary = primary;
            client.fallback_endpoint = Some(fallback_endpoint);
            let result = client
                .fetch(
                    &position(),
                    Ok(Some(Fallback {
                        host: "https://example.qweatherapi.com".into(),
                        key: "test-key".into(),
                    })),
                    &Cancellation::default(),
                )
                .await;
            if status == 200 {
                let observation = result.unwrap();
                assert_eq!(observation.source, WeatherSource::Qweather);
                assert_eq!(observation.temperature_celsius, Some(19.0));
            } else {
                assert_eq!(result.unwrap_err().code, "WEATHER_SERVICE_ERROR");
            }
            first.join().unwrap();
            let request = second.join().unwrap();
            assert!(request.contains("location=121.50%2C31.20"));
            assert!(request.to_lowercase().contains("x-qw-api-key: test-key"));
            assert!(!request.lines().next().unwrap().contains("test-key"));
        }
    });
}

#[test]
fn qweather_gzip_response_is_decompressed_before_parsing() {
    use base64::Engine;
    tauri::async_runtime::block_on(async {
        let bytes = base64::engine::general_purpose::STANDARD.decode("H4sIAAAAAAAACqtWSs5PSVWyUjIyMFDSUcrLL1eyqlbKTyoOycyFCBuZ6RpY6hpZhBgaWBmZahtYWIFVlqRWlChZKb2csQXMyS1QslIytFSqrQUA0DEXJlIAAAA=").unwrap();
        let (url, server) = server_bytes(bytes, 200, "Content-Encoding: gzip\r\n");
        let client = Client::new().unwrap();
        let response = client
            .json(client.http.get(url), &Cancellation::default())
            .await
            .unwrap();
        assert_eq!(
            parse::qweather(&response).unwrap().temperature_celsius,
            Some(19.0)
        );
        server.join().unwrap();
    });
}

struct Denied;
impl LocationSource for Denied {
    fn locate(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Position>> + Send + '_>> {
        Box::pin(async { Err(AppError::new("LOCATION_DENIED", "拒绝")) })
    }
}
#[test]
fn denied_location_preserves_business_state() {
    tauri::async_runtime::block_on(async {
        let runtime = crate::runtime::test_runtime("http://localhost");
        let view = refresh(runtime.clone(), 0, &Denied).await.unwrap();
        assert_eq!(view.availability, Availability::Unavailable);
        assert_eq!(view.error.unwrap().code, "LOCATION_DENIED");
        runtime
            .read(|s, _| {
                assert_eq!(
                    s.connection
                        .query_row("SELECT count(*) FROM chat_rounds", [], |r| r
                            .get::<_, i64>(0))?,
                    0
                );
                assert_eq!(
                    s.connection
                        .query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0))?,
                    0
                );
                Ok(())
            })
            .unwrap();
    });
}

struct Pending {
    started: Arc<AtomicBool>,
    dropped: Arc<AtomicBool>,
}
impl LocationSource for Pending {
    fn locate(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Position>> + Send + '_>> {
        Box::pin(async move {
            struct Mark(Arc<AtomicBool>);
            impl Drop for Mark {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::SeqCst);
                }
            }
            let _mark = Mark(self.dropped.clone());
            self.started.store(true, Ordering::SeqCst);
            std::future::pending().await
        })
    }
}
#[test]
fn cancelled_and_switched_requests_cannot_update_cache() {
    tauri::async_runtime::block_on(async {
        for switched in [false, true] {
            let runtime = crate::runtime::test_runtime("http://localhost");
            let started = Arc::new(AtomicBool::new(false));
            let dropped = Arc::new(AtomicBool::new(false));
            let source = Pending {
                started: started.clone(),
                dropped: dropped.clone(),
            };
            let task_runtime = runtime.clone();
            let task =
                tauri::async_runtime::spawn(async move { refresh(task_runtime, 0, &source).await });
            tokio::time::timeout(Duration::from_secs(2), async {
                while !started.load(Ordering::SeqCst) {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            runtime
                .read(|s, _| {
                    if switched {
                        s.context_revision += 1;
                    }
                    runtime.requests.cancel_key("weather");
                    Ok(())
                })
                .unwrap();
            let error = task.await.unwrap().unwrap_err();
            assert_eq!(
                error.code,
                if switched {
                    "STALE_CONTEXT"
                } else {
                    "CANCELLED"
                }
            );
            assert!(dropped.load(Ordering::SeqCst));
            runtime
                .read(|s, _| {
                    assert!(s.weather.data.is_none());
                    assert!(s.weather.error.is_none());
                    Ok(())
                })
                .unwrap();
        }
    });
}


struct DeniedSource(AtomicUsize);
impl LocationSource for DeniedSource {
    fn locate(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = AppResult<Position>> + Send + '_>> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(AppError::new("LOCATION_DENIED", "定位权限未开启"))
        })
    }
}
#[derive(Default)]
struct WeatherEvents(AtomicUsize);
impl crate::events::PetEventSink for WeatherEvents {
    fn changed(&self, _: &crate::pet::controller::View) {}
    fn failed(&self, _: &AppError) {}
    fn weather_changed(&self) { self.0.fetch_add(1, Ordering::SeqCst); }
}
#[test]
fn background_refreshes_on_start_and_hourly_preserving_cache_on_failure() {
    tauri::async_runtime::block_on(async {
        let runtime = crate::runtime::test_runtime("http://localhost");
        let now = runtime.clock.sample().unwrap().utc;
        runtime.read(|store, _| { store.weather.finish(Ok(data(now))); Ok(()) }).unwrap();
        let source = Arc::new(DeniedSource(AtomicUsize::new(0)));
        let events = Arc::new(WeatherEvents::default());
        let mut worker = Worker::default();
        worker.tick(&runtime, source.clone(), events.clone());
        worker.task.take().unwrap().await.unwrap();
        assert_eq!(source.0.load(Ordering::SeqCst), 1);
        assert_eq!(events.0.load(Ordering::SeqCst), 1);
        runtime.read(|store, _| {
            let view = store.weather.view(0, now);
            assert!(view.data.is_some());
            assert_eq!(view.error.unwrap().code, "LOCATION_DENIED");
            Ok(())
        }).unwrap();
        worker.tick(&runtime, source.clone(), events.clone());
        assert!(worker.task.is_none());
        worker.last_started = Some(std::time::Instant::now() - REFRESH_INTERVAL);
        worker.tick(&runtime, source.clone(), events.clone());
        worker.task.take().unwrap().await.unwrap();
        assert_eq!(source.0.load(Ordering::SeqCst), 2);
        assert_eq!(events.0.load(Ordering::SeqCst), 2);
        runtime.jobs_cancel.cancel();
        worker.last_started = None;
        worker.tick(&runtime, source, events);
        assert!(worker.task.is_none());
    });
}
#[test]
fn background_does_not_overlap_an_inflight_refresh() {
    tauri::async_runtime::block_on(async {
        let runtime = crate::runtime::test_runtime("http://localhost");
        let source = Arc::new(Pending { started: Arc::new(AtomicBool::new(false)), dropped: Arc::new(AtomicBool::new(false)) });
        let events = Arc::new(WeatherEvents::default());
        let mut worker = Worker::default();
        worker.tick(&runtime, source.clone(), events.clone());
        tokio::time::timeout(Duration::from_secs(2), async {
            while !source.started.load(Ordering::SeqCst) { tokio::task::yield_now().await; }
        }).await.unwrap();
        let id = worker.task.as_ref().unwrap().inner().id();
        worker.last_started = Some(std::time::Instant::now() - REFRESH_INTERVAL);
        worker.tick(&runtime, source.clone(), events);
        assert_eq!(worker.task.as_ref().unwrap().inner().id(), id);
        runtime.requests.cancel_key("weather");
        worker.task.take().unwrap().await.unwrap();
        assert!(source.dropped.load(Ordering::SeqCst));
    });
}

#[test]
fn disconnected_weather_request_retries_once_then_recovers() {
    tauri::async_runtime::block_on(async {
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
        let handle=std::thread::spawn(move||{
            let (first,_)=listener.accept().unwrap();drop(first);
            let (mut second,_)=listener.accept().unwrap();let mut request=[0;4096];second.read(&mut request).unwrap();
            let body=r#"{"current":{"time":1725235200,"weather_code":0,"temperature_2m":20}}"#;
            write!(second,"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        });
        let client=Client::new().unwrap();let result=client.json(client.http.get(format!("http://{address}")),&Cancellation::default()).await.unwrap();
        assert_eq!(result["current"]["temperature_2m"],20);handle.join().unwrap();
    });
}
#[test]
fn network_failure_uses_one_minute_retry_and_success_returns_to_hourly() {
    let worker=Worker::default();assert_eq!(worker.interval(),REFRESH_INTERVAL);
    worker.retry_network.store(true,Ordering::Relaxed);assert_eq!(worker.interval(),Duration::from_secs(60));
    worker.retry_network.store(false,Ordering::Relaxed);assert_eq!(worker.interval(),REFRESH_INTERVAL);
}
#[test]
#[ignore = "手动检查公开免费天气接口；默认测试不访问互联网"]
fn manual_public_weather_connection() {
    tauri::async_runtime::block_on(async {let result=Client::new().unwrap().fetch(&position(),Ok(None),&Cancellation::default()).await;assert!(result.is_ok(),"{result:?}");});
}

#[test]
fn manual_refresh_network_failure_also_schedules_background_recovery() {
    tauri::async_runtime::block_on(async {
        let runtime=crate::runtime::test_runtime("http://localhost");runtime.read(|s,_|{s.weather.finish(Err(AppError::new("WEATHER_NETWORK","模拟断线")));Ok(())}).unwrap();
        let source=Arc::new(DeniedSource(AtomicUsize::new(0)));let events=Arc::new(WeatherEvents::default());let mut worker=Worker::default();
        worker.last_started=Some(std::time::Instant::now()-Duration::from_secs(30));worker.tick(&runtime,source.clone(),events.clone());assert!(worker.task.is_none());
        worker.last_started=Some(std::time::Instant::now()-Duration::from_secs(61));worker.tick(&runtime,source.clone(),events);worker.task.take().unwrap().await.unwrap();assert_eq!(source.0.load(Ordering::SeqCst),1);
    });
}
