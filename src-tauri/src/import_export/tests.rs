use super::*;
use base64::Engine;
use rusqlite::params;
use serde_json::json;
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};
use zip::ZipArchive;

const NOW: &str = "2026-10-13T00:00:00Z";
pub(super) fn runtime() -> Runtime {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    runtime.read(|s, _| {
        s.connection.pragma_update(None, "user_version", SCHEMA_VERSION).unwrap();
        s.connection.execute("INSERT INTO personas(id,name,prompt,initial_prompt,color,expressions) VALUES(2,'另一人设','私人设定','初始设定','#aabbcc','{}')", []).unwrap();
        s.connection.execute("INSERT INTO user_profiles(id,name,content,created_at,updated_at) VALUES(10,'用户设定','资料正文',?1,?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO sessions(id,persona_id,user_profile_id,title,created_at,updated_at,intimacy,craft_tier_1_used) VALUES(2,2,10,'另一会话',?1,?1,42,1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO chat_rounds(id,session_id,status,created_at,completed_at) VALUES(10,2,'completed',?1,?1),(11,2,'completed',?1,?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO messages(id,session_id,round_id,role,content,created_at) VALUES(10,2,10,'user','用户原文',?1),(11,2,10,'assistant','AI原文',?1),(12,2,11,'user','触发轮',?1),(13,2,11,'assistant','触发轮回复',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO short_memories(id,session_id,round_id,content,integration_state,created_at,updated_at) VALUES(10,2,10,'已整合短期','integrated',?1,?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO long_memories(id,session_id,title,content,source_count,activated_by_round_id,created_at,updated_at) VALUES(10,2,'长期标题','长期正文',1,11,?1,?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO long_memory_sources VALUES(10,10,1)", []).unwrap();
        s.connection.execute("INSERT INTO notebooks(session_id,content,updated_at) VALUES(2,'自由记事本',?1)", [NOW]).unwrap();
        s.connection.execute("UPDATE session_pet_states SET hunger_elapsed_ms=1234,idle_elapsed_ms=5678,idle_branch='sleeping',sleep_until='2026-10-13T02:00:00Z' WHERE session_id=2", []).unwrap();
        s.connection.execute("INSERT INTO travels(id,session_id,status,started_at,planned_return_at,actual_return_at,midway_letter_at,midway_letter_state,return_letter_state,gift_state,journal_state,gift_kind,facts_state,destination,activities_json,facts_version,facts_ready_at,gift_snapshot_json,created_at) VALUES(10,2,'returned',?1,?1,?1,?1,'done','done','done','done','text','ready','湖边','[{\"elapsed_minutes\":0,\"description\":\"散步\"}]',1,?1,'{\"schema_version\":1,\"kind\":\"text\",\"content\":\"明信片\",\"note\":\"给你\"}',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO souvenirs(id,session_id,travel_id,content,note,created_at) VALUES(10,2,10,'明信片','给你',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO accessories(id,session_id,name,display_text,description,cost,source,created_at) VALUES(10,2,'小帽子','🎩','描述',20,'crafted',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO letters(id,session_id,travel_id,sender,content,has_been_replied,letter_kind,created_at) VALUES(10,2,10,'ai','原信',1,'midway',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO letters(id,session_id,travel_id,sender,content,reply_to_id,reply_due_at,reply_state,letter_kind,created_at) VALUES(11,2,10,'user','回信',10,?1,'cancelled','user',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO custom_festivals(festival_id,kind,name,year,calendar_type,solar_month,solar_day,blessing,source,created_at,updated_at) VALUES((SELECT id FROM festival_catalog WHERE code='solar_new_year'),'festival','元旦',2026,'solar',1,1,'祝福','user',?1,?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO game_plugins(plugin_id,name,author,installed_version,api_version,source_type,package_path,package_sha256,repository,network_origins_json,installed_at,updated_at) VALUES('test.game','测试游戏','作者','1.0.0','1.0.0','local','本机插件路径',?1,'https://github.com/example/game','[\"https://example.com\"]',?2,?2)", params!["a".repeat(64), NOW]).unwrap();
        s.connection.execute("INSERT INTO game_permissions VALUES('test.game','wallet.modify','','allowed','1.0.0',?1),('test.game','network.fetch','https://example.com','allowed','1.0.0',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO game_plugin_settings VALUES('test.game','volume','0.5',?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO game_plugin_data VALUES('test.game','progress','{\"level\":3}',1,?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO game_runs(id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,status,coin_delta,started_at,ended_at) VALUES(10,'test.game','测试游戏','1.0.0',1,'completed',3,?1,?1)", [NOW]).unwrap();
        s.connection.execute("INSERT INTO currency_ledger(id,delta,reason,game_run_id,plugin_id,plugin_version,plugin_display_reason,wallet_request_id,created_at) VALUES(10,3,'plugin_game',10,'test.game','1.0.0','奖励','request-1',?1)", [NOW]).unwrap();
        Ok(())
    }).unwrap();
    runtime
}
fn request(kind: PackageKind, ids: &[&str], include_sessions: bool) -> ExportRequest {
    ExportRequest {
        kind,
        ids: ids.iter().map(|v| (*v).into()).collect(),
        include_sessions,
    }
}
#[derive(Debug)]
struct TestArchive {
    bytes_base64: String,
    size: u64,
    sha256: String,
}
struct TestTransfer(PathBuf);
impl crate::platform::contracts::FileTransferPort for TestTransfer {
    fn save_backup(&self, _: &str) -> AppResult<Option<PathBuf>> {
        Ok(Some(self.0.clone()))
    }
    fn publish(&self, temporary: &std::path::Path, destination: &std::path::Path) -> AppResult<()> {
        #[cfg(windows)]
        {
            crate::platform::windows::files::BackupFiles { owner: 0 }
                .publish(temporary, destination)
        }
        #[cfg(not(windows))]
        {
            fs::rename(temporary, destination).map_err(|_| AppError::storage())
        }
    }
}
fn export(runtime: &Runtime, input: ExportRequest) -> AppResult<TestArchive> {
    let dir = Directory::new();
    let transfer = TestTransfer(dir.0.join("backup.zip"));
    let result = super::export(runtime, input, &transfer)?.unwrap();
    let bytes = fs::read(&transfer.0).map_err(|_| AppError::storage())?;
    Ok(TestArchive {
        bytes_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        size: result.size,
        sha256: result.sha256,
    })
}
fn unpack(output: &TestArchive) -> (Manifest, Tables, Vec<u8>) {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&output.bytes_base64)
        .unwrap();
    assert_eq!(bytes.len() as u64, output.size);
    assert_eq!(archive::hash(&bytes), output.sha256);
    let mut zip = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut raw = vec![];
    zip.by_name("manifest.json")
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    let manifest: Manifest = serde_json::from_slice(&raw).unwrap();
    raw.clear();
    zip.by_name("data.json")
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    assert_eq!(manifest.data_size, raw.len() as u64);
    assert_eq!(manifest.data_sha256, archive::hash(&raw));
    let tables: Tables = serde_json::from_slice(&raw).unwrap();
    assert_eq!(
        manifest.record_counts,
        tables
            .iter()
            .map(|(k, v)| (k.clone(), v.len() as u64))
            .collect()
    );
    for entry in &manifest.files {
        let mut contents = vec![];
        zip.by_name(&entry.path)
            .unwrap()
            .read_to_end(&mut contents)
            .unwrap();
        assert_eq!(entry.size, contents.len() as u64);
        assert_eq!(entry.sha256, archive::hash(&contents));
    }
    (manifest, tables, raw)
}

