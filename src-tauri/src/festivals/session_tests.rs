use super::*;
use serde_json::json;
use crate::{ai::provider::ToolCall,state::RequestLease};
use std::sync::Arc;
fn day(year:i32,month:u32,day:u32)->NaiveDate {NaiveDate::from_ymd_opt(year,month,day).unwrap()}
fn now()->chrono::DateTime<chrono::Utc> {chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc)}
fn input()->session::Input {session::Input{kind:"birthday".into(),name:"我的生日".into(),calendar_type:"solar".into(),month:2,day:29,is_leap_month:false,blessing:"生日快乐".into(),note:None}}
fn setup()->(crate::runtime::Runtime,Arc<RequestLease>,tools::Tools) {
    let runtime=crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    let lease=Arc::new(runtime.requests.start("chat:1".into(),1).unwrap());
    let handler=tools::Tools::new(&runtime,&lease,1,1,0);(runtime,lease,handler)
}
fn call(id:&str,name:&str,args:serde_json::Value)->ToolCall {ToolCall{index:0,id:id.into(),name:name.into(),arguments:args}}
#[test]
fn crud_snapshot_conflict_and_failed_write_keep_original_record() {
    let (runtime,_lease,_handler)=setup();
    runtime.read(|s,_| {
        let record=session::create(&mut s.connection,1,input(),"user",day(2024,1,1),now())?;
        assert!(record.festival_id.is_none());assert_eq!(record.occurrences,vec![day(2024,2,29)]);
        let mut edit=input();edit.name="用户改过的名字".into();
        let changed=session::update(&mut s.connection,1,record.id.parse().unwrap(),&record.revision_token,edit.clone(),day(2024,1,1),now())?;
        assert_ne!(record.revision_token,changed.revision_token); // 同毫秒修改也能识别旧快照。
        assert_eq!(session::delete(&mut s.connection,1,record.id.parse().unwrap(),&record.revision_token,day(2024,1,1)).unwrap_err().code,"CONFLICT");
        assert_eq!(session::get(&s.connection,999,record.id.parse().unwrap(),day(2024,1,1)).unwrap_err().code,"NOT_FOUND");
        s.connection.execute_batch("CREATE TRIGGER reject_date BEFORE UPDATE ON session_festivals BEGIN SELECT RAISE(ABORT,'test failure'); END;")?;
        edit.name="不应保存".into();
        assert!(session::update(&mut s.connection,1,record.id.parse().unwrap(),&changed.revision_token,edit,day(2024,1,1),now()).is_err());
        s.connection.execute_batch("DROP TRIGGER reject_date")?;
        assert_eq!(session::get(&s.connection,1,record.id.parse().unwrap(),day(2024,1,1))?.name,"用户改过的名字");
        session::delete(&mut s.connection,1,record.id.parse().unwrap(),&changed.revision_token,day(2024,1,1))?;
        assert!(session::get(&s.connection,1,record.id.parse().unwrap(),day(2024,1,1)).is_err());Ok(())
    }).unwrap();
}
#[test]
fn twice_lunar_hits_and_year_updates_preserve_definition_and_text() {
    let (runtime,_lease,_handler)=setup();
    runtime.read(|s,_| {
        let mut lunar=input();lunar.calendar_type="lunar".into();lunar.month=11;lunar.day=20;
        let saved=session::create(&mut s.connection,1,lunar,"ai",day(2024,1,1),now())?;
        assert_eq!(saved.occurrences,vec![day(2024,1,1),day(2024,12,20)]);
        for date in [day(2024,1,1),day(2024,12,20)] {assert!(session::today(&s.connection,date,Some(1))?.iter().any(|r|r.scope=="session"&&r.id==saved.id));}
        assert!(!session::today(&s.connection,day(2024,12,20),Some(999))?.iter().any(|r|r.scope=="session"));
        synchronize(&mut s.connection,day(2101,1,1))?;
        let empty=session::get(&s.connection,1,saved.id.parse().unwrap(),day(2101,1,1))?;
        assert!(empty.solar_month.is_none()&&empty.occurrences.is_empty());
        assert_eq!((empty.lunar_month,empty.lunar_day),(Some(11),Some(20)));
        assert_eq!(empty.blessing,saved.blessing);assert_eq!(empty.source,"ai");
        synchronize(&mut s.connection,day(2024,1,1))?;
        assert_eq!(session::get(&s.connection,1,saved.id.parse().unwrap(),day(2024,1,1))?.occurrences.len(),2);
        Ok(())
    }).unwrap();
}
#[test]
fn tool_writes_are_idempotent_require_read_and_reject_cancelled_requests() {
    let (runtime,lease,mut handler)=setup();
    let write=call("create","create_session_festival",json!({"input":input()}));
    let result=handler.execute(&write).unwrap();let id=result["record"]["id"].as_str().unwrap();
    assert_eq!(handler.execute(&write).unwrap()["already_applied"],true);
    assert_eq!(handler.execute(&call("create","create_session_festival",json!({"input":{}}))).unwrap_err().code,"TOOL_CALL_CONFLICT");
    let mut other=tools::Tools::new(&runtime,&lease,1,1,0);
    assert_eq!(other.execute(&call("delete","delete_session_festival",json!({"festival_id":id}))).unwrap_err().code,"FESTIVAL_READ_REQUIRED");
    other.execute(&call("read","get_session_festival",json!({"festival_id":id}))).unwrap();
    let deleted=call("delete","delete_session_festival",json!({"festival_id":id}));
    other.execute(&deleted).unwrap();assert_eq!(other.execute(&deleted).unwrap()["already_applied"],true);
    lease.cancel.cancel();assert_eq!(handler.execute(&call("cancelled","create_session_festival",json!({"input":input()}))).unwrap_err().code,"CANCELLED");
    runtime.read(|s,_|{assert_eq!(s.connection.query_row("SELECT count(*) FROM session_festivals",[],|r|r.get::<_,i64>(0))?,0);Ok(())}).unwrap();
}
#[test]
fn invalid_and_cross_session_tool_arguments_do_not_save() {
    let (runtime,_lease,mut handler)=setup();let mut invalid=input();invalid.blessing="  ".into();
    assert!(handler.execute(&call("bad","create_session_festival",json!({"input":invalid}))).is_err());
    assert!(handler.execute(&call("scope","list_session_festivals",json!({"session_id":"2"}))).is_err());
    assert!(handler.execute(&call("parent","create_session_festival",json!({"input":input(),"festival_id":"1"}))).is_err());
    runtime.read(|s,_|{s.context_revision+=1;Ok(())}).unwrap();
    assert_eq!(handler.execute(&call("stale","create_session_festival",json!({"input":input()}))).unwrap_err().code,"CONTEXT_CHANGED");
}
