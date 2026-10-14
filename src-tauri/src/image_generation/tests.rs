use super::*;
struct Protector;
impl SecretProtector for Protector {fn protect(&self,plain:&str)->AppResult<String>{Ok(format!("encrypted:{plain}"))}fn unprotect(&self,encrypted:&str)->AppResult<String>{encrypted.strip_prefix("encrypted:").map(String::from).ok_or_else(AppError::storage)}}
fn input(key:SecretUpdate)->Input {Input {name:"图片服务".into(),protocol:"openai-compatible".into(),base_url:"https://images.example/v1/".into(),model:"transparent-model".into(),api_key:key}}
#[test]
fn defaults_keys_are_isolated_encrypted_and_not_in_public_config() {
    let mut c=crate::db::test_database();let original=get(&c).unwrap();assert!(!original.enabled);assert!(original.providers.is_empty());assert!(snapshot(&c,&Protector).unwrap().is_none());
    c.execute("INSERT INTO providers(name,model,api_key_encrypted,is_active) VALUES('聊天','model','chat-key',1)",[]).unwrap();
    let config=save(&mut c,None,0,input(SecretUpdate::Set {value:"private-image-key".into()}),&Protector).unwrap();
    assert_eq!(config.providers[0].provider.base_url,"https://images.example/v1");assert!(config.providers[0].has_api_key);
    let value=serde_json::to_string(&config).unwrap();assert!(!value.contains("private-image-key"));assert!(!value.contains("encrypted:"));assert!(!value.contains("chat-key"));
    assert_eq!(secret(&c,"1").unwrap(),Some("encrypted:private-image-key".into()));
    assert!(snapshot(&c,&Protector).unwrap().is_none());
    let config=select(&mut c,Some("1".into()),config.revision).unwrap();
    set_enabled(&mut c,true,config.revision).unwrap();let snap=snapshot(&c,&Protector).unwrap().unwrap();assert_eq!(snap.key,"private-image-key");
    assert!(crate::config::get(&c).unwrap().keys().all(|key|!key.starts_with("image_generation")));
    assert!(crate::config::save(&mut c,std::collections::BTreeMap::from([("image_generation_config".into(),"{}".into())])).is_err());
}
#[test]
fn revision_conflict_current_delete_clear_key_and_no_automatic_selection() {
    let mut c=crate::db::test_database();
    let config=save(&mut c,None,0,input(SecretUpdate::Set {value:"key".into()}),&Protector).unwrap();
    assert_eq!(select(&mut c,Some("1".into()),0).unwrap_err().code,"CONFIG_CONFLICT");
    let config=select(&mut c,Some("1".into()),config.revision).unwrap();let config=set_enabled(&mut c,true,config.revision).unwrap();
    assert_eq!(delete(&mut c,"1",config.revision).unwrap_err().code,"ACTIVE_PROVIDER");
    let config=save(&mut c,Some("1".into()),config.revision,input(SecretUpdate::Clear),&Protector).unwrap();assert!(!config.enabled);assert!(!config.providers[0].has_api_key);
    assert!(set_enabled(&mut c,true,config.revision).is_err());
    let config=select(&mut c,None,config.revision).unwrap();let config=delete(&mut c,"1",config.revision).unwrap();assert!(config.active_id.is_none());assert!(!config.enabled);assert!(config.providers.is_empty());assert_eq!(config.next_id,"2");
}
#[test]
fn root_change_never_reuses_key_and_failure_rolls_back() {
    let mut c=crate::db::test_database();let config=save(&mut c,None,0,input(SecretUpdate::Set {value:"key".into()}),&Protector).unwrap();
    let mut changed=input(SecretUpdate::Keep);changed.base_url="https://new.example/v1".into();
    assert!(save(&mut c,Some("1".into()),config.revision,changed,&Protector).is_err());assert_eq!(get(&c).unwrap().revision,config.revision);assert_eq!(secret(&c,"1").unwrap(),Some("encrypted:key".into()));
    for address in ["http://example.com/v1","https://name:pass@example.com/v1","https://example.com/v1?key=a","file:///tmp/image"] {
        let mut bad=input(SecretUpdate::Set {value:"do-not-save".into()});bad.base_url=address.into();assert!(save(&mut c,None,config.revision,bad,&Protector).is_err());
    }
    c.execute_batch("CREATE TRIGGER fail BEFORE UPDATE ON settings WHEN NEW.key='image_generation_config' BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(save(&mut c,Some("1".into()),config.revision,input(SecretUpdate::Set {value:"replacement".into()}),&Protector).is_err());
    assert_eq!(secret(&c,"1").unwrap(),Some("encrypted:key".into()));assert_eq!(get(&c).unwrap().revision,config.revision);
}
