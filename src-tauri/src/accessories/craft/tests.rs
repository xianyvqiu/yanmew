use super::*;
use std::{path::PathBuf,sync::atomic::{AtomicU64,Ordering}};
struct Events;
impl crate::events::ReminderEventSink for Events {fn reminders_changed(&self){}fn reminder_failed(&self,_:&AppError){}}
impl ChatToolEventSink for Events {fn notebook_changed(&self){}}
static NEXT:AtomicU64=AtomicU64::new(0);
struct Fixture {runtime:Runtime,lease:Arc<RequestLease>,root:PathBuf,context:Context}
impl Fixture {
    fn new()->Self {
        let mut runtime=crate::runtime::test_runtime("http://localhost");
        let root=std::env::temp_dir().join(format!("yanmew-craft-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir_all(&root).unwrap();runtime.paths=Ok(crate::platform::contracts::AppPaths {data_root:root.clone()});
        runtime.read(|s,_| {s.connection.execute("UPDATE sessions SET intimacy=300 WHERE id=1",[])?;s.connection.execute("UPDATE user_state SET coins=2000",[])?;Ok(())}).unwrap();
        let lease=Arc::new(runtime.requests.start("chat:1".into(),1).unwrap());
        Self {runtime,lease,root,context:Context {persona_id:"1".into(),session_id:"1".into(),context_revision:0}}
    }
    fn request(&self,id:&str)->Receipt {request(&self.runtime,&self.lease,self.context.clone(),&call(id,"🎀"),Arc::new(Events)).unwrap()}
    fn coins(&self)->i64 {self.runtime.read(|s,_|currency::get_balance(&s.connection)).unwrap()}
    fn image(&self,id:&str)->String {
        self.runtime.read(|s,_| {let c=s.accessory_crafts.candidate.as_mut().unwrap();c.status=Status::Preparing;c.image_revision=Some(0);Ok(())}).unwrap();
        let mut rgba=image::RgbaImage::new(8,8);for y in 2..6 {for x in 2..6 {rgba.put_pixel(x,y,image::Rgba([240,30,60,255]));}}
        let mut bytes=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(rgba).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
        let png=crate::accessories::images::normalize(&bytes.into_inner()).unwrap();
        let candidate=finish_image(&self.runtime,id,Ok(png)).unwrap().unwrap();candidate.image_file.unwrap()
    }
}
impl Drop for Fixture {fn drop(&mut self){let _=self.runtime.read(|s,_|{s.accessory_crafts.discard();Ok(())});let _=std::fs::remove_dir_all(&self.root);}}
fn call(id:&str,display:&str)->ToolCall {ToolCall {index:0,id:id.into(),name:"request_accessory_craft".into(),arguments:json!({"name":"小蝴蝶结","display_text":display,"description":"想戴上这个小蝴蝶结陪你聊天。"})}}
#[test]
fn symbols_are_real_visuals_and_same_tool_call_does_not_repeat() {
    for display in ["🎀","👑","✿","✧･ﾟ","( • ω • )",":)"] {validate_text(&Text {name:"饰品".into(),display_text:display.into(),description:"我的饰品".into()}).unwrap();}
    for display in ["蝴蝶结","皇冠!","crown","普通说明",""] {assert!(validate_text(&Text {name:"饰品".into(),display_text:display.into(),description:"我的饰品".into()}).is_err());}
    let f=Fixture::new();let first=f.request("one");assert_eq!(first.status,Status::Ready);let repeat=f.request("one");assert_eq!(first.preview_id,repeat.preview_id);
    assert_eq!(request(&f.runtime,&f.lease,f.context.clone(),&call("one","★"),Arc::new(Events)).unwrap_err().code,"TOOL_CALL_CONFLICT");
    assert_eq!(request(&f.runtime,&f.lease,f.context.clone(),&call("two","🎀"),Arc::new(Events)).unwrap_err().code,"REQUEST_BUSY");assert_eq!(f.coins(),2000);
    let preview=get(&f.runtime,&f.context,&first.preview_id).unwrap();assert_eq!(preview.appearance,Appearance::Text);assert_eq!(preview.display_text,"🎀");assert_eq!(preview.cost,"20");
}
#[test]
fn uses_lowest_unused_tier_once_then_repeats_fourth_without_auto_wearing() {
    let f=Fixture::new();
    for (i,tier) in [1,2,3,4,4].iter().enumerate() {
        let receipt=f.request(&format!("call-{i}"));let p=get(&f.runtime,&f.context,&receipt.preview_id).unwrap();assert_eq!(p.tier,*tier);
        let item=confirm(&f.runtime,&f.context,&receipt.preview_id).unwrap();assert!(!item.is_worn);
        assert_eq!(confirm(&f.runtime,&f.context,&receipt.preview_id).unwrap_err().code,"ACCESSORY_PREVIEW_EXPIRED");
    }
    assert_eq!(f.coins(),1210);
    f.runtime.read(|s,_| {let flags:(bool,bool,bool)=s.connection.query_row("SELECT craft_tier_1_used,craft_tier_2_used,craft_tier_3_used FROM sessions WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;assert_eq!(flags,(true,true,true));assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM currency_ledger WHERE reason='accessory_craft'",[],|r|r.get::<_,i64>(0))?,5);Ok(())}).unwrap();
}
#[test]
fn rejection_cooldown_not_persistent_and_unqualified_or_changed_context_cannot_craft() {
    let f=Fixture::new();let first=f.request("one");cancel(&f.runtime,&f.context,&first.preview_id).unwrap();
    assert_eq!(request(&f.runtime,&f.lease,f.context.clone(),&call("two","🎀"),Arc::new(Events)).unwrap_err().code,"ACCESSORY_CRAFT_COOLDOWN");
    f.runtime.read(|s,_| {s.accessory_crafts.refused_until.insert(1,Instant::now()-Duration::from_secs(1));s.connection.execute("UPDATE sessions SET intimacy=9 WHERE id=1",[])?;Ok(())}).unwrap();
    assert_eq!(request(&f.runtime,&f.lease,f.context.clone(),&call("three","🎀"),Arc::new(Events)).unwrap_err().code,"ACCESSORY_NOT_UNLOCKED");
    f.runtime.read(|s,_|{s.context_revision+=1;s.accessory_crafts.discard();Ok(())}).unwrap();
    assert_eq!(request(&f.runtime,&f.lease,f.context.clone(),&call("four","🎀"),Arc::new(Events)).unwrap_err().code,"CONTEXT_CHANGED");
    assert_eq!(State::default().refused_until.len(),0);assert_eq!(f.coins(),2000);
}
#[test]
fn transaction_failure_preserves_preview_file_balance_and_tier() {
    let f=Fixture::new();let receipt=f.request("one");let file=f.image(&receipt.preview_id);
    crate::accessories::images::cleanup_unused(&f.runtime,&[]);assert!(f.root.join(&file).exists());
    f.runtime.read(|s,_|{s.connection.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON currency_ledger BEGIN SELECT RAISE(ABORT,'test'); END;")?;Ok(())}).unwrap();
    assert!(confirm(&f.runtime,&f.context,&receipt.preview_id).is_err());assert_eq!(f.coins(),2000);assert!(f.root.join(&file).exists());
    f.runtime.read(|s,_|{assert_eq!(s.connection.query_row("SELECT COUNT(*) FROM accessories",[],|r|r.get::<_,i64>(0))?,0);assert_eq!(quote(&s.connection,1)?.unwrap().tier,1);s.connection.execute_batch("DROP TRIGGER fail;")?;Ok(())}).unwrap();
    let item=confirm(&f.runtime,&f.context,&receipt.preview_id).unwrap();assert_eq!(item.image_file,Some(file));assert_eq!(f.coins(),1980);
    crate::accessories::images::cleanup_unused(&f.runtime,&[]);assert!(f.root.join(item.image_file.unwrap()).exists());
}
#[test]
fn missing_image_requires_another_confirmation_even_after_preview_poll() {
    for poll in [false,true] {
        let f=Fixture::new();let receipt=f.request("one");let file=f.image(&receipt.preview_id);std::fs::remove_file(f.root.join(file)).unwrap();
        if poll {assert_eq!(get(&f.runtime,&f.context,&receipt.preview_id).unwrap().appearance,Appearance::Text);}
        assert_eq!(confirm(&f.runtime,&f.context,&receipt.preview_id).unwrap_err().code,"ACCESSORY_IMAGE_UNAVAILABLE");assert_eq!(f.coins(),2000);
        let view=get(&f.runtime,&f.context,&receipt.preview_id).unwrap();assert_eq!(view.display_text,"🎀");assert!(view.image_data_url.is_none());
        let item=confirm(&f.runtime,&f.context,&receipt.preview_id).unwrap();assert!(item.image_file.is_none());assert_eq!(f.coins(),1980);
    }
}
#[test]
fn configuration_change_and_cancel_reject_late_image_without_consuming_tier() {
    let f=Fixture::new();let receipt=f.request("one");
    f.runtime.read(|s,_| {let c=s.accessory_crafts.candidate.as_mut().unwrap();c.status=Status::Preparing;c.image_revision=Some(0);assert!(configuration_changed(s).is_some());Ok(())}).unwrap();
    assert!(finish_image(&f.runtime,&receipt.preview_id,Err(AppError::new("CANCELLED","取消"))).unwrap().is_none());
    let p=get(&f.runtime,&f.context,&receipt.preview_id).unwrap();assert_eq!(p.status,Status::Ready);assert_eq!(p.appearance,Appearance::Text);assert_eq!(f.coins(),2000);
    cancel(&f.runtime,&f.context,&receipt.preview_id).unwrap();assert!(finish_image(&f.runtime,&receipt.preview_id,Ok(vec![])).unwrap().is_none());
    assert!(current(&f.runtime,&f.context).unwrap().is_none());
}
#[test]
fn price_change_expires_quote_and_insufficient_wallet_does_not_consume_tier() {
    let f=Fixture::new();let receipt=f.request("one");
    f.runtime.read(|s,_|{crate::config::save(&mut s.connection,std::collections::BTreeMap::from([("accessory_cost_1".into(),"21".into())]))?;Ok(())}).unwrap();
    assert_eq!(confirm(&f.runtime,&f.context,&receipt.preview_id).unwrap_err().code,"ACCESSORY_PREVIEW_EXPIRED");assert!(current(&f.runtime,&f.context).unwrap().is_none());
    let next=f.request("two");
    f.runtime.read(|s,_|{s.connection.execute("UPDATE user_state SET coins=0",[])?;Ok(())}).unwrap();
    assert_eq!(confirm(&f.runtime,&f.context,&next.preview_id).unwrap_err().code,"INSUFFICIENT_COINS");
    assert_eq!(get(&f.runtime,&f.context,&next.preview_id).unwrap().cost,"21");
    f.runtime.read(|s,_|{assert_eq!(quote(&s.connection,1)?.unwrap().tier,1);Ok(())}).unwrap();
}
