use super::*;
use serde_json::json;
fn position() -> Position {
    Position {
        latitude: 31.23,
        longitude: 121.47,
        located_at: Utc::now(),
    }
}
fn region() -> Region {
    Region {
        name: "上海市".into(),
        province: "上海市".into(),
        city: "上海市".into(),
        district: "黄浦区".into(),
        country_code: "cn".into(),
    }
}
fn warning(id: &str, level: &str) -> Warning {
    Warning {
        id: id.into(),
        title: "上海市气象台暴雨预警".into(),
        level: level.into(),
        description: "防范".into(),
        issued_at: "now".into(),
        url: "https://www.nmc.cn/publish/alarm.html".into(),
        source: "气象台".into(),
    }
}
#[test]
fn region_and_forecast_keep_real_nulls_and_distinct_administrative_names() {
    let r=parse::region(&json!({"address":{"state":"上海市","city":"上海市","district":"黄浦区","country_code":"cn"}})).unwrap();
    assert_eq!(r.name, "上海市 · 黄浦区");
    let now = Utc::now();
    let epoch = now.timestamp();
    let f=parse::forecast(&json!({"current":{"apparent_temperature":null,"relative_humidity_2m":53},"hourly":{"time":[epoch,epoch+3600],"temperature_2m":[null,20],"weather_code":[3,0],"precipitation_probability":[null,60]},"daily":{"time":[epoch],"temperature_2m_min":[17],"temperature_2m_max":[22],"weather_code":[3]}}),now).unwrap();
    assert_eq!(f.feels_like, None);
    assert_eq!(f.hourly[0].temperature, None);
    assert_eq!(f.daily[0].rain, None);
    assert_eq!(f.hourly[1].temperature, Some(20.));
    assert!(parse::forecast(&json!({"hourly":{"time":[]},"daily":{"time":[]}}), now).is_err());
}
#[test]
fn public_warnings_filter_old_and_other_cities_and_reject_incomplete_pages() {
    let now = Utc::now();
    let date = (now + chrono::Duration::hours(8))
        .format("%Y/%m/%d %H:%M")
        .to_string();
    let row = |title: &str, time: &str| json!({"title":title,"alertid":"123_20261022","issuetime":time,"url":"/publish/alarm/123.html"});
    let data = json!({"code":0,"data":{"page":{"totalPage":2,"list":[row("上海市气象台发布暴雨橙色预警信号",&date),row("北京市气象台发布暴雨黄色预警信号",&date),row("上海市气象台发布暴雨蓝色预警信号","2020/01/01 00:00"),row("上海市气象台解除暴雨预警",&date)]}}});
    let (items, pages) = parse::warnings(&data, &region(), now).unwrap();
    assert_eq!(pages, 2);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].level, "orange");
    assert!(items[0].url.starts_with("https://www.nmc.cn/"));
    assert!(parse::warnings(
        &json!({"code":0,"data":{"page":{"list":[]}}}),
        &region(),
        now
    )
    .is_err());
}
#[test]
fn public_warnings_merge_province_feed_without_duplicates_or_other_districts() {
    let now = Utc::now();
    let time = (now + chrono::Duration::hours(8)).format("%Y/%m/%d %H:%M").to_string();
    let row = |id: &str, title: &str| json!({"title":title,"alertid":id,"issuetime":time,"url":format!("/publish/alarm/{id}.html")});
    let city = row("city", "上海市气象台发布大风蓝色预警信号");
    let data = json!({"code":0,"data":{"page":{"totalPage":1,"list":[city.clone(),row("baoshan","上海市宝山区气象台发布大风黄色预警信号")]},"provinceAlarms":[city,row("other","江苏省气象台发布大风蓝色预警信号"),row("ended","上海市气象台解除大风预警信号")]}});
    let (items, _) = parse::warnings(&data, &region(), now).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "city");
    let mut only_province = data.clone();
    only_province["data"]["page"]["list"] = json!([]);
    assert_eq!(parse::warnings(&only_province, &region(), now).unwrap().0.len(), 1);
    only_province["data"]["provinceAlarms"] = json!({"changed":"schema"});
    assert!(parse::warnings(&only_province, &region(), now).is_err());
    let mut jiangsu = region(); jiangsu.province = "江苏省".into(); jiangsu.city = "南京市".into(); jiangsu.district = "玄武区".into();
    let joint = json!({"code":0,"data":{"page":{"totalPage":1,"list":[]},"provinceAlarms":[row("joint","江苏省应急管理厅和江苏省气象台发布大风蓝色预警信号")]}});
    assert_eq!(parse::warnings(&joint, &jiangsu, now).unwrap().0.len(), 1);
}
#[test]
fn municipality_region_moves_district_out_of_city_and_keeps_parent() {
    for address in [json!({"state":"上海市","city":"徐汇区","country_code":"cn"}),json!({"state":"上海市","county":"徐汇区","country_code":"cn"})] {
        let found = parse::region(&json!({"address":address})).unwrap();
        assert_eq!(found.city,"上海市"); assert_eq!(found.district,"徐汇区"); assert_eq!(found.name,"上海市 · 徐汇区");
    }
}
#[test]
fn shanghai_index_keeps_central_signal_without_treating_baoshan_as_xuhui() {
    let body = "alarm_index([{\"w1\":\"上海市\",\"w2\":\"上海市\",\"w3\":\"宝山区\",\"w5\":\"大风\",\"w7\":\"黄色\"},{\"w1\":\"上海市中心城区\",\"w2\":\"上海市\",\"w3\":\"上海市\",\"w5\":\"大风\",\"w7\":\"蓝色\"}]);";
    let mut r = region(); r.district = "徐汇区".into();
    let items = parse::shanghai_warnings(body, &r).unwrap();
    assert_eq!(items.len(),1); assert_eq!(items[0].level,"blue"); assert!(items[0].title.contains("中心城区"));
    assert_eq!(items[0].issued_at,"来源未提供"); assert!(items[0].description.contains("解除时间"));
    let mut legacy = r.clone(); legacy.city = "徐汇区".into(); legacy.district.clear();
    assert_eq!(parse::shanghai_warnings(body, &legacy).unwrap()[0].id,items[0].id);
    r.district = "宝山区".into(); let baoshan = parse::shanghai_warnings(body,&r).unwrap();
    assert_eq!(baoshan.len(),1); assert_eq!(baoshan[0].level,"yellow");
    r.district = "浦东新区".into(); assert!(parse::shanghai_warnings(body,&r).unwrap().is_empty());
    let mut nmc = items[0].clone(); nmc.id = "official-id".into(); nmc.title = "上海市气象台发布中心城区大风蓝色预警信号".into();
    assert!(parse::same_warning(&nmc,&items[0])); assert!(!parse::same_warning(&nmc,&baoshan[0]));
}
#[test]
fn shanghai_index_rejects_scripts_and_unknown_schema_instead_of_reporting_no_warnings() {
    for body in ["alarm_index([]);alert(1)","other_callback([])","alarm_index({})","alarm_index([{}])","alarm_index([{\"w1\":\"上海市\",\"w2\":\"上海市\",\"w3\":\"上海市\",\"w5\":\"大风\",\"w7\":\"未知\"}])"] {
        assert!(parse::shanghai_warnings(body,&region()).is_err());
    }
    assert!(parse::shanghai_warnings("alarm_index([])",&region()).unwrap().is_empty());
    let mut other = region(); other.province = "江苏省".into();
    assert!(parse::shanghai_warnings("alarm_index([])",&other).is_err());
}
#[test]
#[ignore = "手动核对公开免费预警来源，不使用个人位置或模型Key"]
fn shanghai_public_warning_sources_read_only_smoke() {
    tauri::async_runtime::block_on(async {
        let client=Client::new().unwrap(); let mut r=region(); r.city="徐汇区".into(); r.district.clear();
        let warnings=client.warnings(Some(&r),Utc::now(),&Cancellation::default()).await.unwrap();
        let sample=serde_json::to_string_pretty(&warnings).unwrap();
        std::fs::write(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.test-temp/207-weather-warnings/rust-public-warnings.json"),sample).unwrap();
        println!("public warning sources verified: {} results for fixed Xuhui region",warnings.len());
    });
}
fn track(lon: f64, lat: f64, time: i64) -> Value {
    json!({"typhoon":[123,"TEST","测试",2630,2630,null,null,"start",[[1,"time",time,"TY",126.,20.,950,40.,"NW",16,[],{"BABJ":[[24,"time",lon,lat,950,40,"BABJ","TY"]]}]]]})
}
#[test]
fn typhoon_uses_official_forecast_points_filters_distance_and_rejects_stale() {
    let now = Utc::now();
    let storm = parse::typhoon(
        &track(121.5, 31.5, now.timestamp_millis()),
        &position(),
        now,
    )
    .unwrap()
    .unwrap();
    assert!(storm.distance_km < 50.);
    assert!(storm.points.last().unwrap().forecast);
    assert_eq!(storm.name, "2630 · 测试");
    assert!(
        parse::typhoon(&track(145., 10., now.timestamp_millis()), &position(), now)
            .unwrap()
            .is_none()
    );
    assert!(parse::typhoon(
        &track(
            121.5,
            31.5,
            (now - chrono::Duration::hours(25)).timestamp_millis()
        ),
        &position(),
        now
    )
    .is_err());
}
#[test]
fn jsonp_and_search_are_parsed_as_data_without_executing_scripts() {
    assert_eq!(
        parse::jsonp("typhoon_jsons_list_default(({\"typhoonList\":[]}));").unwrap()["typhoonList"],
        json!([])
    );
    assert!(parse::jsonp("alert(1)").is_err());
    let links=parse::search("<rss><channel><item><title>台风 &amp; 动态</title><link>https://www.nmc.cn/?a=1&amp;b=2</link><description>官方动态</description></item><item><title>错误链接</title><link>javascript:alert(1)</link></item></channel></rss>").unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].title, "台风 & 动态");
    assert_eq!(links[0].url, "https://www.nmc.cn/?a=1&b=2");
    assert!(parse::search("<html>请登录</html>").is_err());
}
#[test]
fn weather_search_rejects_unrelated_results_and_keeps_weather_reports() {
    let item = |title: &str, url: &str, summary: &str| format!("<item><title>{title}</title><link>{url}</link><description>{summary}</description></item>");
    let mut body = String::from("<rss><channel>");
    for i in 0..9 { body.push_str(&item("本地股票风险预警", &format!("https://example.com/stock/{i}"), "投资资讯")); }
    body.push_str(&item("上海明日天气预报", "https://example.com/weather", "冷空气到来，出行带伞"));
    body.push_str(&item("上海明日天气预报重复项", "https://example.com/weather", "天气动态"));
    body.push_str(&item("雷电黄色预警", "https://example.com/storm", "气象台提醒注意防护"));
    body.push_str(&item("Typhoon forecast", "https://example.com/typhoon", "Track update"));
    body.push_str("</channel></rss>");
    let links = parse::search(&body).unwrap();
    assert_eq!(links.len(), 3);
    assert_eq!(links[0].title, "上海明日天气预报");
    assert_eq!(links[1].title, "雷电黄色预警");
    assert_eq!(links[2].title, "Typhoon forecast");
    assert!(parse::search("<rss><channel><item><title>今日招聘</title><link>https://example.com/jobs</link></item></channel></rss>").unwrap().is_empty());
}
#[test]
fn unchanged_updates_do_not_regenerate_care_errors_keep_old_results() {
    let now = Utc::now();
    let mut feed = Feed::<Warning>::default();
    assert!(update(
        &mut feed,
        Ok(vec![warning("123_old", "yellow")]),
        now,
        parse::warning_key
    ));
    feed.care = Some(Care {
        persona_id: "1".into(),
        name: "小猫".into(),
        text: "记得带伞".into(),
    });
    assert!(!update(
        &mut feed,
        Ok(vec![warning("123_new", "yellow")]),
        now,
        parse::warning_key
    ));
    assert!(feed.care.is_some());
    assert!(!update(
        &mut feed,
        Err(unavailable()),
        now,
        parse::warning_key
    ));
    assert_eq!(feed.items.len(), 1);
    assert!(feed.care.is_some());
    assert!(update(
        &mut feed,
        Ok(vec![warning("123_new", "orange")]),
        now,
        parse::warning_key
    ));
    assert!(feed.care.is_none());
    assert!(update(&mut feed, Ok(vec![]), now, parse::warning_key));
    assert!(!update(&mut feed, Ok(vec![]), now, parse::warning_key));
}
#[test]
fn typhoon_minor_movement_does_not_change_care_key_but_intensity_or_nearby_does() {
    let mut a = parse::typhoon(
        &track(123., 30., Utc::now().timestamp_millis()),
        &position(),
        Utc::now(),
    )
    .unwrap()
    .unwrap();
    a.distance_km = 350.;
    let key = parse::typhoon_key(&[a.clone()]);
    a.distance_km += 10.;
    assert_eq!(key, parse::typhoon_key(&[a.clone()]));
    a.intensity = "SuperTY".into();
    assert_ne!(key, parse::typhoon_key(&[a]));
}
#[test]
fn qweather_filters_expired_and_not_yet_effective_alerts() {
    let now = Utc::now();
    let row = |start: DateTime<Utc>, end: DateTime<Utc>| json!({"id":"1","headline":"暴雨预警","description":"防御指南","effectiveTime":start.to_rfc3339(),"expireTime":end.to_rfc3339(),"color":{"code":"red"}});
    let active = row(
        now - chrono::Duration::hours(1),
        now + chrono::Duration::hours(2),
    );
    assert_eq!(parse::qweather(&json!({"alerts":[active,row(now-chrono::Duration::hours(2),now-chrono::Duration::hours(1)),row(now+chrono::Duration::hours(1),now+chrono::Duration::hours(2))]}),now).unwrap().len(),1);
    assert!(parse::qweather(&json!({"error":"permission"}), now).is_err());
}

