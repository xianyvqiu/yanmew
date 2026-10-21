use super::*;

fn database() -> Connection {
    let c = crate::db::test_database();
    c.execute_batch("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'会话一','a','a'),(2,1,'会话二','a','a'); UPDATE sessions SET craft_tier_1_used=1 WHERE id=1;
        INSERT INTO accessories(id,session_id,name,display_text,description,cost,source,created_at) VALUES
        (1,1,'星星','★','闪亮',20,'crafted','a'),(2,1,'花','✿','花朵',50,'crafted','a'),(3,2,'皇冠','👑','另一会话',0,'travel','a');").unwrap();
    c
}
#[test]
fn list_is_scoped_paginated_and_serializes_large_numbers_as_strings() {
    let c = database();
    c.execute("UPDATE accessories SET cost=?1 WHERE id=1", [i64::MAX])
        .unwrap();
    let page = list(&c, 1, None, 1).unwrap();
    assert_eq!(page[0].id, "2");
    let last = list(&c, 1, Some(2), 30).unwrap();
    assert_eq!(last.len(), 1);
    let value = serde_json::to_value(&last[0]).unwrap();
    assert_eq!(value["cost"], i64::MAX.to_string());
    assert!(value["imageFile"].is_null());
    assert_eq!(get(&c, 1, 3).unwrap_err().code, "NOT_FOUND");
    assert!(list(&c, 1, None, 0).is_err());
    assert!(list(&c, 1, Some(-1), 30).is_err());
}
#[test]
fn wear_keeps_multiple_items_and_wrong_session_does_not_unwear_existing() {
    let mut c = database();
    assert!(wear(&mut c, 1, 1, true).unwrap().is_worn);
    assert!(wear(&mut c, 1, 2, true).unwrap().is_worn);
    assert!(get(&c, 1, 1).unwrap().is_worn);
    assert!(wear(&mut c, 1, 3, true).is_err());
    assert!(get(&c, 1, 2).unwrap().is_worn);
    assert_eq!(
        sell(&mut c, 1, 2, Utc::now()).unwrap_err().code,
        "ACCESSORY_WORN"
    );
    assert_eq!(delete(&mut c, 1, 2).unwrap_err().code, "ACCESSORY_WORN");
    assert!(!wear(&mut c, 1, 2, false).unwrap().is_worn);
    wear(&mut c, 1, 2, false).unwrap();
}
#[test]
fn failed_wear_restores_old_item() {
    let mut c = database();
    wear(&mut c, 1, 1, true).unwrap();
    c.execute_batch("CREATE TRIGGER deny_wear BEFORE UPDATE ON accessories WHEN NEW.id=2 AND NEW.is_worn=1 BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap();
    assert!(wear(&mut c, 1, 2, true).is_err());
    assert!(get(&c, 1, 1).unwrap().is_worn);
    assert!(!get(&c, 1, 2).unwrap().is_worn);
}
#[test]
fn sell_is_ten_coins_once_and_keeps_tier_and_historical_ledger() {
    let mut c = database();
    let (result, _) = sell(&mut c, 1, 1, Utc::now()).unwrap();
    assert_eq!(result.wallet.coins, "10");
    assert_eq!(result.deleted_id, "1");
    assert!(sell(&mut c, 1, 1, Utc::now()).is_err());
    assert_eq!(currency::get_balance(&c).unwrap(), 10);
    let ledger: (i64, String, i64) = c
        .query_row("SELECT delta,reason,ref_id FROM currency_ledger", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(ledger, (10, "accessory_sell".into(), 1));
    assert_eq!(
        c.query_row(
            "SELECT craft_tier_1_used FROM sessions WHERE id=1",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert!(c
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query([])
        .unwrap()
        .next()
        .unwrap()
        .is_none());
}
#[test]
fn ledger_or_delete_failure_and_overflow_keep_item_wallet_and_ledger() {
    for fail in ["ledger", "delete", "overflow"] {
        let mut c = database();
        match fail {
            "ledger" => c.execute_batch("CREATE TRIGGER fail BEFORE INSERT ON currency_ledger BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap(),
            "delete" => c.execute_batch("CREATE TRIGGER fail BEFORE DELETE ON accessories BEGIN SELECT RAISE(ABORT,'test'); END;").unwrap(),
            _ => {c.execute("UPDATE user_state SET coins=?1",[i64::MAX]).unwrap();},
        }
        let before = currency::get_wallet(&c).unwrap();
        assert!(sell(&mut c, 1, 1, Utc::now()).is_err());
        assert_eq!(currency::get_wallet(&c).unwrap(), before);
        assert!(get(&c, 1, 1).is_ok());
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM currency_ledger", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
#[test]
fn delete_and_session_cascade_do_not_reward_or_reset_tiers() {
    let mut c = database();
    let (deleted, _) = delete(&mut c, 1, 1).unwrap();
    assert_eq!(deleted.deleted_id, "1");
    assert!(delete(&mut c, 1, 1).is_err());
    assert_eq!(currency::get_balance(&c).unwrap(), 0);
    assert_eq!(
        c.query_row(
            "SELECT craft_tier_1_used FROM sessions WHERE id=1",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    c.execute("DELETE FROM sessions WHERE id=1", []).unwrap();
    assert!(list(&c, 1, None, 30).unwrap().is_empty());
    assert_eq!(list(&c, 2, None, 30).unwrap().len(), 1);
}
#[test]
fn actual_context_revision_and_session_are_required() {
    let runtime = crate::runtime::test_runtime("http://localhost");
    runtime
        .read(|store, _| {
            require_current(store, 1, 1, 0)?;
            assert_eq!(
                require_current(store, 1, 1, 1).unwrap_err().code,
                "CONTEXT_CHANGED"
            );
            assert_eq!(
                require_current(store, 1, 2, 0).unwrap_err().code,
                "CONTEXT_CHANGED"
            );
            store
                .connection
                .execute("DELETE FROM settings WHERE key='current_session_id'", [])?;
            assert_eq!(
                require_current(store, 1, 1, 0).unwrap_err().code,
                "CONTEXT_CHANGED"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn layers_reorder_current_session_without_changing_wallet_or_worn_state(){
 let mut c=crate::db::test_database();
 c.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'测试','a','a')",[]).unwrap();
 c.execute_batch("INSERT INTO accessories(id,session_id,name,display_text,description,cost,source,created_at,is_worn) VALUES(501,1,'帽子','✦','一',20,'crafted','a',1),(502,1,'围巾','✦','二',20,'crafted','a',1);").unwrap();
 let initial=crate::currency::get_wallet(&c).unwrap().coins;
 layer(&mut c,1,501,"back","keep").unwrap();assert_eq!(get(&c,1,501).unwrap().render_layer,"back");
 layer(&mut c,1,501,"front","top").unwrap();assert!(get(&c,1,501).unwrap().layer_order>get(&c,1,502).unwrap().layer_order);
 layer(&mut c,1,501,"front","bottom").unwrap();assert!(get(&c,1,501).unwrap().layer_order<get(&c,1,502).unwrap().layer_order);
 assert!(layer(&mut c,2,501,"front","top").is_err());assert!(layer(&mut c,1,501,"invalid","top").is_err());assert!(get(&c,1,501).unwrap().is_worn&&get(&c,1,502).unwrap().is_worn);assert_eq!(crate::currency::get_wallet(&c).unwrap().coins,initial);
}
