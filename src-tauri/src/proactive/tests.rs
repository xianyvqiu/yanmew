use super::*;
use crate::{pet::{controller::{Action, InteractionInput}, RandomSource}, platform::contracts::{Clock,ClockSample}};
use std::time::Duration;
struct FixedClock;
impl Clock for FixedClock {
    fn sample(&self) -> AppResult<ClockSample> {
        Ok(ClockSample {utc:DateTime::parse_from_rfc3339("2026-10-06T10:00:00Z").unwrap().with_timezone(&Utc), running:Duration::ZERO})
    }
}
struct FixedRandom(u64);
impl RandomSource for FixedRandom { fn below(&mut self, upper:u64)->AppResult<u64>{Ok(self.0%upper)} }
fn ready(url:&str, random:u64) -> Runtime {
    let mut runtime=crate::runtime::test_runtime(url);
    runtime.clock=Arc::new(FixedClock);
    runtime.random=Arc::new(std::sync::Mutex::new(Box::new(FixedRandom(random))));
    runtime.read(|s,_| {s.connection.execute("UPDATE session_pet_states SET idle_branch='bored',idle_elapsed_ms=3000000,proactive_next_check_idle_ms=3000000 WHERE session_id=1",[])?;Ok(())}).unwrap();
    runtime
}
fn snapshot(runtime:&Runtime)->pet_store::PetSnapshot {runtime.read(|s,_|pet_store::read(&s.connection,1)).unwrap()}

#[test]
fn schedule_uses_saved_idle_and_miss_or_drop_waits_for_next_check() {
    let runtime=ready("http://127.0.0.1:9/v1",30);
    assert!(prepare(&runtime).unwrap().is_none());
    assert_eq!(snapshot(&runtime).proactive_next_check_idle_ms,Some(3300000));
    assert!(prepare(&runtime).unwrap().is_none());
    let runtime=ready("http://127.0.0.1:9/v1",29);
    let work=prepare(&runtime).unwrap().unwrap();
    assert!(prepare(&runtime).unwrap().is_none());
    drop(work); // 等价于未成功保存就结束任务，保留重试位置。
    assert!(prepare(&runtime).unwrap().is_none());
    assert!(snapshot(&runtime).last_proactive_sent_at.is_none());
}

#[test]
fn success_is_atomic_and_deleting_message_does_not_reopen_cycle() {
    let runtime=ready("http://127.0.0.1:9/v1",0);
    let work=prepare(&runtime).unwrap().unwrap();
    runtime.read(|s,_| {s.connection.execute_batch("CREATE TRIGGER fail_proactive BEFORE UPDATE OF last_proactive_sent_at ON session_pet_states BEGIN SELECT RAISE(ABORT,'forced failure'); END;")?;Ok(())}).unwrap();
    assert!(save(&runtime,&work,"主动内容",None).is_err());
    runtime.read(|s,_| {assert_eq!(s.connection.query_row("SELECT count(*) FROM messages",[],|r|r.get::<_,i64>(0))?,0);s.connection.execute_batch("DROP TRIGGER fail_proactive")?;Ok(())}).unwrap();
    let notice=save(&runtime,&work,"主动内容",Some("(^_^)" )).unwrap();
    assert_eq!(notice.message.round_id,None);
    assert!(snapshot(&runtime).proactive_next_check_idle_ms.is_none());
    assert!(snapshot(&runtime).last_proactive_sent_at.is_some());
    assert!(save(&runtime,&work,"重复",None).is_err());
    drop(work);
    runtime.read(|s,_| {
        assert_eq!(s.connection.query_row("SELECT count(*) FROM chat_rounds",[],|r|r.get::<_,i64>(0))?,0);
        assert_eq!(s.connection.query_row("SELECT intimacy FROM sessions WHERE id=1",[],|r|r.get::<_,i64>(0))?,0);
        s.connection.execute("DELETE FROM messages",[])?;Ok(())
    }).unwrap();
    assert!(prepare(&runtime).unwrap().is_none());
}