#[test]
#[ignore = "仅手动核对公开免费来源，不调用模型或使用个人定位"]
fn public_sources_read_only_smoke() {
    tauri::async_runtime::block_on(async {
        let client = Client::new().unwrap();
        let cancel = Cancellation::default();
        let p = position();
        let now = Utc::now();
        let region = client.region(&p, &cancel).await.unwrap();
        let (forecast, warnings, typhoons, links) = tokio::join!(
            client.forecast(&p, now, &cancel),
            client.warnings(Some(&region), now, &cancel),
            client.typhoons(&p, now, &cancel),
            client.search(Some(&region), &cancel)
        );
        let data = Dashboard {
            position: p,
            region: Some(region),
            forecast: Some(forecast.unwrap()),
            forecast_error: None,
            warnings: Feed {
                items: warnings.unwrap(),
                checked_at: Some(now),
                ..Default::default()
            },
            typhoons: Feed {
                items: typhoons.unwrap(),
                checked_at: Some(now),
                ..Default::default()
            },
            links: links.unwrap(),
            search_error: None,
        };
        assert_eq!(data.forecast.as_ref().unwrap().daily.len(), 7);
        assert!(!data.forecast.as_ref().unwrap().hourly.is_empty());
        std::fs::write(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../.test-temp/136-public-sample.json"),
            serde_json::to_vec_pretty(&data).unwrap(),
        )
        .unwrap();
        println!(
            "public sources verified: forecast={}, warnings={}, local typhoons={}, search links={}",
            data.forecast.as_ref().unwrap().daily.len(),
            data.warnings.items.len(),
            data.typhoons.items.len(),
            data.links.len()
        );
    });
}

