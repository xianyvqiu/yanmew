use super::*;
use crate::ai::tests::{client, Response, Server};
fn snapshot(base: &str, model: &str) -> Snapshot {
    Snapshot {
        provider: crate::image_generation::Provider {
            id: "1".into(),
            name: "模拟".into(),
            protocol: "openai-compatible".into(),
            base_url: base.into(),
            model: model.into(),
        },
        revision: 1,
        key: "test-image-key".into(),
    }
}
fn data(request: &reqwest::Request) -> Value {
    serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap()
}
#[test]
fn only_official_hosts_select_native_protocols() {
    for (base, kind) in [
        (
            "https://generativelanguage.googleapis.com/v1beta",
            Kind::Gemini,
        ),
        ("https://dashscope.aliyuncs.com", Kind::Dashscope),
        (
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
            Kind::DashscopeCompatible,
        ),
        (
            "https://abc.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            Kind::DashscopeCompatible,
        ),
        ("https://ark.cn-beijing.volces.com/api/v3", Kind::Ark),
        ("https://openrouter.ai/api/v1", Kind::OpenRouter),
    ] {
        assert_eq!(detect(base), Some(kind));
    }
    for base in [
        "https://generativelanguage.googleapis.com.evil.example/v1beta",
        "https://notdashscope.aliyuncs.com",
        "https://maas.aliyuncs.com.evil.example",
        "https://evil.example/openrouter.ai",
        "https://openrouter.ai.evil.example/api/v1",
        "https://gateway.example/v1",
    ] {
        assert_eq!(detect(base), None);
    }
    assert_eq!(
        detect("https://GENERATIVELANGUAGE.GOOGLEAPIS.COM./v1beta"),
        Some(Kind::Gemini)
    );
}
#[test]
fn gemini_root_full_endpoint_and_openai_base_use_model_binding_and_key_header() {
    for base in [
        "https://generativelanguage.googleapis.com",
        "https://generativelanguage.googleapis.com/v1beta/openai",
        "https://generativelanguage.googleapis.com/v1beta/models/old-model:generateContent",
    ] {
        let cfg = snapshot(base, "gemini-2.5-flash-image");
        let request =
            super::super::edit_request(&client(), &cfg, b"reference", "同一角色挥手", false)
                .unwrap()
                .build()
                .unwrap();
        assert_eq!(request.url().as_str(),"https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent");
        assert!(request.url().query().is_none());
        assert_eq!(request.headers()["x-goog-api-key"], "test-image-key");
        assert!(!request.headers().contains_key("authorization"));
        let body = data(&request);
        assert_eq!(
            body["contents"][0]["parts"][1]["inlineData"]["data"],
            base64::engine::general_purpose::STANDARD.encode(b"reference")
        );
        assert_eq!(body["generationConfig"]["candidateCount"], 1);
        assert!(body.get("background").is_none());
    }
    assert_eq!(
        target(
            Kind::Gemini,
            &snapshot(
                "https://generativelanguage.googleapis.com/v1",
                "models/new-image-model"
            )
        )
        .unwrap()
        .path(),
        "/v1/models/new-image-model:generateContent"
    );
    for model in ["../bad", "new?key=other", "a/b", "bad:method"] {
        assert!(target(
            Kind::Gemini,
            &snapshot("https://generativelanguage.googleapis.com", model)
        )
        .is_err());
    }
}
#[test]
fn dashscope_native_and_compatible_keep_reference_and_old_edit_uses_native() {
    for (base, model, native) in [
        ("https://dashscope.aliyuncs.com", "qwen-image-edit", true),
        (
            "https://space.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1",
            "qwen-image-3.0-pro",
            false,
        ),
        (
            "https://dashscope.aliyuncs.com/compatible-mode/v1/images/edits",
            "qwen-image-edit-plus",
            true,
        ),
    ] {
        let cfg = snapshot(base, model);
        let request =
            super::super::edit_request(&client(), &cfg, b"reference", "同一角色换动作", true)
                .unwrap()
                .build()
                .unwrap();
        let body = data(&request);
        assert_eq!(request.headers()["authorization"], "Bearer test-image-key");
        assert!(request
            .url()
            .as_str()
            .starts_with(base.split("/compatible-mode").next().unwrap()));
        if native {
            assert_eq!(
                request.url().path(),
                "/api/v1/services/aigc/multimodal-generation/generation"
            );
            assert!(body["input"]["messages"][0]["content"][0]["image"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,"));
            assert_eq!(body["parameters"]["n"], 1);
        } else {
            assert_eq!(
                request.url().path(),
                "/compatible-mode/v1/images/generations"
            );
            assert!(body["image"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,"));
            assert_eq!(body["n"], 1);
        }
        assert!(body.get("background").is_none());
        assert!(body.get("size").is_none());
    }
    for model in [
        "wan2.7-image",
        "z-image-turbo",
        "qwen-image-plus",
        "future-image-model",
    ] {
        for base in [
            "https://dashscope.aliyuncs.com",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
        ] {
            for image in [Some(b"reference".as_slice()), None] {
                let built = request(&client(), &snapshot(base, model), "换动作", image)
                    .unwrap()
                    .unwrap()
                    .build()
                    .unwrap();
                let body = data(&built);
                assert_eq!(body["model"], model);
                if image.is_some() {
                    let reference = if base.contains("compatible-mode") {
                        &body["image"]
                    } else {
                        &body["input"]["messages"][0]["content"][0]["image"]
                    };
                    assert_eq!(reference, &json!("data:image/png;base64,cmVmZXJlbmNl"));
                }
            }
        }
    }
}
#[test]
fn ark_and_openrouter_use_their_json_reference_formats_without_transparent_fields() {
    for (kind, cfg) in [
        (
            Kind::Ark,
            snapshot(
                "https://ark.cn-beijing.volces.com/api/v3/images/generations",
                "doubao-seedream-4-5-test",
            ),
        ),
        (
            Kind::OpenRouter,
            snapshot(
                "https://openrouter.ai/api/v1",
                "google/gemini-2.5-flash-image",
            ),
        ),
    ] {
        let request = request_for(kind, &client(), &cfg, "同一角色在海边", Some(b"reference"))
            .unwrap()
            .build()
            .unwrap();
        let body = data(&request);
        assert_eq!(body["model"], cfg.provider.model);
        assert_eq!(request.headers()["authorization"], "Bearer test-image-key");
        assert!(body.get("background").is_none());
        assert!(body.get("output_format").is_none());
        if kind == Kind::Ark {
            assert_eq!(request.url().path(), "/api/v3/images/generations");
            assert_eq!(body["response_format"], "b64_json");
            assert_eq!(body["size"], "2K");
            assert_eq!(body["sequential_image_generation"], "disabled");
            assert!(body["image"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,"));
        } else {
            assert_eq!(request.url().path(), "/api/v1/images");
            assert_eq!(body["n"], 1);
            assert_eq!(body["provider"]["allow_fallbacks"], false);
            assert!(body["input_references"][0]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,"));
        }
    }
    for model in [
        "doubao-seedream-5-0-pro-260628",
        "doubao-seedream-5-0-flash-test",
        "ep-custom-id",
    ] {
        let request = request_for(
            Kind::Ark,
            &client(),
            &snapshot("https://ark.cn-beijing.volces.com/api/v3", model),
            "画猫",
            None,
        )
        .unwrap()
        .build()
        .unwrap();
        assert!(data(&request).get("sequential_image_generation").is_none());
    }
}
#[test]
fn shared_plain_generation_routes_native_requests_without_reference_images() {
    for (base, model) in [
        (
            "https://generativelanguage.googleapis.com/v1beta",
            "image-model",
        ),
        ("https://dashscope.aliyuncs.com", "qwen-image-3.0-pro"),
        (
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen-image-3.0-pro",
        ),
        ("https://ark.cn-beijing.volces.com/api/v3", "seedream-model"),
        ("https://openrouter.ai/api/v1", "image-model"),
    ] {
        let request = super::super::generation_request(
            &client(),
            &snapshot(base, model),
            "一枚透明底小饰品",
            true,
        )
        .unwrap()
        .build()
        .unwrap();
        let body = data(&request);
        assert!(!body.to_string().contains("base64,"));
        assert!(body.get("image").is_none());
        assert!(body.get("input_references").is_none());
        assert!(body.get("api_key").is_none());
    }
}
#[tokio::test]
async fn gemini_skips_thoughts_accepts_camel_and_snake_images_and_rejects_incomplete_output() {
    for inline in [
        json!({"inlineData":{"mimeType":"image/png","data":"AQID"}}),
        json!({"inline_data":{"mime_type":"image/jpeg","data":"AQID"}}),
    ] {
        let value = json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"thought":true,"inlineData":{"mimeType":"image/png","data":"AA=="}},{"text":"说明"},inline]}}]});
        assert_eq!(
            super::super::parse(gemini_image(value).unwrap(), &Cancellation::default())
                .await
                .unwrap(),
            vec![1, 2, 3]
        );
    }
    for value in [
        json!({"promptFeedback":{"blockReason":"SAFETY"}}),
        json!({"candidates":[{"finishReason":"MAX_TOKENS","content":{"parts":[{"inlineData":{"mimeType":"image/png","data":"AQID"}}]}}]}),
        json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":"只返回说明"}]}}]}),
        json!({"candidates":[{"content":{"parts":[{"thought":true,"inlineData":{"mimeType":"image/png","data":"AQID"}}]}}]}),
        json!({"candidates":[{"content":{"parts":[{"inlineData":{"mimeType":"image/svg+xml","data":"AQID"}}]}}]}),
    ] {
        assert!(gemini_image(value).is_err());
    }
}
#[tokio::test]
async fn dashscope_outputs_use_existing_byte_limits_and_private_url_rejection() {
    let value = json!({"output":{"choices":[{"finish_reason":"stop","message":{"content":[{"text":"完成"},{"image":"data:image/png;base64,AQID"}]}}]}});
    assert_eq!(
        super::super::parse(dashscope_image(value).unwrap(), &Cancellation::default())
            .await
            .unwrap(),
        vec![1, 2, 3]
    );
    let private = json!({"output":{"choices":[{"message":{"content":[{"image":"https://127.0.0.1/a.png"}]}}]}});
    assert!(
        super::super::parse(dashscope_image(private).unwrap(), &Cancellation::default())
            .await
            .is_err()
    );
    for value in [
        json!({"code":"InvalidApiKey"}),
        json!({"output":{"task_id":"pending-task","task_status":"PENDING"}}),
        json!({"output":{"choices":[{"finish_reason":"length","message":{"content":[{"image":"data:image/png;base64,AQID"}]}}]}}),
        json!({"output":{"choices":[{"message":{"content":[{"text":"没有图片"}]}}]}}),
    ] {
        assert!(dashscope_image(value).is_err());
    }
    assert!(super::super::parse(
        image_value("data:image/png;base64,invalid!").unwrap(),
        &Cancellation::default()
    )
    .await
    .is_err());
    assert!(super::super::parse(
        image_value(&format!(
            "data:image/png;base64,{}",
            "A".repeat(MAX_IMAGE.div_ceil(3) * 4 + 4)
        ))
        .unwrap(),
        &Cancellation::default()
    )
    .await
    .is_err());
}
#[test]
fn native_error_entries_are_not_treated_as_success_or_replaced_by_later_images() {
    for value in [
        json!({"error":{"message":"private error"}}),
        json!({"data":[{"error":{"message":"failed"}},{"b64_json":"AQID"}]}),
    ] {
        for kind in [Kind::Ark, Kind::OpenRouter, Kind::DashscopeCompatible] {
            let error = normalize_for(kind, value.clone()).unwrap_err();
            assert!(!error.message.contains("private error"));
        }
    }
}
#[tokio::test]
async fn mock_native_json_round_trips_attach_references_and_decode_one_image() {
    for kind in [
        Kind::Gemini,
        Kind::Dashscope,
        Kind::DashscopeCompatible,
        Kind::Ark,
        Kind::OpenRouter,
    ] {
        let value = match kind {
            Kind::Gemini => {
                json!({"candidates":[{"finishReason":"STOP","content":{"parts":[{"inlineData":{"mimeType":"image/png","data":"AQID"}}]}}]})
            }
            Kind::Dashscope => {
                json!({"output":{"choices":[{"message":{"content":[{"image":"data:image/png;base64,AQID"}]}}]}})
            }
            _ => json!({"data":[{"b64_json":"AQID"}]}),
        };
        let cfg = snapshot("http://127.0.0.1:1/v1", "image-model");
        // Shared mock server compares the lowercased request header.
        let path = target(kind, &cfg).unwrap().path().to_ascii_lowercase();
        let server = Server::new_at(
            vec![Response {
                status: 200,
                mime: "application/json",
                chunks: vec![(0, serde_json::to_vec(&value).unwrap())],
            }],
            &path,
        );
        let cfg = snapshot(&server.url, "image-model");
        let request = request_for(kind, &client(), &cfg, "挥手", Some(b"reference")).unwrap();
        let response = request.send().await.unwrap();
        let result = super::super::body(response, MAX_RESPONSE).await.unwrap();
        let normalized = normalize_for(kind, serde_json::from_slice(&result).unwrap()).unwrap();
        assert_eq!(
            super::super::parse(normalized, &Cancellation::default())
                .await
                .unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        server.finish();
    }
}
#[test]
fn model_catalog_routes_stay_on_the_selected_origin() {
    for (base,want,google) in [("https://generativelanguage.googleapis.com/v1beta/openai","https://generativelanguage.googleapis.com/v1beta/models",true),("https://dashscope-intl.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation","https://dashscope-intl.aliyuncs.com/compatible-mode/v1/models",false),("https://ark.cn-beijing.volces.com/api/v3/images/generations","https://ark.cn-beijing.volces.com/api/v3/models",false),("https://openrouter.ai/api/v1/images","https://openrouter.ai/api/v1/images/models",false)] { assert_eq!(catalog(base).unwrap().unwrap(),(want.into(),google)); }
    assert!(catalog("https://gateway.example/v1").is_none());
}

#[test]
fn native_generation_uses_existing_task_budget_without_shortening_legacy_calls() {
    let client = AiClient::new(crate::ai::client::Timeouts {
        total: Duration::from_secs(600),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        super::super::generation_timeout(
            &client,
            &snapshot("https://generativelanguage.googleapis.com/v1beta", "image")
        ),
        Duration::from_secs(600)
    );
    assert_eq!(
        super::super::generation_timeout(&client, &snapshot("https://gateway.example/v1", "image")),
        Duration::from_secs(120)
    );
    assert_eq!(
        detect("https://ark.ap-southeast.bytepluses.com/api/v3"),
        Some(Kind::Ark)
    );
    assert_eq!(detect("https://ark.cn-shanghai.volces.com/api/v3"), None);
}
