use super::*;
use crate::platform::contracts::{AppPaths, FileTransferPort};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
const NOW: &str = "2026-10-13T00:00:00Z";
#[path = "festival_tests.rs"]
mod festival_tests;

#[test]
fn onboarding_message_backup_roundtrip_preserves_edits_without_creating_extra_tutorial() {
    for kind in [PackageKind::Full, PackageKind::Session] {
        let source_dir = Directory::new(); let source = fixture(&source_dir.0);
        let edited = "欢迎使用颜喵 YanMew，这是用户修改后的上手指南。";
        source.read(|s, _| {
            let mut fresh = Connection::open_in_memory()?;
            assert!(crate::db::initialize(&mut fresh)?);
            assert_eq!(count(&fresh, "sessions"), 1); assert_eq!(count(&fresh, "messages"), 1);
            fresh.execute("UPDATE messages SET content=?1", [edited])?;
            s.connection = fresh; Ok(())
        }).unwrap();
        let backup = make(&source, &source_dir.0, kind, if kind == PackageKind::Session { &["1"] } else { &[] }, false);
        let target_dir = Directory::new(); let target = fixture(&target_dir.0);
        let preview = prepare(&target, &backup).unwrap().unwrap();
        confirm(&target, input(&preview)).unwrap();
        target.read(|s, _| {
            assert_eq!(count(&s.connection, "sessions"), 3);
            let (id, session): (i64, i64) = s.connection.query_row("SELECT id,session_id FROM messages WHERE content=?1", [edited], |r| Ok((r.get(0)?, r.get(1)?)))?;
            assert!(id > 1); assert!(session > 2);
            assert_eq!(s.connection.query_row("SELECT count(*) FROM messages WHERE content=?1", [edited], |r| r.get::<_, i64>(0))?, 1);
            let before = count(&s.connection, "messages");
            assert!(!crate::db::initialize(&mut s.connection)?);
            assert_eq!(count(&s.connection, "messages"), before);
            assert_eq!(s.connection.query_row("SELECT content FROM messages WHERE id=?1", [id], |r| r.get::<_, String>(0))?, edited);
            Ok(())
        }).unwrap();
    }
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "yanmew-import-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.parent() == Some(std::env::temp_dir().as_path())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("yanmew-import-test-")
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
struct Transfer(PathBuf);
impl FileTransferPort for Transfer {
    fn save_backup(&self, _: &str) -> AppResult<Option<PathBuf>> {
        Ok(Some(self.0.clone()))
    }
    fn open_backup(&self) -> AppResult<Option<PathBuf>> {
        Ok(Some(self.0.clone()))
    }
    fn publish(&self, from: &Path, to: &Path) -> AppResult<()> {
        fs::rename(from, to).map_err(|_| AppError::storage())
    }
}
fn fixture(root: &Path) -> Runtime {
    let mut runtime = super::super::tests::runtime();
    runtime.paths = Ok(AppPaths {
        data_root: root.into(),
    });
    runtime.read(|s,_| {
        s.connection.execute("UPDATE sessions SET created_at=?1,updated_at=?1 WHERE id=1",[NOW])?;
        s.connection.execute("UPDATE session_pet_states SET updated_at=?1",[NOW])?;
        s.connection.execute("UPDATE travels SET planned_return_at='2026-10-13T01:00:00Z',actual_return_at='2026-10-13T01:00:00Z'",[])?;
        s.connection.execute("INSERT INTO travel_journals(session_id,travel_id,title,content,created_at,updated_at) VALUES(2,10,'旅行标题','游记正文',?1,?1)",[NOW])?;
        Ok(())
    }).unwrap();
    runtime
}
fn make(source: &Runtime, root: &Path, kind: PackageKind, ids: &[&str], include: bool) -> Transfer {
    let path = root.join(format!("{}.zip", NEXT.fetch_add(1, Ordering::Relaxed)));
    let transfer = Transfer(path);
    let request = ExportRequest {
        kind,
        ids: ids.iter().map(|s| (*s).into()).collect(),
        include_sessions: include,
    };
    super::super::export(source, request, &transfer)
        .unwrap()
        .unwrap();
    transfer
}
fn input(p: &Preview) -> Confirm {
    Confirm {
        preview_id: p.preview_id.clone(),
        personas: p
            .personas
            .iter()
            .map(|n| Binding {
                source_id: n.source_id.clone(),
                target_id: n.suggested_target_id.clone(),
            })
            .collect(),
        profiles: p
            .profiles
            .iter()
            .map(|n| Binding {
                source_id: n.source_id.clone(),
                target_id: None,
            })
            .collect(),
        confirm_global_restore: p.kind == PackageKind::Full,
    }
}
fn count(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
#[test]
fn six_packages_append_closed_trees_and_restore_only_their_scope() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    source
        .read(|s, _| {
            s.connection
                .execute("UPDATE user_state SET coins=345", [])?;
            Ok(())
        })
        .unwrap();
    for (kind, ids, include, sessions) in [
        (PackageKind::Full, vec![], false, 2),
        (PackageKind::Session, vec!["2"], false, 1),
        (PackageKind::UserProfile, vec!["10"], false, 0),
        (PackageKind::UserProfile, vec!["10"], true, 1),
        (PackageKind::PersonaProfile, vec!["2"], false, 0),
        (PackageKind::PersonaFull, vec!["2"], false, 1),
        (PackageKind::PluginData, vec!["test.game"], false, 0),
    ] {
        let target_dir = Directory::new();
        let target = fixture(&target_dir.0);
        target
            .read(|s, _| {
                s.connection
                    .execute("UPDATE user_state SET coins=888", [])?;
                s.connection
                    .execute("UPDATE personas SET prompt='本机原人设' WHERE id=2", [])?;
                Ok(())
            })
            .unwrap();
        let transfer = make(&source, &source_dir.0, kind, &ids, include);
        let p = prepare(&target, &transfer).unwrap().unwrap();
        let imported = confirm(&target, input(&p)).unwrap();
        assert_eq!(imported.sessions, sessions);
        target.read(|s,_|{
            let c=&s.connection;assert_eq!(count(c,"sessions"),2+sessions as i64);
            assert_eq!(c.query_row("SELECT prompt FROM personas WHERE id=2",[],|r|r.get::<_,String>(0))?,"本机原人设");
            assert_eq!(count(c,"personas"),2); // 内置/同名人设复用，原定义保留。
            assert_eq!(c.query_row("SELECT coins FROM user_state",[],|r|r.get::<_,i64>(0))?,if kind==PackageKind::Full{345}else{888});
            assert_eq!(count(c,"game_runs"),1);assert_eq!(count(c,"currency_ledger"),1);
            if kind==PackageKind::PluginData {assert_eq!(c.query_row("SELECT count(*) FROM game_permissions WHERE decision='pending' AND decided_at IS NULL",[],|r|r.get::<_,i64>(0))?,2);}
            assert!(c.prepare("PRAGMA foreign_key_check")?.query([])?.next()?.is_none());
            if sessions>0 {
                let sid:i64=c.query_row("SELECT max(id) FROM sessions",[],|r|r.get(0))?;
                assert_eq!(c.query_row("SELECT title FROM sessions WHERE id=2",[],|r|r.get::<_,String>(0))?,"另一会话");
                assert_eq!(c.query_row("SELECT hunger_elapsed_ms FROM session_pet_states WHERE session_id=?1",[sid],|r|r.get::<_,i64>(0))?,1234);
                assert_eq!(c.query_row("SELECT intimacy FROM sessions WHERE id=?1",[sid],|r|r.get::<_,i64>(0))?,42);
                let (reply,parent):(i64,i64)=c.query_row("SELECT l.reply_to_id,parent.id FROM letters l JOIN letters parent ON parent.id=l.reply_to_id WHERE l.session_id=?1",[sid],|r|Ok((r.get(0)?,r.get(1)?)))?;assert_eq!(reply,parent);assert_ne!(parent,10);
                assert_eq!(c.query_row("SELECT count(*) FROM long_memory_sources x JOIN long_memories l ON l.id=x.long_memory_id JOIN short_memories m ON m.id=x.short_memory_id WHERE l.session_id=?1 AND m.session_id=?1",[sid],|r|r.get::<_,i64>(0))?,1);
                assert_eq!(c.query_row("SELECT count(*) FROM travel_journals WHERE session_id=?1",[sid],|r|r.get::<_,i64>(0))?,1);
            }
            if kind!=PackageKind::Full {assert_eq!(c.query_row("SELECT value FROM settings WHERE key='current_session_id'",[],|r|r.get::<_,String>(0))?,"1");}
            Ok(())
        }).unwrap();
        assert!(current_preview(&target, &p.preview_id).is_err());
    }
}
#[test]
fn repeated_import_new_persona_and_profile_reuse_never_replace_sessions() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let transfer = make(
        &source,
        &source_dir.0,
        PackageKind::PersonaFull,
        &["2"],
        false,
    );
    for _ in 0..2 {
        let p = prepare(&target, &transfer).unwrap().unwrap();
        confirm(&target, input(&p)).unwrap();
    }
    target
        .read(|s, _| {
            assert_eq!(count(&s.connection, "personas"), 2);
            assert_eq!(count(&s.connection, "sessions"), 4);
            Ok(())
        })
        .unwrap();
    let p = prepare(&target, &transfer).unwrap().unwrap();
    let mut choose = input(&p);
    choose.personas[0].target_id = None;
    confirm(&target, choose).unwrap();
    target
        .read(|s, _| {
            assert_eq!(count(&s.connection, "personas"), 3);
            assert_eq!(count(&s.connection, "sessions"), 5);
            Ok(())
        })
        .unwrap();
    let transfer = make(
        &source,
        &source_dir.0,
        PackageKind::UserProfile,
        &["10"],
        true,
    );
    let p = prepare(&target, &transfer).unwrap().unwrap();
    let mut choose = input(&p);
    choose.profiles[0].target_id = Some("10".into());
    confirm(&target, choose).unwrap();
    target
        .read(|s, _| {
            assert_eq!(
                s.connection.query_row(
                    "SELECT count(*) FROM sessions WHERE user_profile_id=10",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                2
            );
            Ok(())
        })
        .unwrap();
}
#[test]
fn stale_preview_and_sql_failure_preserve_existing_tree_and_allow_refresh() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let transfer = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
    let p = prepare(&target, &transfer).unwrap().unwrap();
    target
        .read(|s, _| {
            s.connection.execute(
                "UPDATE personas SET prompt='更新后的本机设定' WHERE id=2",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        confirm(&target, input(&p)).err().unwrap().code,
        "BACKUP_PREVIEW_CHANGED"
    );
    let p = refresh(&target, &p.preview_id).unwrap();
    target.read(|s,_|{s.connection.execute_batch("CREATE TRIGGER import_test_fail BEFORE INSERT ON messages BEGIN SELECT RAISE(ABORT,'simulated failure'); END;")?;Ok(())}).unwrap();
    assert!(confirm(&target, input(&p)).is_err());
    target
        .read(|s, _| {
            assert_eq!(count(&s.connection, "sessions"), 2);
            assert_eq!(count(&s.connection, "user_profiles"), 1);
            assert_eq!(count(&s.connection, "messages"), 4);
            s.connection
                .execute_batch("DROP TRIGGER import_test_fail;")?;
            Ok(())
        })
        .unwrap();
    confirm(&target, input(&p)).unwrap();
}
fn rewrite(path: &Path, change: impl FnOnce(&mut Manifest, &mut Value)) {
    let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut manifest: Manifest =
        serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    let mut data: Value = serde_json::from_reader(zip.by_name("data.json").unwrap()).unwrap();
    let mut files = Vec::new();
    for file in &manifest.files {
        let mut bytes = Vec::new();
        zip.by_name(&file.path)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        files.push((file.path.clone(), bytes));
    }
    drop(zip);
    change(&mut manifest, &mut data);
    let raw = serde_json::to_vec(&data).unwrap();
    manifest.data_size = raw.len() as u64;
    manifest.data_sha256 = super::super::archive::hash(&raw);
    manifest.record_counts = data
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_array().unwrap().len() as u64))
        .collect();
    let mut out = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    out.start_file("manifest.json", options).unwrap();
    serde_json::to_writer(&mut out, &manifest).unwrap();
    out.start_file("data.json", options).unwrap();
    out.write_all(&raw).unwrap();
    for (name, bytes) in files {
        out.start_file(name, options).unwrap();
        out.write_all(&bytes).unwrap();
    }
    out.finish().unwrap();
}
#[test]
fn invalid_versions_scope_references_and_file_paths_do_not_touch_target() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    for case in 0..4 {
        let t = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
        rewrite(&t.0, |manifest, data| match case {
            0 => manifest.schema_version = 999,
            1 => data["messages"][0]["session_id"] = Value::String("999".into()),
            2 => {
                data["sessions"][0]["id"] = Value::String("999".into());
            }
            _ => data["personas"][0]["chat_bg_file"] = Value::String("../../outside.png".into()),
        });
        assert!(prepare(&target, &t).is_err());
        target
            .read(|s, _| {
                assert_eq!(count(&s.connection, "sessions"), 2);
                Ok(())
            })
            .unwrap();
    }
}
#[test]
fn full_restore_maps_selection_clears_keys_and_preserves_global_amount_exactly() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    source
        .read(|s, _| {
            s.connection.execute(
                "UPDATE providers SET api_key_encrypted='private-source-key'",
                [],
            )?;
            s.connection
                .execute("UPDATE personas SET is_active=0 WHERE id=1", [])?;
            s.connection
                .execute("UPDATE personas SET is_active=1 WHERE id=2", [])?;
            s.connection.execute(
                "UPDATE settings SET value='2' WHERE key='current_session_id'",
                [],
            )?;
            s.connection.execute("UPDATE user_state SET coins=77", [])?;
            Ok(())
        })
        .unwrap();
    let t = make(&source, &source_dir.0, PackageKind::Full, &[], false);
    let p = prepare(&target, &t).unwrap().unwrap();
    let mut choose = input(&p);
    choose.confirm_global_restore = false;
    assert!(confirm(&target, choose).is_err());
    confirm(&target, input(&p)).unwrap();
    target
        .read(|s, _| {
            let c = &s.connection;
            assert_eq!(
                c.query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0))?,
                77
            );
            assert_eq!(
                c.query_row(
                    "SELECT count(*) FROM providers WHERE api_key_encrypted IS NOT NULL",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                0
            );
            let current: String = c.query_row(
                "SELECT value FROM settings WHERE key='current_session_id'",
                [],
                |r| r.get(0),
            )?;
            assert_ne!(current, "2");
            assert_eq!(
                c.query_row(
                    "SELECT persona_id FROM sessions WHERE id=?1",
                    [current],
                    |r| r.get::<_, i64>(0)
                )?,
                2
            );
            assert_eq!(
                c.query_row(
                    "SELECT count(*) FROM embedding_providers WHERE is_builtin=1",
                    [],
                    |r| r.get::<_, i64>(0)
                )?,
                1
            );
            Ok(())
        })
        .unwrap();
}
#[test]
fn images_are_published_before_references_and_rolled_back_on_failure() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let mut pixels = image::RgbaImage::from_pixel(256, 256, image::Rgba([0, 0, 0, 0]));
    for y in 96..160 {
        for x in 96..160 {
            pixels.put_pixel(x, y, image::Rgba([255, 100, 150, 255]));
        }
    }
    let image = image::DynamicImage::ImageRgba8(pixels);
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    let bytes = bytes.into_inner();
    let asset =
        crate::accessories::images::publish(source.paths.as_ref().unwrap(), &bytes).unwrap();
    let bg = crate::persona::background::publish(&source, "png", &bytes).unwrap();
    source
        .read(|s, _| {
            s.connection
                .execute("UPDATE accessories SET image_file=?1 WHERE id=10", [asset])?;
            s.connection
                .execute("UPDATE personas SET chat_bg_file=?1 WHERE id=2", [bg])?;
            Ok(())
        })
        .unwrap();
    let t = make(
        &source,
        &source_dir.0,
        PackageKind::PersonaFull,
        &["2"],
        false,
    );
    let p = prepare(&target, &t).unwrap().unwrap();
    let mut choose = input(&p);
    choose.personas[0].target_id = None;
    target.read(|s,_|{s.connection.execute_batch("CREATE TRIGGER import_test_fail BEFORE INSERT ON messages BEGIN SELECT RAISE(ABORT,'simulated'); END;")?;Ok(())}).unwrap();
    assert!(confirm(&target, choose).is_err());
    let assets = target_dir.0.join("assets/accessories");
    if assets.exists() {
        assert_eq!(
            fs::read_dir(assets)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|e| e.path().is_file())
                .count(),
            0
        );
    }
    target
        .read(|s, _| {
            s.connection
                .execute_batch("DROP TRIGGER import_test_fail;")?;
            Ok(())
        })
        .unwrap();
    let mut choose = input(&p);
    choose.personas[0].target_id = None;
    confirm(&target, choose).unwrap();
    target
        .read(|s, _| {
            let sid: i64 = s
                .connection
                .query_row("SELECT max(id) FROM sessions", [], |r| r.get(0))?;
            let filename: String = s.connection.query_row(
                "SELECT image_file FROM accessories WHERE session_id=?1",
                [sid],
                |r| r.get(0),
            )?;
            let data = crate::accessories::images::read(target.paths.as_ref().unwrap(), &filename)?;
            assert!(data.starts_with("data:image/png;base64,"));
            Ok(())
        })
        .unwrap();
}

