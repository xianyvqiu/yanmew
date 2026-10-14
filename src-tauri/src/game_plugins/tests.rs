use super::*;
use rusqlite::Connection;
fn manifest() -> Manifest {
    serde_json::from_value(serde_json::json!({"id":"org.example.test","name":"测试插件","description":"存档和事务测试","author":"test","version":"1.0.0","api_version":"1.0.0","entry":"ui/index.html","storage_version":1,"permissions":["wallet.modify","network.fetch"],"network_origins":["https://example.com"],"settings_schema":[{"key":"audio.volume","title":"音量","type":"integer","default":50,"minimum":0,"maximum":100}]})).unwrap()
}
fn setup() -> (Connection, Manifest) {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    c.execute_batch(include_str!("../../../db/schema.sql"))
        .unwrap();
    c.execute(
        "INSERT INTO user_state(id,coins,updated_at) VALUES(1,10,'now')",
        [],
    )
    .unwrap();
    let m = manifest();
    register(&c, &m, "versions/test", "abc", false, Utc::now(), &[]).unwrap();
    (c, m)
}
fn grant(c: &mut Connection, m: &Manifest, decision: Decision) {
    decide(
        c,
        &m.id,
        &m.version,
        &PermissionChoice {
            permission: "wallet.modify".into(),
            scope: "".into(),
            decision,
        },
        Utc::now(),
    )
    .unwrap();
}
fn request(run: &runs::Run, delta: i64) -> runs::WalletRequest {
    runs::WalletRequest {
        request_id: "settle-1".into(),
        game_run_id: run.id.clone(),
        delta,
        reason: "测试奖励".into(),
        details: None,
    }
}
#[test]
fn wallet_atomic_duplicate_and_revocation() {
    let (mut c, m) = setup();
    grant(&mut c, &m, Decision::Allowed);
    let r = runs::start(&c, &m.id, &m.version, Utc::now()).unwrap();
    let req = request(&r, 5);
    let a = runs::change_coins(&mut c, &m.id, &m.version, &r.id, &req, Utc::now()).unwrap();
    assert_eq!(a.balance, "15");
    grant(&mut c, &m, Decision::Denied);
    let b = runs::change_coins(&mut c, &m.id, &m.version, &r.id, &req, Utc::now()).unwrap();
    assert!(b.duplicate);
    assert_eq!(b.ledger_id, a.ledger_id);
    let mut next = request(&r, 5);
    next.request_id = "next".into();
    assert_eq!(
        runs::change_coins(&mut c, &m.id, &m.version, &r.id, &next, Utc::now())
            .unwrap_err()
            .code,
        "PLUGIN_PERMISSION_DENIED"
    );
    assert_eq!(crate::currency::get_balance(&c).unwrap(), 15);
}
#[test]
fn insufficient_balance_and_request_conflict() {
    let (mut c, m) = setup();
    grant(&mut c, &m, Decision::Allowed);
    let r = runs::start(&c, &m.id, &m.version, Utc::now()).unwrap();
    assert_eq!(
        runs::change_coins(
            &mut c,
            &m.id,
            &m.version,
            &r.id,
            &request(&r, -11),
            Utc::now()
        )
        .unwrap_err()
        .code,
        "INSUFFICIENT_COINS"
    );
    assert_eq!(runs::get_run(&c, &r.id).unwrap().coin_delta, "0");
    runs::change_coins(
        &mut c,
        &m.id,
        &m.version,
        &r.id,
        &request(&r, 0),
        Utc::now(),
    )
    .unwrap();
    assert_eq!(
        runs::change_coins(
            &mut c,
            &m.id,
            &m.version,
            &r.id,
            &request(&r, 1),
            Utc::now()
        )
        .unwrap_err()
        .code,
        "PLUGIN_REQUEST_CONFLICT"
    );
}
#[test]
fn pure_play_stays_pure_and_scope_cannot_be_invented() {
    let (mut c, m) = setup();
    let r = runs::start(&c, &m.id, &m.version, Utc::now()).unwrap();
    grant(&mut c, &m, Decision::Allowed);
    assert!(runs::change_coins(
        &mut c,
        &m.id,
        &m.version,
        &r.id,
        &request(&r, 5),
        Utc::now()
    )
    .is_err());
    assert!(decide(
        &mut c,
        &m.id,
        &m.version,
        &PermissionChoice {
            permission: "network.fetch".into(),
            scope: "https://evil.example".into(),
            decision: Decision::Allowed
        },
        Utc::now()
    )
    .is_err());
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM currency_ledger", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn storage_settings_and_namespace() {
    let (mut c, m) = setup();
    assert!(save_setting(&c, &m, "audio.volume", &serde_json::json!(101), Utc::now()).is_err());
    assert!(save_setting(&c, &m, "unknown", &serde_json::json!(1), Utc::now()).is_err());
    save_setting(&c, &m, "audio.volume", &serde_json::json!(80), Utc::now()).unwrap();
    assert_eq!(merged_settings(&c, &m).unwrap()["audio.volume"], 80);
    data_set(
        &mut c,
        &m.id,
        &m.version,
        "progress.main",
        &serde_json::json!({"level":42}),
        Utc::now(),
    )
    .unwrap();
    let mut other = m.clone();
    other.id = "org.example.other".into();
    register(&c, &other, "versions/other", "def", false, Utc::now(), &[]).unwrap();
    assert_eq!(
        data_get(&c, &other.id, &other.version, "progress.main").unwrap(),
        None
    );
    assert!(data_get(&c, &m.id, "2.0.0", "progress.main").is_err());
}
#[test]
fn close_keeps_progress_and_history_survives_uninstall() {
    let (mut c, m) = setup();
    grant(&mut c, &m, Decision::Allowed);
    data_set(
        &mut c,
        &m.id,
        &m.version,
        "progress.main",
        &serde_json::json!(7),
        Utc::now(),
    )
    .unwrap();
    let r = runs::start(&c, &m.id, &m.version, Utc::now()).unwrap();
    runs::change_coins(
        &mut c,
        &m.id,
        &m.version,
        &r.id,
        &request(&r, 5),
        Utc::now(),
    )
    .unwrap();
    runs::stop_plugin(&c, &m.id, "cancelled", Utc::now()).unwrap();
    assert_eq!(snapshot(&c, &m.id).unwrap().data["progress.main"], 7);
    uninstall(&mut c, &m.id, &m.version, true, Utc::now()).unwrap();
    assert_eq!(runs::get_run(&c, &r.id).unwrap().coin_delta, "5");
    assert_eq!(crate::currency::get_balance(&c).unwrap(), 15);
}
#[test]
fn complete_idempotent_and_finished_rejects_new_wallet_request() {
    let (mut c, m) = setup();
    grant(&mut c, &m, Decision::Allowed);
    let r = runs::start(&c, &m.id, &m.version, Utc::now()).unwrap();
    let details = Some(serde_json::json!({"rounds":3}));
    runs::complete(
        &c,
        &m.id,
        &m.version,
        &r.id,
        "done",
        details.clone(),
        Utc::now(),
    )
    .unwrap();
    runs::complete(&c, &m.id, &m.version, &r.id, "done", details, Utc::now()).unwrap();
    assert!(runs::complete(&c, &m.id, &m.version, &r.id, "different", None, Utc::now()).is_err());
    assert!(runs::change_coins(
        &mut c,
        &m.id,
        &m.version,
        &r.id,
        &request(&r, 5),
        Utc::now()
    )
    .is_err());
}
#[test]
fn update_scopes_and_snapshot_validation() {
    let (mut c, m) = setup();
    grant(&mut c, &m, Decision::Allowed);
    let mut newer = m.clone();
    newer.version = "1.1.0".into();
    newer.network_origins.push("https://other.example".into());
    let tx = c.transaction().unwrap();
    register(&tx, &newer, "versions/new", "new", false, Utc::now(), &[]).unwrap();
    tx.commit().unwrap();
    assert!(allowed(&c, &m.id, &newer.version, "wallet.modify", "").unwrap());
    assert!(!allowed(
        &c,
        &m.id,
        &newer.version,
        "network.fetch",
        "https://other.example"
    )
    .unwrap());
    let mut s = snapshot(&c, &m.id).unwrap();
    s.storage_version = 2;
    assert!(validate_snapshot(&newer, &s).is_err());
}

