use super::*;
use std::{io::Cursor, path::PathBuf};

struct Fixture {
    root: PathBuf,
    paths: AppPaths,
    resources: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().canonicalize().unwrap();
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = base.join(format!("yanmew-presets-{}-{unique}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let paths = AppPaths::portable(root.join("data"));
        fs::create_dir(&paths.data_root).unwrap();
        let resources = root.join("sources");
        let mut image = image::RgbaImage::new(8, 8);
        for y in 2..6 { for x in 2..6 { image.put_pixel(x, y, image::Rgba([80, 140, 230, 255])); } }
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png).unwrap();
        for preset in PRESETS {
            fs::create_dir_all(resources.join(preset.key)).unwrap();
            for (category, _, _) in ARTWORKS {
                fs::write(resources.join(preset.key).join(format!("{category}.png")), png.get_ref()).unwrap();
            }
        }
        Self { root, paths, resources }
    }
    fn files(&self) -> usize {
        fs::read_dir(self.paths.data_root.join("assets/accessories")).map(|entries| entries.filter_map(Result::ok).filter(|e| e.path().extension().is_some_and(|v| v == "png")).count()).unwrap_or(0)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // 仅清理本测试在已解析临时根下创建、且带固定前缀的目录。
        if let Ok(base) = std::env::temp_dir().canonicalize() {
            if self.root.parent() == Some(base.as_path()) && self.root.file_name().is_some_and(|n| n.to_string_lossy().starts_with("yanmew-presets-")) {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }
}

#[test]
fn installs_four_editable_image_personas_without_changing_current_or_sessions() {
    let f = Fixture::new();
    let mut c = crate::db::test_database();
    let sessions: i64 = c.query_row("SELECT count(*) FROM sessions", [], |r| r.get(0)).unwrap();
    assert!(install_from(&mut c, &f.paths, &f.resources).unwrap());
    assert!(installed(&c).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM personas", [], |r| r.get::<_,i64>(0)).unwrap(), 5);
    assert_eq!(c.query_row("SELECT name FROM personas WHERE is_active=1", [], |r| r.get::<_,String>(0)).unwrap(), "电子小猫");
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE is_builtin=1", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
    assert_eq!(c.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_,i64>(0)).unwrap(), sessions);
    assert_eq!(f.files(), 36);
    for preset in PRESETS {
        let (id, current, initial): (i64, String, String) = c.query_row("SELECT id,prompt,initial_prompt FROM personas WHERE name=?1 AND is_builtin=0 AND pet_image_enabled=1", [preset.name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(current, initial);
        let library = crate::pet_creation::library(&c, id).unwrap();
        assert!(library.enabled);
        assert_eq!(library.items.len(), 9);
        assert!(library.items.iter().any(|a| a.kind == "base"));
        assert!(library.items.iter().any(|a| a.category == "standby" && a.enabled));
        for artwork in library.items {
            assert!(artwork.preview_file.is_none());
            let path = crate::accessories::images::managed_file(&f.paths, artwork.image_file.as_deref().unwrap()).unwrap();
            crate::accessories::images::validate_png(&fs::read(path).unwrap()).unwrap();
        }
    }
}

#[test]
fn same_named_user_persona_is_preserved_and_edits_or_deletions_never_reseed() {
    let f = Fixture::new();
    let mut c = crate::db::test_database();
    c.execute("INSERT INTO personas(name,prompt,initial_prompt) VALUES('GPT','用户自己的设定','原始设定')", []).unwrap();
    let user_id = c.last_insert_rowid();
    install_from(&mut c, &f.paths, &f.resources).unwrap();
    assert_eq!(c.query_row("SELECT prompt FROM personas WHERE id=?1", [user_id], |r| r.get::<_,String>(0)).unwrap(), "用户自己的设定");
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='GPT'", [], |r| r.get::<_,i64>(0)).unwrap(), 2);
    c.execute("UPDATE personas SET name='我的大小姐',prompt='改过的设定' WHERE name='Claude'", []).unwrap();
    c.execute("DELETE FROM personas WHERE name='Gemini'", []).unwrap();
    let files = f.files();
    // 安装完成后不再读取随包文件；删除素材包也不能重置用户的选择。
    assert!(!install_from(&mut c, &f.paths, &f.root.join("missing")).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='Gemini'", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(c.query_row("SELECT prompt FROM personas WHERE name='我的大小姐'", [], |r| r.get::<_,String>(0)).unwrap(), "改过的设定");
    assert_eq!(f.files(), files);
}

#[test]
fn write_failure_rolls_back_all_rows_marker_and_new_files_then_allows_retry() {
    let f = Fixture::new();
    let mut c = crate::db::test_database();
    c.execute_batch("CREATE TRIGGER reject_preset BEFORE INSERT ON personas WHEN NEW.name='Claude' BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(install_from(&mut c, &f.paths, &f.resources).is_err());
    assert!(!installed(&c).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM personas", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    assert_eq!(f.files(), 0);
    c.execute_batch("DROP TRIGGER reject_preset;").unwrap();
    assert!(install_from(&mut c, &f.paths, &f.resources).unwrap());
    assert_eq!(f.files(), 36);
}

#[test]
fn invalid_or_missing_image_does_not_create_any_personas_or_files() {
    let f = Fixture::new();
    let mut c = crate::db::test_database();
    fs::write(f.resources.join("deepseek/chat.png"), b"not a PNG").unwrap();
    assert_eq!(install_from(&mut c, &f.paths, &f.resources).unwrap_err().code, "PRESET_PERSONA_RESOURCE_ERROR");
    assert!(!installed(&c).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM personas", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
    assert_eq!(f.files(), 0);
    assert!(install_from(&mut c, &f.paths, &f.root.join("missing")).is_err());
}

#[test]
fn bundled_images_install_and_survive_database_reopen() {
    let f = Fixture::new();
    let (mut c, created) = crate::db::open(&f.paths).unwrap();
    assert!(created);
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/personas");
    assert!(install_from(&mut c, &f.paths, &source).unwrap());
    assert_eq!(f.files(), 36);
    drop(c);
    let (mut reopened, created) = crate::db::open(&f.paths).unwrap();
    assert!(!created);
    assert!(!install_from(&mut reopened, &f.paths, &f.root.join("missing")).unwrap());
    for preset in PRESETS {
        let id: i64 = reopened.query_row("SELECT id FROM personas WHERE name=?1", [preset.name], |r| r.get(0)).unwrap();
        let library = crate::pet_creation::library(&reopened, id).unwrap();
        assert!(library.enabled);
        assert_eq!(library.items.len(), 9);
        for artwork in library.items {
            let file = crate::accessories::images::managed_file(&f.paths, artwork.image_file.as_deref().unwrap()).unwrap();
            crate::accessories::images::validate_png(&fs::read(file).unwrap()).unwrap();
        }
    }
}
