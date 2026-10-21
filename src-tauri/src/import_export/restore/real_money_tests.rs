use super::*;
use crate::real_money::{self,Action,Actor,EntryInput};
fn add(rt:&Runtime,key:&str,amount:&str){rt.read(|s,_|{let cat=s.connection.query_row("SELECT id FROM real_categories WHERE name='食物'",[],|r|r.get::<_,i64>(0))?;real_money::mutate(&mut s.connection,key,Action::SaveEntry{id:None,revision:None,entry:EntryInput{title:"真实午餐".into(),kind:"expense".into(),category_id:cat.to_string(),asset_id:"1".into(),description:"现实记账备份".into(),amount:amount.into(),occurred_local:Some("2026-10-13T10:00".into()),recorder:None},recurrence:None},chrono::Utc::now(),Actor{source:"manual",name:"用户".into()})?;Ok(())}).unwrap();}
#[test]
fn real_money_full_backup_restores_exact_ids_without_replaying_entries(){
 let a=Directory::new();let b=Directory::new();let source=fixture(&a.0);let target=fixture(&b.0);add(&source,"lunch","12.34");add(&target,"old-target","99.99");let transfer=make(&source,&a.0,PackageKind::Full,&[],false);let p=prepare(&target,&transfer).unwrap().unwrap();confirm(&target,input(&p)).unwrap();target.read(|s,_|{assert_eq!(real_money::balances(&s.connection)?[&1],-1234);assert_eq!(s.connection.query_row("SELECT count(*) FROM real_entries",[],|r|r.get::<_,i64>(0))?,1);assert_eq!(s.connection.query_row("SELECT count(*) FROM real_operations WHERE request_id='lunch'",[],|r|r.get::<_,i64>(0))?,1);real_money::validate(&s.connection)}).unwrap();
}
#[test]
fn real_money_persona_package_never_replaces_global_finances(){
 let a=Directory::new();let b=Directory::new();let source=fixture(&a.0);let target=fixture(&b.0);add(&source,"private-source","100");add(&target,"keep-target","3.21");let transfer=make(&source,&a.0,PackageKind::PersonaFull,&["2"],false);let p=prepare(&target,&transfer).unwrap().unwrap();confirm(&target,input(&p)).unwrap();target.read(|s,_|{assert_eq!(real_money::balances(&s.connection)?[&1],-321);assert_eq!(s.connection.query_row("SELECT count(*) FROM real_entries WHERE amount_cents=10000",[],|r|r.get::<_,i64>(0))?,0);Ok(())}).unwrap();
}
#[test]
fn real_money_v17_backup_restores_empty_real_book(){
 let a=Directory::new();let b=Directory::new();let source=fixture(&a.0);let target=fixture(&b.0);add(&target,"old","6");let transfer=make(&source,&a.0,PackageKind::Full,&[],false);rewrite(&transfer.0,|m,_|m.schema_version=17);let p=prepare(&target,&transfer).unwrap().unwrap();confirm(&target,input(&p)).unwrap();target.read(|s,_|{assert_eq!(s.connection.query_row("SELECT count(*) FROM real_assets",[],|r|r.get::<_,i64>(0))?,3);assert_eq!(s.connection.query_row("SELECT count(*) FROM real_entries",[],|r|r.get::<_,i64>(0))?,0);real_money::validate(&s.connection)}).unwrap();
}