#[test]
fn pending_rounds_stop_after_restore_and_partial_import_does_not_cancel_current_work() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    source.read(|s,_|{s.connection.execute("INSERT INTO chat_rounds(id,session_id,status,created_at) VALUES(12,2,'pending',?1)",[NOW])?;s.connection.execute("INSERT INTO messages(session_id,round_id,role,content,created_at) VALUES(2,12,'user','未完成的新消息',?1)",[NOW])?;Ok(())}).unwrap();
    let t = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
    let p = prepare(&target, &t).unwrap().unwrap();
    let lease = target.requests.start("chat:1".into(), 1).unwrap();
    confirm(&target, input(&p)).unwrap();
    lease.require_scope(&target.requests, "chat:1", 1).unwrap();
    target.read(|s,_|{assert_eq!(s.context_revision,0);assert_eq!(s.connection.query_row("SELECT count(*) FROM chat_rounds WHERE stop_reason='recovered_after_restart' AND status='interrupted' AND completed_at IS NOT NULL",[],|r|r.get::<_,i64>(0))?,1);Ok(())}).unwrap();
}
#[test]
fn default_persona_mapping_is_enforced_and_multiple_names_are_not_guessed() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let t = make(
        &source,
        &source_dir.0,
        PackageKind::PersonaProfile,
        &["1"],
        false,
    );
    let p = prepare(&target, &t).unwrap().unwrap();
    assert!(p.personas[0].is_builtin);
    let mut choose = input(&p);
    choose.personas[0].target_id = None;
    assert!(confirm(&target, choose).is_err());
    cancel(&target, &p.preview_id).unwrap();
    target
        .read(|s, _| {
            s.connection.execute(
                "INSERT INTO personas(name,prompt,initial_prompt) VALUES('另一人设','同名不同设定','初始设定')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    let t = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
    let p = prepare(&target, &t).unwrap().unwrap();
    assert!(p.personas[0].requires_choice);
    assert_eq!(p.personas[0].candidates.len(), 2);
    assert!(p.personas[0].suggested_target_id.is_none());
    let mut choose = input(&p);
    choose.personas[0].target_id = Some("2".into());
    confirm(&target, choose).unwrap();
}
#[test]
fn cancelling_preview_removes_owned_staging_and_hash_mismatch_is_rejected() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let t = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
    let p = prepare(&target, &t).unwrap().unwrap();
    assert_eq!(
        fs::read_dir(&target_dir.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with("backup-import-"))
            .count(),
        1
    );
    cancel(&target, &p.preview_id).unwrap();
    assert_eq!(
        fs::read_dir(&target_dir.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with("backup-import-"))
            .count(),
        0
    );
    rewrite(&t.0, |_, data| {
        data["short_memories"][0]["integration_state"] = Value::String("pending".into());
    });
    assert!(prepare(&target, &t).is_err());
    let t = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
    let mut zip = zip::ZipArchive::new(fs::File::open(&t.0).unwrap()).unwrap();
    let mut manifest: Manifest =
        serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    let mut data = Vec::new();
    zip.by_name("data.json")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    drop(zip);
    manifest.data_sha256 = "0".repeat(64);
    let mut out = zip::ZipWriter::new(fs::File::create(&t.0).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    out.start_file("manifest.json", options).unwrap();
    serde_json::to_writer(&mut out, &manifest).unwrap();
    out.start_file("data.json", options).unwrap();
    out.write_all(&data).unwrap();
    out.finish().unwrap();
    assert!(prepare(&target, &t).is_err());
}

#[test]
fn builtin_festival_links_restore_by_code_with_different_database_ids() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    source.read(|s, _| {
        let original: i64 = s.connection.query_row("SELECT id FROM festival_catalog WHERE code='solar_new_year'", [], |r| r.get(0))?;
        let tx = s.connection.transaction()?;
        tx.execute_batch("PRAGMA defer_foreign_keys=ON")?;
        tx.execute("UPDATE festival_catalog SET id=1000 WHERE id=?1", [original])?;
        tx.execute("UPDATE session_festivals SET festival_id=1000 WHERE festival_id=?1", [original])?;
        tx.commit()?;
        Ok(())
    }).unwrap();
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let backup = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
    let preview = prepare(&target, &backup).unwrap().unwrap();
    confirm(&target, input(&preview)).unwrap();
    target.read(|s, _| {
        let mapped: i64 = s.connection.query_row("SELECT festival_id FROM session_festivals WHERE session_id=(SELECT MAX(id) FROM sessions)", [], |r| r.get(0))?;
        assert_ne!(mapped, 1000);
        assert_eq!(s.connection.query_row("SELECT code FROM festival_catalog WHERE id=?1", [mapped], |r| r.get::<_, String>(0))?, "solar_new_year");
        assert_eq!(count(&s.connection, "festival_catalog"), 13);
        Ok(())
    }).unwrap();
}

#[test]
fn full_backup_remaps_first_party_pending_request_without_replaying_coins() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    source.read(|s,_| {
        s.connection.execute_batch("PRAGMA foreign_keys=OFF; BEGIN; UPDATE game_plugins SET plugin_id='com.yanmew.game.dice' WHERE plugin_id='test.game'; UPDATE game_permissions SET plugin_id='com.yanmew.game.dice'; UPDATE game_plugin_settings SET plugin_id='com.yanmew.game.dice'; UPDATE game_plugin_data SET plugin_id='com.yanmew.game.dice'; UPDATE game_runs SET plugin_id='com.yanmew.game.dice'; UPDATE currency_ledger SET plugin_id='com.yanmew.game.dice'; COMMIT; PRAGMA foreign_keys=ON;")?;
        let envelope=serde_json::json!({"version":1,"progress":{"level":3},"pending":{"request":{"request_id":"request-1","game_run_id":"10","delta":3,"reason":"奖励","details":{}},"after":{"level":4}}});
        s.connection.execute("DELETE FROM game_plugin_data",[])?;
        s.connection.execute("INSERT INTO game_plugin_data VALUES('com.yanmew.game.dice','state.main',?1,1,?2)",rusqlite::params![envelope.to_string(),NOW])?;
        s.connection.execute("UPDATE user_state SET coins=345",[])?;Ok(())
    }).unwrap();
    let t = make(&source, &source_dir.0, PackageKind::Full, &[], false);
    let p = prepare(&target, &t).unwrap().unwrap();
    confirm(&target, input(&p)).unwrap();
    target
        .read(|s, _| {
            let raw: String = s.connection.query_row(
                "SELECT value_json FROM game_plugin_data WHERE data_key='state.main'",
                [],
                |r| r.get(0),
            )?;
            let restored: Value = serde_json::from_str(&raw).unwrap();
            let request: crate::game_plugins::runs::WalletRequest =
                serde_json::from_value(restored["pending"]["request"].clone()).unwrap();
            assert_ne!(request.game_run_id, "10");
            let receipt = crate::game_plugins::runs::change_coins(
                &mut s.connection,
                "com.yanmew.game.dice",
                "1.0.0",
                "999",
                &request,
                chrono::DateTime::parse_from_rfc3339(NOW)
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            )?;
            assert!(receipt.duplicate);
            assert_eq!(receipt.balance, "345");
            assert_eq!(count(&s.connection, "currency_ledger"), 1);
            assert_eq!(restored["progress"]["level"], 3);
            assert_eq!(restored["pending"]["after"]["level"], 4);
            Ok(())
        })
        .unwrap();
}