#[test]
fn warning_body_extracts_public_description_and_guidelines_as_text() {
    let body=parse::warning_body("<html><nav>导航</nav><div id=alarmtext><p>气象台发布暴雨预警，请留意当地天气与出行情况。</p></div><div id=alarmtext><strong>防御指南：</strong><p>请根据官方信息做好准备。</p></div><script>malicious()</script></html>").unwrap();
    assert!(body.contains("防御指南"));
    assert!(!body.contains("导航"));
    assert!(!body.contains("malicious"));
    assert!(parse::warning_body("<html>服务暂不可用</html>").is_none());
}

#[test]
fn information_refresh_keeps_care_completed_while_fetching_the_same_event() {
    let mut snapshot = Feed::<Warning> {
        items: vec![warning("1_today", "yellow")],
        ..Default::default()
    };
    let mut current = snapshot.clone();
    current.care = Some(Care {
        persona_id: "1".into(),
        name: "小猫".into(),
        text: "记得带伞".into(),
    });
    current
        .attempted
        .insert("1".into(), parse::warning_key(&current.items));
    retain_care(&mut snapshot, &current, parse::warning_key);
    assert_eq!(snapshot.care.as_ref().unwrap().text, "记得带伞");
    assert_eq!(snapshot.attempted, current.attempted);
    snapshot.items[0].level = "red".into();
    snapshot.care = None;
    retain_care(&mut snapshot, &current, parse::warning_key);
    assert!(snapshot.care.is_none());
}

