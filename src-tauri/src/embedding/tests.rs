use super::*;
#[test]
fn vector_validation_and_http_protocol() {
    assert!(normalize(vec![0.0; 2]).is_err());
    assert!(normalize(vec![f32::NAN]).is_err());
    assert!(decode(&[0; 7], 2).is_err());
    assert_eq!(normalize(vec![3.0, 4.0]).unwrap(), vec![0.6, 0.8]);
    use std::io::{Read, Write};
    for (status, body, valid) in [
        (
            "200 OK",
            r#"{"data":[{"index":0,"embedding":[3,4]}]}"#,
            true,
        ),
        (
            "200 OK",
            r#"{"data":[{"index":0,"embedding":[0,0]}]}"#,
            false,
        ),
        (
            "200 OK",
            r#"{"data":[{"index":1,"embedding":[1,2]}]}"#,
            false,
        ),
        ("401 Unauthorized", "private upstream detail", false),
    ] {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = server.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .unwrap();
            let mut buffer = [0; 8192];
            let count = stream.read(&mut buffer).unwrap();
            assert!(String::from_utf8_lossy(&buffer[..count]).starts_with("POST /v1/embeddings "));
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let client = crate::ai::client::AiClient::new(Default::default()).unwrap();
        let result = tauri::async_runtime::block_on(client.embedding(
            &crate::providers::ConnectionTest {
                base_url: format!("http://{address}/v1"),
                model: "model".into(),
                api_key: None,
            },
            "测试",
            &Cancellation::default(),
        ));
        assert_eq!(result.is_ok(), valid);
        if let Err(error) = result {
            assert!(!error.message.contains("private upstream"));
        }
        handle.join().unwrap();
    }
}

#[test]
#[ignore = "一次真实资源验证，资源变化后才重跑"]
fn local_resource_validation() {
    #[repr(C)]
    #[derive(Default)]
    struct Memory {
        cb: u32,
        faults: u32,
        peak: usize,
        working: usize,
        paged_peak: usize,
        paged: usize,
        nonpaged_peak: usize,
        nonpaged: usize,
        pagefile: usize,
        pagefile_peak: usize,
    }
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut Memory,
            size: u32,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
    }
    fn memory() -> Memory {
        let mut m = Memory::default();
        m.cb = std::mem::size_of::<Memory>() as u32;
        assert_ne!(
            unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut m, m.cb) },
            0
        );
        m
    }
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/models/bge-small-zh-v1.5");
    let engine = Engine::new(Ok(root.clone()));
    let before = memory();
    let started = std::time::Instant::now();
    let model = engine.load().unwrap();
    let load_ms = started.elapsed().as_secs_f64() * 1000.0;
    engine.state.lock().unwrap().model = Some(model);
    let after_load = memory();
    let cancel = Cancellation::default();
    let docs = [
        "用户读完三体，最喜欢史强，讨论了大史这个人物。",
        "用户去杭州旅行，在西湖散步，带回桂花糕。",
        "用户明天参加面试，讨论了面试准备。",
        "用户今天中午吃了牛肉面。",
    ];
    let first = std::time::Instant::now();
    let mut vectors = vec![engine.local(docs[0], false, &cancel).unwrap().values];
    let first_ms = first.elapsed().as_secs_f64() * 1000.0;
    let steady = std::time::Instant::now();
    for doc in &docs[1..] {
        vectors.push(engine.local(doc, false, &cancel).unwrap().values);
    }
    let steady_ms = steady.elapsed().as_secs_f64() * 1000.0 / 3.0;
    let mut calibration = Vec::new();
    for (query, expected) in [
        ("以前最喜欢三体里的谁", Some(0)),
        ("之前去哪个湖边散步，有什么糕点伴手礼", Some(1)),
        ("之前怎么准备面试", Some(2)),
        ("帮我写一个排序算法", None),
    ] {
        let q = engine.local(query, true, &cancel).unwrap().values;
        let scores = vectors
            .iter()
            .map(|v| v.iter().zip(&q).map(|(a, b)| a * b).sum::<f32>())
            .collect::<Vec<_>>();
        let best = scores
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        if let Some(expected) = expected {
            assert_eq!(best, expected);
        }
        calibration.push(serde_json::json!({"query":query,"expected":expected,"scores":scores}));
    }
    assert_eq!(
        engine
            .local(&"长".repeat(600), false, &cancel)
            .unwrap_err()
            .code,
        "EMBEDDING_TOO_LONG"
    );
    let after = memory();
    let size = std::fs::metadata(root.join("model.onnx")).unwrap().len();
    let record = serde_json::json!({"loadMs":load_ms,"firstInferenceMs":first_ms,"steadyInferenceMs":steady_ms,"beforeWorkingBytes":before.working,"loadedWorkingBytes":after_load.working,"finalWorkingBytes":after.working,"peakWorkingBytes":after.peak,"modelBytes":size,"logicalCpus":std::thread::available_parallelism().unwrap().get(),"intraThreads":4,"calibration":calibration});
    std::fs::write(
        root.join("../../../my_doc/embedding-resource-validation.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    println!("{record}");
}