#[test]
fn unknown_old_game_run_is_rejected_without_replaying_unsettled_wallet_request() {
    let dir=Directory::new();let runtime=fixture(&dir.0);
    runtime.read(|s,_| {
        let request:crate::game_plugins::runs::WalletRequest=serde_json::from_value(serde_json::json!({"request_id":"unsettled-old-request","game_run_id":"999999","delta":-5,"reason":"未结算操作","details":{}})).unwrap();
        let before:i64=s.connection.query_row("SELECT coins FROM user_state",[],|r|r.get(0))?;
        let error=crate::game_plugins::runs::change_coins(&mut s.connection,"test.game","1.0.0","10",&request,chrono::DateTime::parse_from_rfc3339(NOW).unwrap().with_timezone(&chrono::Utc)).err().unwrap();
        assert_eq!(error.code,"PLUGIN_PERMISSION_DENIED");assert_eq!(count(&s.connection,"currency_ledger"),1);assert_eq!(s.connection.query_row("SELECT coins FROM user_state",[],|r|r.get::<_,i64>(0))?,before);Ok(())
    }).unwrap();
}

#[test]
fn import_cleanup_preserves_other_operations_uncommitted_images() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let paths = [
        "chat_background/background-123-456.png",
        "assets/accessories/accessory-123-456-789.png",
        "assets/accessories/staging/accessory-123-456-789.png",
    ];
    for relative in paths {
        let path = target_dir.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"another operation owns this file").unwrap();
    }
    let backup = make(&source, &source_dir.0, PackageKind::Session, &["2"], false);
    let preview = prepare(&target, &backup).unwrap().unwrap();
    // 先覆盖失败清理，再使用同一预览重试成功。
    target.read(|s, _| {
        s.connection.execute_batch("CREATE TRIGGER reject_import BEFORE INSERT ON sessions BEGIN SELECT RAISE(ABORT,'test failure'); END;")?;
        Ok(())
    }).unwrap();
    assert!(confirm(&target, input(&preview)).is_err());
    for relative in paths { assert!(target_dir.0.join(relative).exists(), "{relative}"); }
    target.read(|s, _| { s.connection.execute_batch("DROP TRIGGER reject_import")?; Ok(()) }).unwrap();
    confirm(&target, input(&preview)).unwrap();
    for relative in paths {
        assert_eq!(fs::read(target_dir.0.join(relative)).unwrap(), b"another operation owns this file");
    }
}