fn archive(entries: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    use std::io::{Cursor, Write};
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    for (name, bytes) in entries {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
#[test]
fn already_committed_request_can_be_verified_after_upgrade() {
    let (mut c, m) = setup();
    grant(&mut c, &m, Decision::Allowed);
    let old = runs::start(&c, &m.id, &m.version, Utc::now()).unwrap();
    let req = request(&old, 5);
    let result = runs::change_coins(&mut c, &m.id, &m.version, &old.id, &req, Utc::now()).unwrap();
    runs::stop_plugin(&c, &m.id, "cancelled", Utc::now()).unwrap();
    let mut new = m.clone();
    new.version = "1.1.0".into();
    register(&c, &new, "versions/new", "new", false, Utc::now(), &[]).unwrap();
    let active = runs::start(&c, &new.id, &new.version, Utc::now()).unwrap();
    let replay =
        runs::change_coins(&mut c, &new.id, &new.version, &active.id, &req, Utc::now()).unwrap();
    assert!(replay.duplicate);
    assert_eq!(replay.ledger_id, result.ledger_id);
    uninstall(&mut c, &new.id, &new.version, true, Utc::now()).unwrap();
    let history = runs::ledger(&c, &new.id, None, 30).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].delta, "5");
    assert_eq!(history[0].plugin_version, "1.0.0");
}
fn valid_package(m: &Manifest) -> package::Package {
    package::validate(archive(vec![
        ("manifest.json", serde_json::to_vec(m).unwrap()),
        ("ui/index.html", b"<!doctype html>".to_vec()),
    ]))
    .unwrap()
}
#[test]
fn package_rejects_traversal_case_aliases_unsupported_api_and_resources() {
    let m = manifest();
    let base = || {
        vec![
            ("manifest.json", serde_json::to_vec(&m).unwrap()),
            ("ui/index.html", vec![]),
        ]
    };
    for path in [
        "../outside.js",
        "C:/evil.js",
        "ui/CON.js",
        "ui/a%2f.js",
        "ui\\evil.js",
        "ui/a.exe",
    ] {
        let mut entries = base();
        entries.push((path, vec![]));
        assert!(package::validate(archive(entries)).is_err(), "{path}");
    }
    let mut entries = base();
    entries.push(("ui/INDEX.html", vec![]));
    assert!(package::validate(archive(entries)).is_err());
    let mut newer = m.clone();
    newer.api_version = "2.0.0".into();
    assert!(package::validate(archive(vec![
        ("manifest.json", serde_json::to_vec(&newer).unwrap()),
        ("ui/index.html", vec![])
    ]))
    .is_err());
    assert!(package::validate(vec![1, 2, 3]).is_err());
    assert_eq!(valid_package(&m).manifest.id, m.id);
}
#[test]
fn package_expansion_limit_and_missing_entry() {
    let m = manifest();
    assert!(package::validate(archive(vec![(
        "manifest.json",
        serde_json::to_vec(&m).unwrap()
    )]))
    .is_err());
    assert!(package::validate(archive(vec![
        ("manifest.json", serde_json::to_vec(&m).unwrap()),
        ("ui/index.html", vec![]),
        ("ui/big.txt", vec![b'x'; package::MAX_FILE + 1])
    ]))
    .is_err());
}
#[test]
fn migrated_data_and_registration_commit_together() {
    let (mut c, m) = setup();
    data_set(
        &mut c,
        &m.id,
        &m.version,
        "progress.main",
        &serde_json::json!({"old":7}),
        Utc::now(),
    )
    .unwrap();
    let original = snapshot(&c, &m.id).unwrap();
    let mut new = m.clone();
    new.version = "2.0.0".into();
    new.storage_version = 2;
    let mut migrated = original.clone();
    migrated.storage_version = 2;
    migrated
        .data
        .insert("progress.main".into(), serde_json::json!({"level":7}));
    {
        let tx = c.transaction().unwrap();
        register(&tx, &new, "versions/new", "new", false, Utc::now(), &[]).unwrap();
        replace_snapshot(&tx, &new, &migrated, Utc::now()).unwrap(); // 模拟提交之前进程失败：drop 回滚。
    }
    assert_eq!(get(&c, &m.id).unwrap().installed_version, "1.0.0");
    assert_eq!(snapshot(&c, &m.id).unwrap(), original);
    let tx = c.transaction().unwrap();
    register(&tx, &new, "versions/new", "new", false, Utc::now(), &[]).unwrap();
    replace_snapshot(&tx, &new, &migrated, Utc::now()).unwrap();
    tx.commit().unwrap();
    assert_eq!(snapshot(&c, &m.id).unwrap(), migrated);
    let mut invalid = migrated.clone();
    invalid
        .data
        .insert("../outside".into(), serde_json::json!(0));
    let tx = c.transaction().unwrap();
    assert!(replace_snapshot(&tx, &new, &invalid, Utc::now()).is_err());
    drop(tx);
    assert_eq!(snapshot(&c, &m.id).unwrap(), migrated);
}
#[test]
fn immutable_publish_integrity_and_orphan_cleanup() {
    let (c, m) = setup();
    let pkg = valid_package(&m);
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!(
            "plugin-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
    let relative = package::publish(&root, &pkg).unwrap();
    assert_eq!(package::publish(&root, &pkg).unwrap(), relative);
    let mut p = get(&c, &m.id).unwrap();
    p.package_path = Some(relative.clone());
    p.package_sha256 = pkg.sha.clone();
    assert!(package::load(&root, &p).is_ok());
    package::cleanup(&root, &std::collections::HashSet::from([relative.clone()])).unwrap();
    assert!(root.join(&relative).exists());
    std::fs::write(root.join(&relative).join("package.yanmew-game"), b"broken").unwrap();
    assert!(package::load(&root, &p).is_err());
    package::cleanup(&root, &Default::default()).unwrap();
    assert!(!root.join(relative).exists());
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .canonicalize()
        .unwrap();
    assert!(root.canonicalize().unwrap().starts_with(base));
    std::fs::remove_dir_all(root).unwrap();
}