#[test]
fn six_kinds_have_closed_scopes_and_preserve_states() {
    let runtime = runtime();
    let cases = [
        request(PackageKind::Full, &[], false),
        request(PackageKind::Session, &["2"], false),
        request(PackageKind::UserProfile, &["10"], false),
        request(PackageKind::PersonaProfile, &["2"], false),
        request(PackageKind::PersonaFull, &["2"], false),
        request(PackageKind::PluginData, &["test.game"], false),
        request(PackageKind::UserProfile, &["10"], true),
    ];
    for input in cases {
        let kind = input.kind;
        let include_sessions = input.include_sessions;
        let output = export(&runtime, input).unwrap();
        let (manifest, tables, _) = unpack(&output);
        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.schema_version, SCHEMA_VERSION);
        assert!(!tables.contains_key("long_memory_embeddings"));
        assert!(!tables.contains_key("festival_catalog"));
        if kind != PackageKind::Full {
            for table in [
                "providers",
                "provider_models",
                "provider_roles",
                "embedding_providers",
                "settings",
                "reminders",
                "user_state",
                "currency_ledger",
                "game_runs",
            ] {
                assert!(!tables.contains_key(table));
            }
        }
        if ![PackageKind::Full, PackageKind::PluginData].contains(&kind) {
            assert!(!tables.contains_key("game_plugins"));
        }
        if [PackageKind::Session, PackageKind::PersonaFull].contains(&kind)
            || (kind == PackageKind::UserProfile && include_sessions)
        {
            assert_eq!(tables["sessions"].len(), 1);
            assert_eq!(tables["sessions"][0]["id"], "2");
            assert_eq!(tables["sessions"][0]["craft_tier_1_used"], 1);
            assert_eq!(tables["session_pet_states"][0]["hunger_elapsed_ms"], 1234);
            assert_eq!(
                tables["session_pet_states"][0]["sleep_until"],
                "2026-10-13T02:00:00Z"
            );
            assert_eq!(
                tables["short_memories"][0]["integration_state"],
                "integrated"
            );
            assert_eq!(tables["long_memories"][0]["activated_by_round_id"], "11");
            assert_eq!(tables["long_memory_sources"][0]["source_order"], 1);
            assert_eq!(tables["travels"][0]["journal_state"], "done");
            assert!(tables["travel_journals"].is_empty());
            assert_eq!(tables["letters"][0]["has_been_replied"], 1);
            assert_eq!(tables["letters"][1]["reply_to_id"], "10");
            if kind == PackageKind::Full {
                assert_eq!(tables["custom_festivals"][0]["festival_code"], "solar_new_year");
                assert!(!tables["custom_festivals"][0].contains_key("festival_id"));
            } else {
                assert!(!tables.contains_key("custom_festivals"));
            }
        }
        if kind == PackageKind::PersonaProfile {
            assert_eq!(tables.keys().collect::<Vec<_>>(), vec!["persona_food_settings", "persona_skills", "personas", "pet_artworks", "skills"]);
        }
        if kind == PackageKind::UserProfile && !include_sessions {
            assert_eq!(tables.keys().collect::<Vec<_>>(), vec!["user_profiles"]);
        }
    }
}
#[test]
fn provider_keys_weather_host_and_unknown_settings_are_not_exported() {
    let runtime = runtime();
    runtime.read(|s, _| {
        s.connection.execute("UPDATE providers SET api_key_encrypted='CHAT_SECRET'", [])?;
        s.connection.execute("INSERT INTO embedding_providers(name,kind,base_url,api_key_encrypted,model,is_builtin,is_active) VALUES('联网','online','https://example.com','EMBED_SECRET','model',0,0)", [])?;
        for (key, value) in [("weather_qweather_api_key_encrypted", "WEATHER_SECRET"), ("weather_qweather_api_host", "PRIVATE_HOST"),
            ("image_generation_provider_1_api_key_encrypted", "IMAGE_SECRET"), ("web_search_provider_1_api_key_encrypted", "SEARCH_SECRET"),
            ("unknown_sensitive_key", "UNKNOWN_SECRET"), ("theme_mode", "dark")] {
            s.connection.execute("INSERT INTO settings VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
        }
        s.connection.execute("INSERT INTO settings VALUES('image_generation_config',?1)", [json!({"version":1,"revision":0,"nextId":2,"enabled":true,"activeId":"1","providers":[{"id":"1","name":"生图","protocol":"openai_images","baseUrl":"https://example.com","model":"image-model","apiKey":"HIDDEN_SECRET"}]}).to_string()])?;
        Ok(())
    }).unwrap();
    let (_, tables, raw) =
        unpack(&export(&runtime, request(PackageKind::Full, &[], false)).unwrap());
    let text = String::from_utf8(raw).unwrap();
    for secret in [
        "CHAT_SECRET",
        "EMBED_SECRET",
        "WEATHER_SECRET",
        "PRIVATE_HOST",
        "IMAGE_SECRET",
        "SEARCH_SECRET",
        "UNKNOWN_SECRET",
        "HIDDEN_SECRET",
        "api_key_encrypted",
    ] {
        assert!(!text.contains(secret), "{secret}");
    }
    assert!(tables["embedding_providers"]
        .iter()
        .all(|row| row["is_builtin"] == 0));
    assert!(text.contains("image-model"));
    assert!(text.contains("dark"));
}
#[test]
fn plugin_data_carries_sources_but_no_history_balance_or_effective_permissions() {
    let runtime = runtime();
    let (_, tables, raw) = unpack(
        &export(
            &runtime,
            request(PackageKind::PluginData, &["test.game"], false),
        )
        .unwrap(),
    );
    assert_eq!(tables.len(), 4);
    assert_eq!(
        tables["game_plugins"][0]["repository"],
        "https://github.com/example/game"
    );
    assert!(tables["game_plugins"][0]["package_path"].is_null());
    assert!(tables["game_permissions"]
        .iter()
        .all(|p| p["decision"] == "pending" && p["decided_at"].is_null()));
    assert_eq!(tables["game_plugin_data"][0]["value_json"], "{\"level\":3}");
    assert!(!String::from_utf8(raw).unwrap().contains("本机插件路径"));
    runtime
        .read(|s, _| {
            assert_eq!(
                s.connection.query_row(
                    "SELECT decision FROM game_permissions WHERE permission='wallet.modify'",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                "allowed"
            );
            Ok(())
        })
        .unwrap();
}
#[test]
fn missing_sold_accessory_reference_is_cleared_only_in_export_copy() {
    let runtime = runtime();
    runtime.read(|s, _| { s.connection.execute("INSERT INTO currency_ledger(id,delta,reason,ref_id,created_at) VALUES(11,5,'accessory_sell',999,?1)", [NOW])?; Ok(()) }).unwrap();
    let (manifest, tables, _) =
        unpack(&export(&runtime, request(PackageKind::Full, &[], false)).unwrap());
    assert!(tables["currency_ledger"]
        .iter()
        .find(|r| r["id"] == "11")
        .unwrap()["ref_id"]
        .is_null());
    assert!(manifest
        .warnings
        .iter()
        .any(|warning| warning.code == "MISSING_ACCESSORY_REFERENCE"));
    runtime
        .read(|s, _| {
            assert_eq!(
                s.connection.query_row(
                    "SELECT ref_id FROM currency_ledger WHERE id=11",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                999
            );
            Ok(())
        })
        .unwrap();
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "yanmew-export-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        // 仅删除本测试创建且仍处于临时目录内的唯一目录。
        if self.0.parent() == Some(std::env::temp_dir().as_path())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("yanmew-export-")
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
#[test]
fn referenced_images_are_hashed_once_and_missing_images_fall_back_without_db_changes() {
    let dir = Directory::new();
    let mut runtime = runtime();
    runtime.paths = Ok(crate::platform::contracts::AppPaths::portable(dir.0.clone()));
    fs::create_dir(dir.0.join("chat_background")).unwrap();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .save(dir.0.join("chat_background/test.png"))
        .unwrap();
    runtime
        .read(|s, _| {
            s.connection
                .execute("UPDATE personas SET chat_bg_file='test.png' WHERE id=2", [])?;
            s.connection.execute(
                "UPDATE accessories SET image_file='assets/accessories/missing.png' WHERE id=10",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    let (manifest, tables, _) =
        unpack(&export(&runtime, request(PackageKind::PersonaFull, &["2"], false)).unwrap());
    assert_eq!(manifest.files.len(), 1);
    assert!(tables["personas"][0]["chat_bg_file"]
        .as_str()
        .unwrap()
        .starts_with("files/"));
    assert!(tables["accessories"][0]["image_file"].is_null());
    assert!(manifest.warnings.iter().any(|w| w.code == "MISSING_IMAGE"));
    fs::remove_file(dir.0.join("chat_background/test.png")).unwrap();
    let (_, tables, _) = unpack(
        &export(
            &runtime,
            request(PackageKind::PersonaProfile, &["2"], false),
        )
        .unwrap(),
    );
    assert!(tables["personas"][0]["chat_bg_file"].is_null());
    runtime
        .read(|s, _| {
            assert_eq!(
                s.connection.query_row(
                    "SELECT chat_bg_file FROM personas WHERE id=2",
                    [],
                    |r| r.get::<_, String>(0)
                )?,
                "test.png"
            );
            Ok(())
        })
        .unwrap();
}
#[test]
fn invalid_selection_version_and_current_reference_fail_without_mutation() {
    let runtime = runtime();
    for input in [
        request(PackageKind::Full, &["1"], false),
        request(PackageKind::Session, &[], false),
        request(PackageKind::Session, &["2", "2"], false),
        request(PackageKind::Session, &["02"], false),
        request(PackageKind::PersonaProfile, &["2"], true),
    ] {
        assert_eq!(
            export(&runtime, input).unwrap_err().code,
            "VALIDATION_ERROR"
        );
    }
    assert_eq!(
        export(&runtime, request(PackageKind::Session, &["999"], false))
            .unwrap_err()
            .code,
        "NOT_FOUND"
    );
    runtime.read(|s, _| { s.connection.execute("INSERT INTO settings VALUES('current_session_id','999') ON CONFLICT(key) DO UPDATE SET value=excluded.value", [])?; Ok(()) }).unwrap();
    assert_eq!(
        export(&runtime, request(PackageKind::Full, &[], false))
            .unwrap_err()
            .code,
        "BACKUP_RELATION_INVALID"
    );
    runtime
        .read(|s, _| {
            s.connection.pragma_update(None, "user_version", 999)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        export(&runtime, request(PackageKind::Session, &["2"], false))
            .unwrap_err()
            .code,
        "BACKUP_VERSION_UNSUPPORTED"
    );
}

#[test]
fn more_than_sqlite_parameter_limit_exports_every_selected_profile() {
    let runtime = runtime();
    runtime.read(|s,_|{
        s.connection.execute("WITH RECURSIVE n(id) AS (VALUES(100) UNION ALL SELECT id+1 FROM n WHERE id<33099) INSERT INTO user_profiles SELECT id,'资料' || id,'原文',?1,?1 FROM n",[NOW])?;
        Ok(())
    }).unwrap();
    let ids = (100..=33099).map(|n| n.to_string()).collect();
    let output = export(
        &runtime,
        ExportRequest {
            kind: PackageKind::UserProfile,
            ids,
            include_sessions: false,
        },
    )
    .unwrap();
    let (manifest, tables, _) = unpack(&output);
    assert_eq!(manifest.record_counts["user_profiles"], 33000);
    assert_eq!(tables["user_profiles"].len(), 33000);
}

#[test]
fn cancelled_dialog_and_failed_publish_preserve_original_and_remove_temporary_files() {
    use crate::platform::contracts::FileTransferPort;
    struct Cancel;
    impl FileTransferPort for Cancel {
        fn save_backup(&self, _: &str) -> AppResult<Option<PathBuf>> {
            Ok(None)
        }
        fn publish(&self, _: &std::path::Path, _: &std::path::Path) -> AppResult<()> {
            panic!("取消后不能发布");
        }
    }
    struct Fail(PathBuf);
    impl FileTransferPort for Fail {
        fn save_backup(&self, _: &str) -> AppResult<Option<PathBuf>> {
            Ok(Some(self.0.clone()))
        }
        fn publish(&self, _: &std::path::Path, _: &std::path::Path) -> AppResult<()> {
            Err(AppError::new("BACKUP_PUBLISH_FAILED", "模拟发布失败"))
        }
    }
    let runtime = runtime();
    assert!(
        super::export(&runtime, request(PackageKind::Full, &[], false), &Cancel)
            .unwrap()
            .is_none()
    );
    let dir = Directory::new();
    let destination = dir.0.join("backup.zip");
    fs::write(&destination, b"existing backup").unwrap();
    let error = super::export(
        &runtime,
        request(PackageKind::Full, &[], false),
        &Fail(destination.clone()),
    )
    .unwrap_err();
    assert_eq!(error.code, "BACKUP_PUBLISH_FAILED");
    assert_eq!(fs::read(&destination).unwrap(), b"existing backup");
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn disk_snapshot_releases_main_database_before_compression_and_is_cleaned() {
    use std::io::{Seek, SeekFrom, Write};
    let runtime = runtime();
    let dir = Directory::new();
    let path = dir.0.join("source.db");
    runtime
        .read(|store, _| {
            store
                .connection
                .execute("VACUUM main INTO ?1", [path.to_str().unwrap()])?;
            store.connection = rusqlite::Connection::open(&path)?;
            store.connection.pragma_update(None, "foreign_keys", "ON")?;
            Ok(())
        })
        .unwrap();
    let input = request(PackageKind::Full, &[], false);
    let mut source = runtime
        .read(|store, _| {
            snapshot::independent(
                &runtime,
                &store.connection,
                &input,
                std::ffi::OsStr::new(".yanmew-backup-test.tmp"),
            )
        })
        .unwrap()
        .unwrap();
    struct MutatingWriter {
        inner: Cursor<Vec<u8>>,
        runtime: Runtime,
        changed: bool,
    }
    impl Write for MutatingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if !self.changed {
                // 压缩开始后主库仍能正常提交；导出快照不包含这条后来插入的消息。
                self.runtime.read(|store,_|{store.connection.execute("INSERT INTO messages(session_id,role,content,created_at) VALUES(2,'assistant','导出期间的新消息',?1)",[NOW])?;Ok(())}).unwrap();
                self.changed = true;
            }
            self.inner.write(bytes)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl Seek for MutatingWriter {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(pos)
        }
    }
    let writer = MutatingWriter {
        inner: Cursor::new(Vec::new()),
        runtime: runtime.clone(),
        changed: false,
    };
    let pinned = std::mem::take(&mut source.pinned);
    let (manifest, writer) =
        archive::write(&runtime, &source.connection, &source.scope, pinned, writer).unwrap();
    assert_eq!(manifest.record_counts["messages"], 4);
    let mut zip = ZipArchive::new(Cursor::new(writer.inner.into_inner())).unwrap();
    let mut json = String::new();
    zip.by_name("data.json")
        .unwrap()
        .read_to_string(&mut json)
        .unwrap();
    assert!(!json.contains("导出期间的新消息"));
    drop(source);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    runtime
        .read(|store, _| {
            assert_eq!(
                store.connection.query_row(
                    "SELECT count(*) FROM messages WHERE content='导出期间的新消息'",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
    drop(runtime);
}

#[test]
fn reminder_email_backup_excludes_secrets_binding_and_outbox() {
    let runtime=runtime();
    runtime.read(|s,p| {
        let input=crate::reminder_email::Input {revision:0,secret:crate::crypto::SecretUpdate::Set {value:"EMAIL-SECRET-MUST-NOT-EXPORT".into()},connection:crate::reminder_email::ConnectionConfig {email:"u@qq.com".into(),host:"smtp.qq.com".into(),port:465,encryption:"tls".into(),username:"u@qq.com".into(),from:"u@qq.com".into(),to:"u@qq.com".into()}};
        crate::reminder_email::save(&mut s.connection,p,input)?;
        s.connection.execute("INSERT INTO reminder_email_deliveries(kind,recipient,subject,body,config_revision,state,created_at) VALUES('due','u@qq.com','private subject','private body',1,'pending',?1)",[NOW])?;Ok(())
    }).unwrap();
    let backup=export(&runtime,request(PackageKind::Full,&[],false)).unwrap();
    let (_,tables,raw)=unpack(&backup);
    let text=String::from_utf8(raw).unwrap();assert!(!text.contains("EMAIL-SECRET-MUST-NOT-EXPORT"));assert!(!text.contains("reminder_email_secret"));assert!(!text.contains("private subject"));
    assert!(!tables.contains_key("reminder_email_deliveries"));assert!(!tables.contains_key("reminder_email_items"));
    let email=tables["settings"].iter().find(|r|r["key"]=="reminder_email_config").unwrap();
    let config:Value=serde_json::from_str(email["value"].as_str().unwrap()).unwrap();assert_eq!(config["enabled"],false);assert_eq!(config["verified"],false);assert_eq!(config["revision"],0);
}