#[test]
fn cooldown_blocks_new_cycle_but_hungry_display_does_not() {
    let runtime=ready("http://127.0.0.1:9/v1",0);
    runtime.read(|s,_| {s.connection.execute("UPDATE session_pet_states SET last_proactive_sent_at='2026-10-06T09:31:00Z'",[])?;Ok(())}).unwrap();
    assert!(prepare(&runtime).unwrap().is_none());
    runtime.read(|s,_| {s.connection.execute("UPDATE session_pet_states SET idle_elapsed_ms=3300000,hunger_elapsed_ms=10000000,last_proactive_sent_at='2026-10-06T09:30:00Z'",[])?;Ok(())}).unwrap();
    assert!(prepare(&runtime).unwrap().is_some());
}

#[test]
fn interaction_and_user_chat_cancel_without_saving_proactive() {
    for chat in [false,true] {
        let runtime=ready("http://127.0.0.1:9/v1",0);
        let work=prepare(&runtime).unwrap().unwrap();
        if chat { let _user=crate::chat::prepare_reply(&runtime,1,1,0,"hi".into()).unwrap(); }
        else {runtime.pet_interact(InteractionInput{persona_id:"1".into(),session_id:"1".into(),context_revision:0,action:Action::Click}).unwrap();}
        assert!(work.lease.cancel.check().is_err());
        assert!(save(&runtime,&work,"迟到",None).is_err());
        assert_eq!(snapshot(&runtime).idle_elapsed_ms,0);
    }
}

#[test]
fn hidden_and_context_changes_reject_late_results() {
    for hidden in [true,false] {
        let runtime=ready("http://127.0.0.1:9/v1",0);
        let work=prepare(&runtime).unwrap().unwrap();
        if hidden {runtime.pet_visibility(Some(true),None).unwrap();runtime.pet_visibility(Some(false),None).unwrap();}
        else {runtime.read(|s,_| {s.context_revision+=1;Ok(())}).unwrap();}
        assert!(save(&runtime,&work,"旧结果",None).is_err());
        assert!(snapshot(&runtime).last_proactive_sent_at.is_none());
    }
}

#[test]
fn proactive_reuses_expression_context_and_tools_without_fabricated_user() {
    tauri::async_runtime::block_on(async {
        use crate::ai::tests::{Server,Response,text_event,finish_event};
        let server=Server::new(vec![Response::json(serde_json::json!({"expression":"(^_^)"})),Response::sse(&(text_event("一起聊聊吧")+&finish_event("stop")))]);
        let runtime=ready(&server.url,0);
        let work=prepare(&runtime).unwrap().unwrap();
        let choice=crate::chat::run_reply(runtime.ai.as_ref().unwrap(),&work.prepared,|_|Ok(())).await.unwrap();
        assert_eq!(choice.content,"一起聊聊吧");
        assert_eq!(snapshot(&runtime).idle_elapsed_ms,3000000);
        let requests=server.requests.lock().unwrap();
        let messages=requests[1]["messages"].as_array().unwrap();
        assert!(messages.iter().all(|m|m["role"]!="user"));
        assert!(requests[1]["tools"].as_array().unwrap().iter().any(|t|t["function"]["name"]=="read_notebook"));
        drop(requests);
        save(&runtime,&work,&choice.content,Some("(^_^)" )).unwrap();
        server.finish();
    });
}

#[test]
fn tools_reject_other_tasks_and_other_session_leases() {
    let requests=crate::state::RequestManager::default();
    let valid=requests.start("proactive:1".into(),1).unwrap();
    assert!(valid.require_conversation_scope(&requests,1,1).is_ok());
    assert!(valid.require_conversation_scope(&requests,2,1).is_err());
    let invalid=requests.start("short-memory:1".into(),1).unwrap();
    assert!(invalid.require_conversation_scope(&requests,1,1).is_err());
}
