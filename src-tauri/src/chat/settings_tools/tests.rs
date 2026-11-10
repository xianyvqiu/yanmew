use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};
#[derive(Default)]
struct Events {config:AtomicUsize,wallet:AtomicUsize}
impl crate::events::ReminderEventSink for Events {fn reminders_changed(&self){}fn reminder_failed(&self,_:&AppError){}}
impl ChatToolEventSink for Events {
    fn notebook_changed(&self){}
    fn configuration_changed(&self){self.config.fetch_add(1,Ordering::SeqCst);}
    fn wallet_changed(&self){self.wallet.fetch_add(1,Ordering::SeqCst);}
}
fn setup()->(Runtime,Tools,Arc<Events>){
    let runtime=crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    let lease=Arc::new(runtime.requests.start("chat:1".into(),1).unwrap());
    let events=Arc::new(Events::default());
    let tools=Tools::new(&runtime,&lease,1,1,0,None,false,events.clone());
    (runtime,tools,events)
}
fn call(id:&str,name:&str,args:Value)->ToolCall{ToolCall{index:0,id:id.into(),name:name.into(),arguments:args}}
fn read(tools:&mut Tools,name:&str)->Value{tools.execute(&call(name,name,json!({}))).unwrap()}
fn theme(runtime:&Runtime)->String{runtime.read(|s,_|Ok(crate::config::get(&s.connection)?["theme_mode"].clone())).unwrap()}
#[test]fn theme_updates_are_narrow_confirmed_and_idempotent(){
    let (runtime,mut tools,events)=setup();assert_eq!(read(&mut tools,"get_ui_theme")["mode"],"system");
    for args in [json!({"mode":"dark","confirmedByUser":false}),json!({"mode":"invalid","confirmedByUser":true}),json!({"mode":"dark","confirmedByUser":true,"tool_call_rounds":99})] {assert!(tools.execute(&call(&args.to_string(),"set_ui_theme",args)).is_err());}
    assert_eq!(theme(&runtime),"system");
    let change=call("set-theme","set_ui_theme",json!({"mode":"dark","confirmedByUser":true}));
    assert_eq!(tools.execute(&change).unwrap()["changed"],true);assert_eq!(theme(&runtime),"dark");assert_eq!(events.config.load(Ordering::SeqCst),1);
    assert_eq!(tools.execute(&change).unwrap()["alreadyApplied"],true);assert_eq!(events.config.load(Ordering::SeqCst),1);
    assert_eq!(tools.execute(&call("set-theme","set_ui_theme",json!({"mode":"light","confirmedByUser":true}))).unwrap_err().code,"TOOL_CALL_CONFLICT");
}
#[test]fn sign_in_reads_without_awarding_and_both_duplicate_ids_and_new_ids_cannot_award_again(){
    let (runtime,mut tools,events)=setup();let before=read(&mut tools,"get_daily_sign_in_status");assert_eq!(before["signedIn"],false);
    assert!(tools.execute(&call("no-confirm","daily_sign_in",json!({"confirmedByUser":false}))).is_err());
    assert!(tools.execute(&call("forged-reward","daily_sign_in",json!({"confirmedByUser":true,"coins":1000,"date":"tomorrow"}))).is_err());
    let request=call("sign","daily_sign_in",json!({"confirmedByUser":true}));let first=tools.execute(&request).unwrap();
    assert_eq!(first["newlySignedIn"],true);assert_eq!(first["status"]["signedIn"],true);
    let reward=first["grantedCoins"].as_str().unwrap().parse::<i64>().unwrap();
    assert_eq!(first["status"]["balance"].as_str().unwrap().parse::<i64>().unwrap(),before["balance"].as_str().unwrap().parse::<i64>().unwrap()+reward);
    for repeat in [request,call("different-id","daily_sign_in",json!({"confirmedByUser":true}))] {let result=tools.execute(&repeat).unwrap();assert_eq!(result["newlySignedIn"],false);assert!(result["grantedCoins"].is_null());}
    assert_eq!(read(&mut tools,"get_daily_sign_in_status")["signedIn"],true);assert_eq!(events.wallet.load(Ordering::SeqCst),1);
    runtime.read(|s,_|{assert_eq!(s.connection.query_row("SELECT count(*) FROM currency_ledger WHERE reason IN ('daily_sign_in','sign_in_festival')",[],|r|r.get::<_,i64>(0))?,1);Ok(())}).unwrap();
}
#[test]fn manual_sign_in_and_failed_transaction_use_the_same_daily_guard(){
    let (runtime,mut tools,events)=setup();
    runtime.read(|s,_|{let now=runtime.clock.sample()?.utc;let mut random=runtime.random.lock().unwrap();crate::currency::daily_sign_in(&mut s.connection,now,&chrono::Local,random.as_mut())?;Ok(())}).unwrap();
    let result=tools.execute(&call("signed-in-ui","daily_sign_in",json!({"confirmedByUser":true}))).unwrap();assert!(result["grantedCoins"].is_null());assert_eq!(events.wallet.load(Ordering::SeqCst),0);
    let (runtime,mut tools,events)=setup();let before=read(&mut tools,"get_daily_sign_in_status")["balance"].clone();
    runtime.read(|s,_|{s.connection.execute_batch("CREATE TRIGGER refuse_tool_sign_in BEFORE INSERT ON currency_ledger BEGIN SELECT RAISE(ABORT,'test failure'); END;")?;Ok(())}).unwrap();
    assert!(tools.execute(&call("failed","daily_sign_in",json!({"confirmedByUser":true}))).is_err());
    let after=read(&mut tools,"get_daily_sign_in_status");assert_eq!(after["signedIn"],false);assert_eq!(after["balance"],before);assert_eq!(events.wallet.load(Ordering::SeqCst),0);
}
#[test]fn appearance_preserves_omitted_fields_and_null_resets_only_the_requested_color(){
    let (runtime,mut tools,events)=setup();
    assert_eq!(tools.execute(&call("unread","update_chat_appearance",json!({"opacity":40,"confirmedByUser":true}))).unwrap_err().code,"APPEARANCE_NOT_READ");
    assert_eq!(read(&mut tools,"get_chat_appearance")["scope"],"persona");
    let initial=call("appearance","update_chat_appearance",json!({"opacity":40,"bubbleStyle":"frosted","userTextColor":"#112233","assistantTextColor":"#445566","confirmedByUser":true}));
    tools.execute(&initial).unwrap();assert_eq!(tools.execute(&initial).unwrap()["alreadyApplied"],true);
    tools.execute(&call("reset-color","update_chat_appearance",json!({"assistantTextColor":null,"confirmedByUser":true}))).unwrap();
    let value=runtime.read(|s,_|background::snapshot(s,1,Some(0))).unwrap();assert_eq!(value.opacity,40);assert_eq!(value.appearance.bubble_style,"frosted");assert_eq!(value.appearance.user_text_color.as_deref(),Some("#112233"));assert!(value.appearance.assistant_text_color.is_none());assert_eq!(events.config.load(Ordering::SeqCst),2);
    assert!(tools.execute(&call("bad-color","update_chat_appearance",json!({"userTextColor":"red","confirmedByUser":true}))).is_err());
    assert!(tools.execute(&call("file-path","update_chat_appearance",json!({"imagePath":"C:/example.png","confirmedByUser":true}))).is_err());
}
#[test]fn conflicts_cancellation_wrong_context_and_regeneration_do_not_write(){
    let (runtime,mut tools,_)=setup();read(&mut tools,"get_chat_appearance");
    runtime.read(|s,_|{s.connection.execute("UPDATE personas SET chat_bg_opacity=60 WHERE id=1",[])?;Ok(())}).unwrap();
    assert_eq!(tools.execute(&call("stale","update_chat_appearance",json!({"opacity":20,"confirmedByUser":true}))).unwrap_err().code,"CONFLICT");
    assert_eq!(runtime.read(|s,_|background::snapshot(s,1,Some(0))).unwrap().opacity,60);
    tools.lease.cancel.cancel();assert!(tools.execute(&call("cancelled","set_ui_theme",json!({"mode":"dark","confirmedByUser":true}))).is_err());assert_eq!(theme(&runtime),"system");
    let (runtime,mut tools,_)=setup();tools.read_only=true;read(&mut tools,"get_ui_theme");read(&mut tools,"get_daily_sign_in_status");read(&mut tools,"get_chat_appearance");
    for (name,args) in [("set_ui_theme",json!({"mode":"dark","confirmedByUser":true})),("daily_sign_in",json!({"confirmedByUser":true})),("update_chat_appearance",json!({"opacity":20,"confirmedByUser":true}))] {assert_eq!(tools.execute(&call(name,name,args)).unwrap_err().code,"PERMISSION_DENIED");}
    tools.read_only=false;runtime.read(|s,_|{s.context_revision+=1;Ok(())}).unwrap();assert!(tools.execute(&call("wrong-context","daily_sign_in",json!({"confirmedByUser":true}))).is_err());
}
struct Folder(std::path::PathBuf);
impl Folder {fn new()->Self{let path=std::env::temp_dir().join(format!("yanmew-settings-tools-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::create_dir(&path).unwrap();Self(path)}}
impl Drop for Folder {fn drop(&mut self){let path=self.0.canonicalize().unwrap();assert_eq!(path.parent().unwrap(),std::env::temp_dir().canonicalize().unwrap());assert!(path.file_name().unwrap().to_string_lossy().starts_with("yanmew-settings-tools-"));std::fs::remove_dir_all(path).unwrap();}}
fn png()->Vec<u8>{let mut out=std::io::Cursor::new(vec![]);image::DynamicImage::new_rgb8(2,2).write_to(&mut out,image::ImageFormat::Png).unwrap();out.into_inner()}
#[test]fn uploaded_image_is_pinned_to_this_user_message_and_background_is_an_independent_copy(){
    let folder=Folder::new();let mut runtime=crate::runtime::test_runtime("http://127.0.0.1:9/v1");runtime.paths=Ok(crate::platform::contracts::AppPaths::portable(folder.0.clone()));
    let (prepared,receipt)=super::super::submit_images(&runtime,1,1,0,"把这张图设为背景".into(),vec![crate::chat_images::Input{filename:"example.png".into(),data_url:format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(png()))}]).unwrap();
    let events=Arc::new(Events::default());let mut tools=Tools::new(&runtime,&prepared.lease,1,1,0,prepared.image_message,false,events.clone());
    assert_eq!(read(&mut tools,"get_chat_appearance")["uploadedImages"][0]["index"],1);
    assert!(tools.execute(&call("bad-index","update_chat_appearance",json!({"backgroundImageIndex":2,"confirmedByUser":true}))).is_err());
    let value=tools.execute(&call("set-image","update_chat_appearance",json!({"backgroundImageIndex":1,"opacity":35,"confirmedByUser":true}))).unwrap();assert_eq!(value["hasBackground"],true);
    let original=&receipt.round.user_message.images[0].image_file;let source=crate::chat_images::managed_file(runtime.paths.as_ref().unwrap(),original).unwrap();std::fs::remove_file(source).unwrap();
    let view=background::get(&runtime,1,Some(0)).unwrap();assert!(view.image_data_url.is_some());assert_eq!(view.record.opacity,35);
    tools.execute(&call("clear","update_chat_appearance",json!({"clearBackground":true,"confirmedByUser":true}))).unwrap();assert!(!read(&mut tools,"get_chat_appearance")["hasBackground"].as_bool().unwrap());
    tools.image_message=Some(receipt.round.assistant_message.as_ref().map(|m|m.id.parse().unwrap()).unwrap_or(i64::MAX));assert!(tools.execute(&call("foreign-images","get_chat_appearance",json!({}))).is_err());
}
#[test]fn final_request_guard_rejects_background_commit_and_cleans_new_file(){
    let folder=Folder::new();let (mut runtime,_,_)=setup();runtime.paths=Ok(crate::platform::contracts::AppPaths::portable(folder.0.clone()));
    let expected=runtime.read(|s,_|background::snapshot(s,1,Some(0))).unwrap();let checks=AtomicUsize::new(0);
    let result=background::save_guarded(&runtime,1,Some(0),background::Input{expected:expected.clone(),opacity:50,blur:0,image:Some(background::Upload{file_name:"test.png".into(),base64:base64::engine::general_purpose::STANDARD.encode(png())}),reset:false,appearance:None},|_|if checks.fetch_add(1,Ordering::SeqCst)==0{Ok(())}else{Err(AppError::new("CANCELLED","test cancellation"))});
    assert!(result.is_err());assert_eq!(runtime.read(|s,_|background::snapshot(s,1,Some(0))).unwrap(),expected);
    let root=folder.0.join("chat_background");assert!(root.is_dir());assert_eq!(std::fs::read_dir(root).unwrap().count(),0);
}
#[test]fn schema_exposes_only_six_specific_tools_with_strict_arguments(){
    let definitions=definitions();assert_eq!(definitions.len(),6);
    for tool in definitions{let name=tool["function"]["name"].as_str().unwrap();assert_eq!(tool["function"]["parameters"]["additionalProperties"],false);if is_write(name){assert!(tool["function"]["parameters"]["required"].as_array().unwrap().iter().any(|field|field=="confirmedByUser"));}}
}
