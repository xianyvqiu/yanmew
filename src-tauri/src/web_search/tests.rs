use super::*;
use crate::{
    ai::{
        client::{AiClient, Timeouts},
        provider::ToolCall,
        tests::{Response, Server},
    },
    runtime::test_runtime,
    state::Cancellation,
};
use serde_json::{json, Value};
fn input(kind: Kind, base: &str, key: SecretUpdate) -> Input {
    Input {
        name: "搜索服务".into(),
        kind,
        base_url: base.into(),
        api_key: key,
    }
}
fn set_key() -> SecretUpdate {
    SecretUpdate::Set {
        value: "fake-test-key".into(),
    }
}
fn configure(runtime: &crate::runtime::Runtime, kind: Kind, base: &str) {
    runtime
        .read(|s, secrets| {
            save(
                &mut s.connection,
                None,
                input(kind, base, set_key()),
                secrets,
            )?;
            enable(&mut s.connection, true)
        })
        .unwrap();
}
fn response(value: Value) -> Response {
    Response {
        status: 200,
        mime: "application/json",
        chunks: vec![(0, serde_json::to_vec(&value).unwrap())],
    }
}
struct Protected;
impl SecretProtector for Protected {
    fn protect(&self, text: &str) -> AppResult<String> {
        Ok(format!("protected:{text}"))
    }
    fn unprotect(&self, text: &str) -> AppResult<String> {
        text.strip_prefix("protected:")
            .map(str::to_string)
            .ok_or_else(AppError::storage)
    }
}
#[test]
fn configuration_is_persistent_encrypted_and_changes_invalidate_snapshot() {
    let runtime = test_runtime("https://example.com/v1");
    runtime
        .read(|s, _| {
            let view = view(&s.connection)?;
            assert!(!view.enabled);
            assert!(view.providers.is_empty());
            Ok(())
        })
        .unwrap();
    runtime
        .read(|s, _| {
            let secrets = &Protected;
            save(
                &mut s.connection,
                None,
                input(Kind::Tavily, "https://api.tavily.com", set_key()),
                secrets,
            )?;
            enable(&mut s.connection, true)?;
            let stamp = snapshot(&s.connection, secrets, None)?.unwrap();
            let public = serde_json::to_string(&view(&s.connection)?).unwrap();
            assert!(!public.contains("fake-test-key"));
            assert!(view(&s.connection)?.providers[0].has_api_key);
            assert_ne!(secret(&s.connection, "1")?.unwrap(), "fake-test-key");
            assert_eq!(
                delete(&mut s.connection, "1").unwrap_err().code,
                "VALIDATION_ERROR"
            );
            let before = load(&s.connection)?.revision;
            assert!(save(
                &mut s.connection,
                Some("1".into()),
                input(Kind::Bocha, "https://api.bocha.cn", SecretUpdate::Keep),
                secrets
            )
            .is_err());
            assert_eq!(load(&s.connection)?.revision, before);
            save(
                &mut s.connection,
                Some("1".into()),
                input(Kind::Tavily, "https://api.tavily.com", SecretUpdate::Clear),
                secrets,
            )?;
            assert!(!view(&s.connection)?.enabled);
            assert_eq!(
                stamp.check(&s.connection).unwrap_err().code,
                "WEB_SEARCH_CONFIGURATION_CHANGED"
            );
            assert!(enable(&mut s.connection, true).is_err());
            Ok(())
        })
        .unwrap();
}
#[test]
fn both_search_protocols_send_five_results_and_normalize_sources() {
    tauri::async_runtime::block_on(async {
        for (kind, path, data) in [
            (
                Kind::Tavily,
                "/v1/search",
                json!({"results":[{"title":"中文来源","url":"https://example.com/news","content":"事实摘要","published_date":"2026-10-08"}]}),
            ),
            (
                Kind::Custom,
                "/v1/search",
                json!({"results":[{"title":"中文来源","url":"https://example.com/news","content":"事实摘要","published_date":"2026-10-08"}]}),
            ),
            (
                Kind::Bocha,
                "/v1/v1/web-search",
                json!({"code":200,"data":{"webPages":{"value":[{"name":"中文来源","url":"https://example.com/news","summary":"事实摘要","datePublished":"2026-10-08"}]}}}),
            ),
        ] {
            let server = Server::new_at(vec![response(data)], path);
            let runtime = test_runtime("https://example.com/v1");
            configure(&runtime, kind, &server.url);
            let stamp = runtime
                .read(|s, secrets| snapshot(&s.connection, secrets, None))
                .unwrap()
                .unwrap();
            let client = AiClient::new(Timeouts::default()).unwrap();
            let args = tools::SearchArgs {
                query: "最新消息".into(),
                topic: "news".into(),
                time_range: Some("week".into()),
            };
            let result = network::search(&client, &stamp, &args, &Cancellation::default())
                .await
                .unwrap();
            assert_eq!(result[0].title, "中文来源");
            assert_eq!(result[0].excerpt, "事实摘要");
            assert_eq!(result[0].published_at.as_deref(), Some("2026-10-08"));
            let requests = server.requests.lock().unwrap();
            let body = &requests[0];
            assert_eq!(body["query"], "最新消息");
            if kind != Kind::Bocha {
                assert_eq!(body["max_results"], 5);
                assert_eq!(body["time_range"], "week");
                assert_eq!(body["include_raw_content"], false);
            } else {
                assert_eq!(body["count"], 5);
                assert_eq!(body["freshness"], "oneWeek");
                assert_eq!(body["summary"], true);
            }
            drop(requests);
            server.finish();
        }
    });
}
#[test]
fn tool_replay_does_not_spend_again_and_configuration_changes_block_results() {
    tauri::async_runtime::block_on(async {
        let server = Server::new_at(
            vec![response(
                json!({"results":[{"title":"资料","url":"https://example.com/news","content":"摘要"}]}),
            )],
            "/v1/search",
        );
        let runtime = test_runtime("https://example.com/v1");
        configure(&runtime, Kind::Tavily, &server.url);
        let client = AiClient::new(Timeouts::default()).unwrap();
        let cancel = Cancellation::default();
        let mut session = SearchSession::new(&runtime);
        let call = ToolCall {
            index: 0,
            id: "search-1".into(),
            name: "search_web".into(),
            arguments: json!({"query":"今天的新闻"}),
        };
        let first = session
            .execute(&client, &runtime, &call, &cancel)
            .await
            .unwrap();
        assert_eq!(
            first,
            session
                .execute(&client, &runtime, &call, &cancel)
                .await
                .unwrap()
        );
        assert!(session.footer("回复").contains("https://example.com/news"));
        let open = ToolCall {
            index: 1,
            id: "open-1".into(),
            name: "open_web_page".into(),
            arguments: json!({"url":"https://other.example.com/"}),
        };
        assert_eq!(
            session
                .execute(&client, &runtime, &open, &cancel)
                .await
                .unwrap_err()
                .code,
            "WEB_PAGE_DENIED"
        );
        runtime
            .read(|s, _| enable(&mut s.connection, false))
            .unwrap();
        assert_eq!(
            session
                .execute(&client, &runtime, &call, &cancel)
                .await
                .unwrap_err()
                .code,
            "WEB_SEARCH_CONFIGURATION_CHANGED"
        );
        let mut off = SearchSession::new(&runtime);
        assert!(!off.available());
        assert_eq!(
            off.execute(&client, &runtime, &call, &cancel)
                .await
                .unwrap_err()
                .code,
            "PERMISSION_DENIED"
        );
        cancel.cancel();
        assert_eq!(
            off.execute(&client, &runtime, &call, &cancel)
                .await
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        server.finish();
    });
}
#[test]
fn reader_denies_private_addresses_and_extracts_only_visible_text() {
    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "192.168.1.1",
        "169.254.169.254",
        "100.64.0.1",
        "::1",
        "fc00::1",
        "fe80::1",
        "::ffff:127.0.0.1",
        "2001:db8::1",
    ] {
        assert!(!network::public_ip(address.parse().unwrap()), "{address}");
    }
    for address in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        assert!(network::public_ip(address.parse().unwrap()), "{address}");
    }
    for url in [
        "file:///C:/secret",
        "http://localhost/",
        "http://127.0.0.1/",
        "https://user:pass@example.com/",
        "http://[::1]/",
    ] {
        assert!(network::page_url(url).is_err());
    }
    let (title,text)=network::extract_html("<html><head><title>新闻</title></head><body><nav>菜单</nav><main><h1>标题</h1><p>事实 &amp; 来源🙂</p><script>不可信脚本</script><aside>广告</aside></main></body></html>").unwrap();
    assert_eq!(title, "新闻");
    assert!(text.contains("事实 & 来源🙂"));
    assert!(!text.contains("菜单"));
    assert!(!text.contains("不可信脚本"));
    assert!(!text.contains("广告"));
}
#[test]
fn service_errors_and_invalid_results_are_not_model_provider_errors() {
    assert_eq!(
        network::parse_search(Kind::Bocha, &json!({"code":401,"msg":"fake-test-key"}))
            .unwrap_err()
            .code,
        "WEB_SEARCH_FAILED"
    );
    assert!(network::parse_search(
        Kind::Tavily,
        &json!({"results":[{"url":"file:///C:/secret"}]})
    )
    .unwrap()
    .is_empty());
    assert!(network::parse_search(Kind::Tavily, &json!({"unexpected":"bad"})).is_err());
}
