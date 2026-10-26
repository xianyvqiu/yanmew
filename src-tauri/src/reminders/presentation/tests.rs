use super::*;
use crate::ai::tests::{Response,Server};
use chrono::{TimeZone,Utc};
use serde_json::json;

fn setup(url:&str)->Runtime{
    let runtime=crate::runtime::test_runtime(url);
    runtime.read(|s,_|{
        s.connection.execute_batch("INSERT INTO reminders(id,name,kind,content,status,fire_at,source,created_at) VALUES(1,'喝水','once','一杯水','pending','2026-10-23T01:00:00Z','manual','2026-10-23T00:00:00Z');")?;
        super::super::dispatch(&mut s.connection,Utc.with_ymd_and_hms(2026,10,23,1,0,0).unwrap(),&Utc,false)?;Ok(())
    }).unwrap();runtime
}
#[test]
fn online_trigger_is_done_and_persists_one_shared_text_and_message(){
    tauri::async_runtime::block_on(async{
        let server=Server::new(vec![Response::json(json!({"text":"喝口水，休息一下吧。"}))]);let runtime=setup(&server.url);
        runtime.read(|s,_|{let row=super::super::get_occurrence(&s.connection,1)?;assert_eq!(row.status,crate::models::ReminderStatus::Done);assert!(row.auto_completed);assert!(row.completed_at.is_some());Ok(())}).unwrap();
        assert!(process_one(runtime.clone()).await.unwrap().0);
        assert!(!process_one(runtime.clone()).await.unwrap().0);
        runtime.read(|s,_|{
            let row:(String,String,i64)=s.connection.query_row("SELECT presentation_state,presentation_text,presentation_message_id FROM reminder_occurrences WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            assert_eq!((row.0.as_str(),row.1.as_str()),("ready","喝口水，休息一下吧。"));
            let message:(String,Option<i64>)=s.connection.query_row("SELECT content,round_id FROM messages WHERE id=?1",[row.2],|r|Ok((r.get(0)?,r.get(1)?)))?;assert_eq!(message,(row.1,None));
            assert!(finish(&mut s.connection,1,Some("重复"),"2026-10-23T02:00:00Z")?.is_none());
            assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM messages WHERE round_id IS NULL",[],|r|r.get::<_,i64>(0))?,1);Ok(())
        }).unwrap();assert_eq!(server.requests.lock().unwrap().len(),1);
    });
}
#[test]
fn switching_session_does_not_redirect_the_triggered_message(){
    let runtime=setup("http://127.0.0.1:1/v1");
    runtime.read(|s,_|{
        let original:i64=s.connection.query_row("SELECT presentation_session_id FROM reminder_occurrences WHERE id=1",[],|r|r.get(0))?;
        s.connection.execute("INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(1,'另一会话','2026-10-23T00:00:00Z','2026-10-23T00:00:00Z')",[])?;let other=s.connection.last_insert_rowid();
        s.connection.execute("UPDATE settings SET value=?1 WHERE key='current_session_id'",[other.to_string()])?;
        finish(&mut s.connection,1,Some("该喝水啦。"),"2026-10-23T01:01:00Z")?;
        assert_eq!(s.connection.query_row("SELECT session_id FROM messages WHERE id=(SELECT presentation_message_id FROM reminder_occurrences WHERE id=1)",[],|r|r.get::<_,i64>(0))?,original);
        assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM messages WHERE session_id=?1",[other],|r|r.get::<_,i64>(0))?,0);Ok(())
    }).unwrap();
}
#[test]
fn interrupted_generation_recovers_without_requesting_ai_or_duplicating_chat(){
    tauri::async_runtime::block_on(async{
        let server=Server::new(vec![]);let runtime=setup(&server.url);
        let task=prepare(&runtime).unwrap().unwrap();assert_eq!(task.id,1);
        assert!(process_one(runtime.clone()).await.unwrap().0);
        assert!(!process_one(runtime.clone()).await.unwrap().0);
        runtime.read(|s,_|{
            let text:String=s.connection.query_row("SELECT presentation_text FROM reminder_occurrences WHERE id=1",[],|r|r.get(0))?;assert_eq!(text,"喝水事件提醒：一杯水");
            assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM messages WHERE round_id IS NULL",[],|r|r.get::<_,i64>(0))?,1);Ok(())
        }).unwrap();assert!(server.requests.lock().unwrap().is_empty());
    });
}
#[test]
fn deleted_target_session_is_not_recreated_or_redirected(){
    let runtime=setup("http://127.0.0.1:1/v1");runtime.read(|s,_|{
        s.connection.execute("DELETE FROM sessions WHERE id=(SELECT presentation_session_id FROM reminder_occurrences WHERE id=1)",[])?;
        assert!(finish(&mut s.connection,1,Some("该喝水啦。"),"2026-10-23T01:01:00Z")?.is_none());
        let row:(String,Option<i64>)=s.connection.query_row("SELECT presentation_state,presentation_message_id FROM reminder_occurrences WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?)))?;assert_eq!(row,("ready".into(),None));Ok(())
    }).unwrap();
}
