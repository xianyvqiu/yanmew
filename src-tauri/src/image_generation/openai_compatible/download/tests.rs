use super::*;
use serde_json::json;

fn reply(host: &str, kind: u16, records: Value) -> Value {
    json!({"Status":0,"TC":false,"Question":[{"name":host,"type":kind}],"Answer":records})
}
#[test]
fn resolves_aliases_and_both_question_formats_without_unrelated_records() {
    let mut value = reply(
        "CDN.Example.",
        1,
        json!([
            {"name":"cdn.example.","type":5,"data":"edge.example."},
            {"name":"edge.example.","type":1,"data":"8.8.8.8"},
            {"name":"other.example.","type":1,"data":"127.0.0.1"}
        ]),
    );
    let expected = vec!["8.8.8.8:8443".parse::<SocketAddr>().unwrap()];
    assert_eq!(answers(&value, "cdn.example", 1, 8443).unwrap(), expected);
    value["Question"] = json!({"name":"cdn.example.","type":1});
    assert_eq!(answers(&value, "cdn.example", 1, 8443).unwrap(), expected);
    let value = reply(
        "cdn.example",
        28,
        json!([{"name":"cdn.example","type":28,"data":"2606:4700:4700::1111"}]),
    );
    assert_eq!(
        answers(&value, "cdn.example", 28, 443).unwrap(),
        vec!["[2606:4700:4700::1111]:443".parse::<SocketAddr>().unwrap()]
    );
    assert!(
        answers(&reply("cdn.example", 28, json!([])), "cdn.example", 28, 443)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn fake_private_and_reserved_addresses_never_become_download_targets() {
    for ip in [
        "127.0.0.1",
        "10.0.0.1",
        "192.168.1.1",
        "172.16.0.1",
        "169.254.169.254",
        "198.18.0.1",
        "198.19.0.249",
        "100.64.0.1",
        "192.0.2.1",
        "198.51.100.1",
        "203.0.113.1",
        "0.0.0.0",
        "::1",
        "fc00::1",
        "fe80::1",
        "::ffff:10.0.0.1",
    ] {
        let kind = if ip.contains(':') { 28 } else { 1 };
        let value = reply(
            "cdn.example",
            kind,
            json!([{"name":"cdn.example","type":kind,"data":ip}]),
        );
        assert!(answers(&value, "cdn.example", kind, 443).is_err(), "{ip}");
    }
    assert!(!public_addresses(&[]));
    assert!(!public_addresses(&[
        "8.8.8.8:443".parse().unwrap(),
        "198.19.0.1:443".parse().unwrap()
    ]));
    assert!(public_addresses(&["8.8.8.8:443".parse().unwrap()]));
}
#[test]
fn rejects_wrong_questions_truncated_bad_records_and_alias_loops() {
    let valid = reply(
        "cdn.example",
        1,
        json!([{"name":"cdn.example","type":1,"data":"8.8.8.8"}]),
    );
    for (key, bad) in [
        ("Status", json!(2)),
        ("TC", json!(true)),
        ("Answer", json!("bad")),
        ("Question", json!([{"name":"other.example","type":1}])),
        ("Question", json!([{"name":"cdn.example","type":28}])),
        ("Question", json!([])),
    ] {
        let mut value = valid.clone();
        value[key] = bad;
        assert!(answers(&value, "cdn.example", 1, 443).is_err());
    }
    for records in [
        json!([{"name":"cdn.example","type":1,"data":"invalid"}]),
        json!([{"name":"cdn.example","type":1,"data":"2606:4700:4700::1111"}]),
        json!([{"name":"cdn.example","type":5,"data":"cdn.example"}]),
        json!([{"name":"cdn.example","type":5,"data":"localhost."}]),
        json!([{"name":"cdn.example","type":5,"data":"a.example"},{"name":"a.example","type":5,"data":"cdn.example"}]),
        json!([{"name":"cdn.example","type":5,"data":"a.example"},{"name":"cdn.example","type":5,"data":"b.example"}]),
        json!([{"name":"cdn.example","type":1,"data":"8.8.8.8"},{"name":"cdn.example","type":1,"data":"10.0.0.1"}]),
    ] {
        assert!(answers(&reply("cdn.example", 1, records), "cdn.example", 1, 443).is_err());
    }
}
#[test]
fn rejects_private_hosts_and_unsafe_redirect_targets_including_trailing_dots() {
    for raw in [
        "https://localhost./x",
        "https://LOCALHOST./x",
        "https://printer/x",
        "https://device.local./x",
        "https://device.lan/x",
        "https://device.internal/x",
        "https://device.home/x",
        "https://2130706433/x",
        "https://[::ffff:127.0.0.1]/x",
        "https://198.19.0.1/x",
        "https://192.0.2.1/x",
        "http://cdn.example/x",
        "file:///image.png",
        "https://user:secret@cdn.example/x",
    ] {
        assert!(public_url(raw).is_err(), "{raw}");
    }
    assert!(public_url("https://cdn.example/image.png?signature=preserved").is_ok());
}

fn server(status: u16) -> (String, std::thread::JoinHandle<Vec<String>>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let worker = std::thread::spawn(move || {
        let mut requests = Vec::new();
        for _ in 0..2 {
            let start = std::time::Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(start.elapsed() < Duration::from_secs(10));
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0u8; 1024];
            while !bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                assert!(bytes.len() < 8192);
            }
            let header = String::from_utf8(bytes).unwrap();
            assert!(!header.to_ascii_lowercase().contains("authorization:"));
            assert!(header.starts_with("GET /resolve?name=cdn.example&type="));
            let kind = if header.lines().next().unwrap().contains("type=28") {
                28
            } else {
                1
            };
            let record = if kind == 1 {
                json!([{"name":"cdn.example","type":1,"data":"8.8.8.8"}])
            } else {
                json!([])
            };
            let body = reply("cdn.example", kind, record).to_string();
            write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            requests.push(header);
        }
        requests
    });
    (format!("http://resolver.invalid:{port}/resolve"), worker)
}
#[tokio::test]
async fn independent_dns_falls_back_and_sends_only_the_hostname() {
    let (first, a) = server(503);
    let (second, b) = server(200);
    let resolvers = [
        (first.as_str(), "resolver.invalid", &["127.0.0.1"][..]),
        (second.as_str(), "resolver.invalid", &["127.0.0.1"][..]),
    ];
    let addresses = public_dns("cdn.example", 8443, &Cancellation::default(), &resolvers)
        .await
        .unwrap();
    assert_eq!(
        addresses,
        vec!["8.8.8.8:8443".parse::<SocketAddr>().unwrap()]
    );
    assert_eq!(a.join().unwrap().len(), 2);
    assert_eq!(b.join().unwrap().len(), 2);
}
#[tokio::test]
async fn cancellation_stops_dns_before_any_request() {
    let cancel = Cancellation::default();
    cancel.cancel();
    assert_eq!(
        public_dns("cdn.example", 443, &cancel, &RESOLVERS)
            .await
            .unwrap_err()
            .code,
        "CANCELLED"
    );
    assert_eq!(
        read("https://cdn.example/image.png", &cancel)
            .await
            .unwrap_err()
            .code,
        "CANCELLED"
    );
}
#[tokio::test]
#[ignore = "显式联网验收：仅下载官网已有PNG，不调用生图API"]
async fn public_image_download_works_with_current_network_without_generation() {
    let cancel = Cancellation::default();
    let bytes = cancel
        .wait(
            Duration::from_secs(45),
            read("https://www.python.org/static/img/python-logo.png", &cancel),
        )
        .await
        .unwrap();
    assert_eq!(
        image::guess_format(&bytes).unwrap(),
        image::ImageFormat::Png
    );
    assert!(bytes.len() > 1000);
}
