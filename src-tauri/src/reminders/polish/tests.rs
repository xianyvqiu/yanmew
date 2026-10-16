use super::*;
use crate::ai::tests::{Response, Server};
use serde_json::json;

fn setup(url: &str) -> Runtime {
    let runtime = crate::runtime::test_runtime(url);
    runtime.read(|s, _| {
        s.connection.execute_batch("INSERT INTO reminders(id,name,kind,content,status,fire_at,source,created_at) VALUES(1,'喝水','once','一杯水','pending',NULL,'manual','2026-10-14T01:00:00Z');
            INSERT INTO reminder_occurrences(id,reminder_id,scheduled_at,status,delivery_state,created_at) VALUES(1,1,'2026-10-14T01:00:00Z','pending','queued','2026-10-14T01:00:00Z');")?;
        Ok(())
    }).unwrap();
    runtime
}
#[test]
fn concurrent_readers_share_one_generation_and_never_modify_persisted_facts() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(
            json!({"text":"该喝水啦，休息一下吧。"}),
        )]);
        let runtime = setup(&server.url);
        let changes = runtime
            .read(|s, _| Ok(s.connection.total_changes()))
            .unwrap();
        let (a, b) = tokio::join!(
            get(runtime.clone(), 1, "1".into(), 0),
            get(runtime.clone(), 1, "1".into(), 0)
        );
        assert_eq!(
            a.unwrap().unwrap().text.as_deref(),
            Some("该喝水啦，休息一下吧。")
        );
        assert_eq!(
            b.unwrap().unwrap().text.as_deref(),
            Some("该喝水啦，休息一下吧。")
        );
        assert!(get(runtime.clone(), 1, "1".into(), 0)
            .await
            .unwrap()
            .unwrap()
            .text
            .is_some());
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        let requests = server.requests.lock().unwrap();
        let request = &requests[0];
        assert!(request.get("tools").is_none());
        assert!(request["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("一杯水"));
        drop(requests);
        runtime
            .read(|s, _| {
                assert_eq!(s.connection.total_changes(), changes);
                assert_eq!(
                    super::super::get_occurrence(&s.connection, 1)?.content,
                    "一杯水"
                );
                Ok(())
            })
            .unwrap();
    });
}
#[test]
fn invalid_generation_falls_back_once_without_poisoning_pet_health() {
    tauri::async_runtime::block_on(async {
        let server = Server::new(vec![Response::json(json!({"text":""}))]);
        let runtime = setup(&server.url);
        assert!(get(runtime.clone(), 1, "1".into(), 0)
            .await
            .unwrap()
            .unwrap()
            .text
            .is_none());
        assert!(get(runtime.clone(), 1, "1".into(), 0)
            .await
            .unwrap()
            .unwrap()
            .text
            .is_none());
        assert_eq!(server.requests.lock().unwrap().len(), 1);
        runtime
            .read(|s, _| {
                assert_eq!(
                    super::super::get_occurrence(&s.connection, 1)?.status,
                    ReminderStatus::Pending
                );
                Ok(())
            })
            .unwrap();
        assert!(runtime
            .read(|s, _| Ok(s.pet.provider_error.is_none()))
            .unwrap());
    });
}
#[test]
fn missing_provider_and_missed_or_completed_reminders_do_not_request_ai() {
    tauri::async_runtime::block_on(async {
        let runtime = setup("http://127.0.0.1:1/v1");
        runtime
            .read(|s, _| {
                s.connection.execute(
                    "UPDATE reminder_occurrences SET status='expired',delivery_state='missed'",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        assert!(get(runtime.clone(), 1, "1".into(), 0)
            .await
            .unwrap()
            .is_none());
        runtime
            .read(|s, _| {
                s.connection.execute(
                    "UPDATE reminder_occurrences SET status='pending',delivery_state='queued'",
                    [],
                )?;
                s.connection.execute("DELETE FROM provider_roles", [])?;
                s.connection.execute("DELETE FROM providers", [])?;
                Ok(())
            })
            .unwrap();
        assert!(get(runtime.clone(), 1, "1".into(), 0)
            .await
            .unwrap()
            .is_none());
    });
}
#[test]
fn late_results_are_discarded_after_record_or_context_changes() {
    tauri::async_runtime::block_on(async {
        for change in ["content", "done", "delete", "context", "provider"] {
            let mut response = Response::json(json!({"text":"旧润色"}));
            response.chunks[0].0 = 150;
            let server = Server::new(vec![response]);
            let runtime = setup(&server.url);
            let worker = runtime.clone();
            let task =
                tauri::async_runtime::spawn(async move { get(worker, 1, "1".into(), 0).await });
            for _ in 0..100 {
                if !server.requests.lock().unwrap().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            assert_eq!(server.requests.lock().unwrap().len(), 1);
            runtime
                .read(|s, _| {
                    match change {
                        "content" => {
                            s.connection
                                .execute("UPDATE reminders SET content='新详情'", [])?;
                        }
                        "done" => {
                            s.connection
                                .execute("UPDATE reminder_occurrences SET status='done'", [])?;
                        }
                        "delete" => {
                            s.connection
                                .execute("DELETE FROM reminders WHERE id=1", [])?;
                        }
                        "context" => {
                            s.context_revision += 1;
                        }
                        _ => {
                            s.connection
                                .execute("UPDATE provider_models SET model='new-model' WHERE purpose='chat'", [])?;
                        }
                    }
                    Ok(())
                })
                .unwrap();
            assert!(
                task.await.unwrap().unwrap().unwrap().text.is_none(),
                "{change}"
            );
        }
    });
}