#[test]
fn failed_full_import_keeps_chat_running_then_success_saves_partial_once() {
    let source_dir = Directory::new();
    let source = fixture(&source_dir.0);
    let target_dir = Directory::new();
    let target = fixture(&target_dir.0);
    let backup = make(&source, &source_dir.0, PackageKind::Full, &[], false);
    let preview = prepare(&target, &backup).unwrap().unwrap();
    let round_id = target.read(|s, _| {
        let round = crate::sessions::start_round(&mut s.connection, 1, 1, "当前问题")?;
        let id = round.round.id.clone();
        s.chat_requests.insert(1, crate::chat::RequestSnapshot {
            request_id: "active-before-import".into(), persona_id: "1".into(), session_id: "1".into(),
            context_revision: s.context_revision, round_id: id.clone(), seq: 1,
            partial_content: "正在生成的部分回复".into(), expression: None, notice: None,
            budget: None, web_activity: None, round, regenerating: false,
        });
        // 在中断状态写入之后令事务失败，验证整笔事务确实回滚。
        s.connection.execute_batch("CREATE TRIGGER reject_chat_stop AFTER UPDATE OF status ON chat_rounds WHEN NEW.stop_reason='context_changed' BEGIN SELECT RAISE(ABORT,'test failure'); END;")?;
        Ok(id)
    }).unwrap();
    let lease = target.requests.start("chat:1".into(), 0).unwrap();
    assert!(confirm(&target, input(&preview)).is_err());
    lease.require_scope(&target.requests, "chat:1", 0).unwrap();
    target.read(|s, _| {
        assert!(s.chat_requests.contains_key(&1));
        assert_eq!(s.context_revision, 0);
        assert_eq!(count(&s.connection, "sessions"), 2);
        let round = crate::sessions::round_snapshot(&s.connection, 1, 1, round_id.parse().unwrap())?;
        assert_eq!(round.round.status, crate::models::ChatRoundStatus::Pending);
        assert!(round.assistant_message.is_none());
        s.connection.execute_batch("DROP TRIGGER reject_chat_stop")?;
        Ok(())
    }).unwrap();
    confirm(&target, input(&preview)).unwrap();
    assert!(lease.require_scope(&target.requests, "chat:1", 0).is_err());
    target.read(|s, _| {
        assert!(!s.chat_requests.contains_key(&1));
        let round = crate::sessions::round_snapshot(&s.connection, 1, 1, round_id.parse().unwrap())?;
        assert_eq!(round.round.stop_reason.as_deref(), Some("context_changed"));
        assert_eq!(round.assistant_message.unwrap().content, "正在生成的部分回复");
        assert_eq!(s.connection.query_row("SELECT count(*) FROM messages WHERE round_id=?1 AND role='assistant'", [&round_id], |r| r.get::<_, i64>(0))?, 1);
        Ok(())
    }).unwrap();
}
