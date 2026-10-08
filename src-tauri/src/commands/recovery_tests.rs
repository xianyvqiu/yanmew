use super::*;
use crate::{
    ai::tests::{Response, Server},
    pet::controller::Activity,
};
use serde_json::json;

fn mark_dormant(runtime: &Runtime) {
    runtime
        .read(|store, secrets| {
            let token = store.pet.health(&store.connection, secrets)?;
            store
                .pet
                .report_ai_failure(&token, &AppError::new("PROVIDER_UNAVAILABLE", "连接中断"));
            Ok(())
        })
        .unwrap();
}

#[test]
fn current_model_test_recovers_only_after_a_valid_completion() {
    for (response, success) in [
        (Response::json(json!("OK")), true),
        (
            Response {
                status: 200,
                mime: "application/json",
                chunks: vec![(0, br#"{"choices":[]}"#.to_vec())],
            },
            false,
        ),
        (
            Response {
                status: 401,
                mime: "application/json",
                chunks: vec![(0, b"private upstream error".to_vec())],
            },
            false,
        ),
    ] {
        let server = Server::new(vec![response]);
        let runtime = crate::runtime::test_runtime(&server.url);
        mark_dormant(&runtime);
        let result = tauri::async_runtime::block_on(check_provider(runtime.clone(), 1));
        assert_eq!(result.is_ok(), success);
        assert_eq!(
            runtime.pet_view().unwrap().activity == Activity::Dormant,
            !success
        );
        let requests = server.requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["model"], "fake-model");
        assert_eq!(requests[0]["max_tokens"], 32);
        if let Err(error) = result {
            assert!(!error.message.contains("private upstream"));
        }
        drop(requests);
        server.finish();
    }
}

#[test]
fn testing_another_provider_does_not_recover_the_current_one() {
    let server = Server::new(vec![Response::json(json!("OK"))]);
    let runtime = crate::runtime::test_runtime(&server.url);
    runtime
        .read(|store, _| {
            store.connection.execute(
                "INSERT INTO providers(id,name,base_url,model) VALUES(2,'另一个',?1,'other-model')",
                [&server.url],
            )?;
            Ok(())
        })
        .unwrap();
    mark_dormant(&runtime);
    tauri::async_runtime::block_on(check_provider(runtime.clone(), 2)).unwrap();
    assert_eq!(runtime.pet_view().unwrap().activity, Activity::Dormant);
    assert_eq!(server.requests.lock().unwrap()[0]["model"], "other-model");
    server.finish();
}

#[test]
fn late_failed_test_cannot_sleep_a_changed_provider_configuration() {
    let server = Server::new(vec![Response {
        status: 200,
        mime: "application/json",
        chunks: vec![(150, br#"{"choices":[]}"#.to_vec())],
    }]);
    let runtime = crate::runtime::test_runtime(&server.url);
    mark_dormant(&runtime);
    tauri::async_runtime::block_on(async {
        let change = async {
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while server.requests.lock().unwrap().is_empty() {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            runtime
                .read(|store, _| {
                    store
                        .connection
                        .execute("UPDATE providers SET model='new-model' WHERE id=1", [])?;
                    Ok(())
                })
                .unwrap();
            assert_eq!(runtime.pet_view().unwrap().error, None);
        };
        let (result, ()) = tokio::join!(check_provider(runtime.clone(), 1), change);
        assert_eq!(result.unwrap_err().code, "CONTEXT_CHANGED");
    });
    assert_eq!(runtime.pet_view().unwrap().error, None);
    server.finish();
}