#[test]
fn information_worker_loads_once_and_waits_for_current_location() {
    struct Events;
    impl crate::events::PetEventSink for Events {
        fn changed(&self, _: &crate::pet::controller::View) {}
        fn failed(&self, _: &AppError) {}
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".test-temp")
        .canonicalize()
        .unwrap();
    let dir = root.join(format!(
        "136-information-worker-{}",
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    std::fs::create_dir(&dir).unwrap();
    let mut runtime = crate::runtime::test_runtime("http://localhost");
    runtime.paths = Ok(crate::platform::contracts::AppPaths::portable(dir.clone()));
    tauri::async_runtime::block_on(async {
        let mut worker = Worker::default();
        worker.tick(&runtime, Arc::new(Events));
        worker.task.take().unwrap().await.unwrap();
        assert!(worker.loaded);
        assert!(worker.last.is_none());
        worker.tick(&runtime, Arc::new(Events));
        assert!(worker.task.is_none());
        assert!(worker.last.is_none());
    });
    let canonical = dir.canonicalize().unwrap();
    assert!(canonical.starts_with(&root) && canonical != root);
    std::fs::remove_dir_all(canonical).unwrap();
}

#[test]
fn persisted_cache_keeps_care_deduplication_and_rejects_other_locations() {
    let mut d = Dashboard {
        position: position(),
        region: Some(region()),
        forecast: None,
        forecast_error: None,
        warnings: Default::default(),
        typhoons: Default::default(),
        links: vec![],
        search_error: None,
    };
    d.warnings.items.push(warning("1_old", "yellow"));
    let key = parse::warning_key(&d.warnings.items);
    d.warnings.attempted.insert("1".into(), key.clone());
    d.warnings.care = Some(Care {
        persona_id: "1".into(),
        name: "小猫".into(),
        text: "记得带伞".into(),
    });
    let mut restored: Dashboard = serde_json::from_slice(&serde_json::to_vec(&d).unwrap()).unwrap();
    assert_eq!(restored.warnings.attempted["1"], key);
    assert!(!update(
        &mut restored.warnings,
        Ok(vec![warning("1_new", "yellow")]),
        Utc::now(),
        parse::warning_key
    ));
    assert_eq!(restored.warnings.care.unwrap().text, "记得带伞");
    assert!(same_place(&d, &position()));
    let mut far = position();
    far.latitude = 40.;
    assert!(!same_place(&d, &far));
}

#[test]
fn care_failure_requires_manual_retry_and_success_survives_cache_reload() {
    struct Events;
    impl crate::events::PetEventSink for Events {
        fn changed(&self,_:&crate::pet::controller::View){}
        fn failed(&self,_:&AppError){}
    }
    use crate::ai::tests::{Response,Server};
    let body=serde_json::json!({"choices":[{"index":0,"message":{"role":"assistant","content":"出门记得带伞呀。"},"finish_reason":"stop"}]});
    let server=Server::new(vec![Response{status:503,mime:"application/json",chunks:vec![]},Response{status:200,mime:"application/json",chunks:vec![(0,serde_json::to_vec(&body).unwrap())]}]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".test-temp")
        .canonicalize()
        .unwrap();
    let dir = root.join(format!(
        "136-care-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    std::fs::create_dir(&dir).unwrap();
    let mut runtime = crate::runtime::test_runtime(&server.url);
    runtime.paths = Ok(crate::platform::contracts::AppPaths::portable(dir.clone()));
    runtime
        .read(|s, _| {
            s.connection.execute("UPDATE provider_models SET max_output_tokens=4096 WHERE purpose='chat'", []).unwrap();
            s.weather.information = Some(Dashboard {
                position: position(),
                region: Some(region()),
                forecast: None,
                forecast_error: None,
                warnings: Feed {
                    items: vec![warning("1_today", "yellow")],
                    ..Default::default()
                },
                typhoons: Default::default(),
                links: vec![],
                search_error: None,
            });
            Ok(())
        })
        .unwrap();
    tauri::async_runtime::block_on(async {
        assert!(care::generate(&runtime, 0, true, false, false, Arc::new(Events)).await.is_err());
        runtime.read(|s,_|{assert_eq!(s.weather.information.as_ref().unwrap().warnings.care_status["1"].state,"failed");Ok(())}).unwrap();
        let _ = care::generate(&runtime, 0, true, false, false, Arc::new(Events)).await;
        care::generate(&runtime, 0, true, false, true, Arc::new(Events)).await.unwrap();
        assert!(care::generate(&runtime, 0, true, false, true, Arc::new(Events)).await.is_err());
        runtime
            .read(|s, _| {
                assert_eq!(
                    s.weather
                        .information
                        .as_ref()
                        .unwrap()
                        .warnings
                        .care
                        .as_ref()
                        .unwrap()
                        .text,
                    "出门记得带伞呀。"
                );
                s.weather.information = None;
                Ok(())
            })
            .unwrap();
        load(&runtime).unwrap();
        let _ = care::generate(&runtime, 0, true, false, false, Arc::new(Events)).await;
    });
    assert_eq!(server.requests.lock().unwrap().len(),2);
    assert_eq!(server.requests.lock().unwrap()[1]["max_tokens"],4096);
    server.finish();
    let canonical = dir.canonicalize().unwrap();
    assert!(canonical.starts_with(&root) && canonical != root);
    std::fs::remove_dir_all(canonical).unwrap();
}

#[test]
fn care_cache_recovers_failed_or_interrupted_attempts_without_auto_retry(){
 let mut data=Dashboard{position:position(),region:Some(region()),forecast:None,forecast_error:None,warnings:Feed{items:vec![warning("w","yellow")],..Default::default()},typhoons:Default::default(),links:vec![],search_error:None};
 let key=parse::warning_key(&data.warnings.items);data.warnings.attempted.insert("1".into(),key.clone());
 care::recover(&mut data);assert_eq!(data.warnings.care_status["1"].state,"failed");assert_eq!(data.warnings.attempted["1"],key);
 data.warnings.care_status.get_mut("1").unwrap().state="generating".into();care::recover(&mut data);assert!(data.warnings.care_status["1"].error.as_ref().unwrap().contains("中断"));
 assert!(update(&mut data.warnings,Ok(vec![warning("w","red")]),Utc::now(),parse::warning_key));assert!(data.warnings.care_status.is_empty());
}
