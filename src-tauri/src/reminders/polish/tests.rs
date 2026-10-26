use super::*;
#[test]
fn readers_only_return_persisted_text_without_generating(){
 tauri::async_runtime::block_on(async{
  let runtime=crate::runtime::test_runtime("http://127.0.0.1:1/v1");
  runtime.read(|s,_|{s.connection.execute_batch("INSERT INTO reminders(id,name,kind,content,status,source,created_at) VALUES(1,'喝水','once','一杯水','done','manual','2026-10-23T00:00:00Z');INSERT INTO reminder_occurrences(id,reminder_id,scheduled_at,status,delivery_state,created_at,presentation_state,presentation_text) VALUES(1,1,'2026-10-23T01:00:00Z','done','shown','2026-10-23T01:00:00Z','ready','喝口水吧。');")?;Ok(())}).unwrap();
  let before=runtime.read(|s,_|Ok(s.connection.total_changes())).unwrap();
  let(a,b)=tokio::join!(get(runtime.clone(),1,"1".into(),0),get(runtime.clone(),1,"1".into(),0));
  assert_eq!(a.unwrap().unwrap().text.as_deref(),Some("喝口水吧。"));assert_eq!(b.unwrap().unwrap().text.as_deref(),Some("喝口水吧。"));
  runtime.read(|s,_|{assert_eq!(s.connection.total_changes(),before);Ok(())}).unwrap();
 });
}
#[test]
fn wrong_context_does_not_disclose_a_late_result(){
 tauri::async_runtime::block_on(async{let runtime=crate::runtime::test_runtime("http://127.0.0.1:1/v1");assert!(get(runtime,1,"1".into(),9).await.is_err());});
}
