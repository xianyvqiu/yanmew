use super::*;
use std::{io::Cursor, path::PathBuf};

#[test]
fn docking_upgrade_preserves_existing_images_and_does_not_reseed_deleted_actions() {
    let f=Fixture::new();let mut c=crate::db::test_database();install_from(&mut c,&f.paths,&f.resources).unwrap();
    let gpt:i64=c.query_row("SELECT id FROM personas WHERE name='GPT'",[],|r|r.get(0)).unwrap();
    let base:String=c.query_row("SELECT image_file FROM pet_artworks WHERE persona_id=?1 AND kind='base'",[gpt],|r|r.get(0)).unwrap();
    c.execute("DELETE FROM personas WHERE name='Gemini'",[]).unwrap();
    c.execute("DELETE FROM settings WHERE key=?1",[DOCK_SEED_KEY]).unwrap();
    c.execute("UPDATE pet_artworks SET description='用户的使用说明',enabled=0 WHERE persona_id=?1 AND category='peek_left'",[gpt]).unwrap();
    c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND category IN('peek_right','rest_sit','rest_lie')",[gpt]).unwrap();
    install_docking_from(&mut c,&f.paths,&f.resources).unwrap();
    assert_eq!(c.query_row("SELECT image_file FROM pet_artworks WHERE persona_id=?1 AND kind='base'",[gpt],|r|r.get::<_,String>(0)).unwrap(),base);
    assert_eq!(c.query_row("SELECT description,enabled FROM pet_artworks WHERE persona_id=?1 AND category='peek_left'",[gpt],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).unwrap(),("用户的使用说明".into(),0));
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1",[gpt],|r|r.get::<_,i64>(0)).unwrap(),31);
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='Gemini'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND category='rest_lie'",[gpt]).unwrap();
    install_docking_from(&mut c,&f.paths,&f.resources).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='rest_lie'",[gpt],|r|r.get::<_,i64>(0)).unwrap(),0);
}

struct Fixture {
    root: PathBuf,
    paths: AppPaths,
    resources: PathBuf,
}

#[test]
fn toss_categories_seed_once_preserving_edits_and_deleted_personas(){
 let f=Fixture::new();let mut c=crate::db::test_database();install_from(&mut c,&f.paths,&f.resources).unwrap();
 c.execute("DELETE FROM settings WHERE key=?1",[TOSS_SEED_KEY]).unwrap();
 c.execute("UPDATE pet_artworks SET name='用户的上抛',description='专属反应',enabled=0 WHERE persona_id=(SELECT id FROM personas WHERE name='GPT') AND category='toss_up'",[]).unwrap();
 c.execute("DELETE FROM pet_artworks WHERE persona_id=(SELECT id FROM personas WHERE name='Claude') AND category IN ('toss_up','toss_down','landing')",[]).unwrap();
 c.execute("DELETE FROM personas WHERE name='Gemini'",[]).unwrap();
 install_categories_from(&mut c,&f.paths,&f.resources,TOSS_SEED_KEY,TOSS_ARTWORKS).unwrap();
 assert_eq!(c.query_row("SELECT description,enabled FROM pet_artworks WHERE name='用户的上抛'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).unwrap(),("专属反应".into(),0));
 let id:i64=c.query_row("SELECT id FROM personas WHERE name='Claude'",[],|r|r.get(0)).unwrap();
 assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category IN ('toss_up','toss_down','landing')",[id],|r|r.get::<_,i64>(0)).unwrap(),3);
 assert!(crate::pet_creation::chat_choices(&c,id).unwrap().unwrap().iter().any(|a|["toss_up","toss_down","landing"].contains(&a.category.as_str())));
 assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='Gemini'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND category='landing'",[id]).unwrap();
 install_categories_from(&mut c,&f.paths,&f.resources,TOSS_SEED_KEY,TOSS_ARTWORKS).unwrap();
 assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='landing'",[id],|r|r.get::<_,i64>(0)).unwrap(),0);
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
            for (category, _, _) in ARTWORKS.iter().chain(CHAT_EMOTIONS.iter()).chain(LIFT_ARTWORKS.iter()).chain(ERROR_ARTWORKS.iter()).chain(TOSS_ARTWORKS.iter()).chain(WAIT_PET_ARTWORKS.iter()).chain(SURPRISE_ARTWORKS.iter()) {
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
fn installs_four_editable_image_personas_with_default_sessions_preserving_current() {
    let f = Fixture::new();
    let mut c = crate::db::test_database();
    let sessions: i64 = c.query_row("SELECT count(*) FROM sessions", [], |r| r.get(0)).unwrap();
    assert!(install_from(&mut c, &f.paths, &f.resources).unwrap());
    assert!(installed(&c).unwrap());
    assert_eq!(c.query_row("SELECT count(*) FROM personas", [], |r| r.get::<_,i64>(0)).unwrap(), 5);
    assert_eq!(c.query_row("SELECT name FROM personas WHERE is_active=1", [], |r| r.get::<_,String>(0)).unwrap(), "电子小猫");
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE is_builtin=1", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
    assert_eq!(c.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_,i64>(0)).unwrap(), sessions+4);
    assert_eq!(f.files(), 124);
    for preset in PRESETS {
        let (id, current, initial): (i64, String, String) = c.query_row("SELECT id,prompt,initial_prompt FROM personas WHERE name=?1 AND is_builtin=0 AND pet_image_enabled=1", [preset.name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(current, initial);
        assert_eq!(current,preset.food_prompt());
        assert!(preset.matches_prompt(preset.prompt));
        assert!(preset.matches_prompt(&current));
        let colors: (String, String, String) = c.query_row("SELECT color,chat_user_text_color,chat_assistant_text_color FROM personas WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!(colors, (preset.color.into(), preset.color.into(), preset.color.into()));
        let library = crate::pet_creation::library(&c, id).unwrap();
        assert!(library.enabled);
        assert_eq!(library.items.len(), 31);
        let chat_choices=crate::pet_creation::chat_choices(&c,id).unwrap().unwrap();
        assert_eq!(chat_choices.len(),31);
        assert!(chat_choices.iter().any(|a|matches!(a.category.as_str(),"peek_left"|"peek_right"|"rest_sit"|"rest_lie")));
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
    assert_eq!(f.files(), 124);
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
    assert_eq!(f.files(), 124);
    drop(c);
    let (mut reopened, created) = crate::db::open(&f.paths).unwrap();
    assert!(!created);
    assert!(!install_from(&mut reopened, &f.paths, &f.root.join("missing")).unwrap());
    for preset in PRESETS {
        let id: i64 = reopened.query_row("SELECT id FROM personas WHERE name=?1", [preset.name], |r| r.get(0)).unwrap();
        let library = crate::pet_creation::library(&reopened, id).unwrap();
        assert!(library.enabled);
        assert_eq!(library.items.len(), 31);
        struct First;impl crate::pet::RandomSource for First{fn below(&mut self,_:u64)->AppResult<u64>{Ok(0)}}
        for (reaction,words)in [(0,&["开心","喜悦"] as &[_]),(1,&["惊讶"]),(2,&["害羞"])] {
            let expression=crate::pet_creation::select_click(&reopened,id,reaction,&mut First).unwrap().unwrap();
            let artwork=library.items.iter().find(|a|crate::pet_creation::marker(&a.id)==expression).unwrap();
            assert!(words.iter().any(|word|artwork.name.contains(word)),"{}的反应{}选到了{}",preset.name,reaction,artwork.name);
        }
        for artwork in library.items {
            let file = crate::accessories::images::managed_file(&f.paths, artwork.image_file.as_deref().unwrap()).unwrap();
            crate::accessories::images::validate_png(&fs::read(file).unwrap()).unwrap();
        }
    }
}

#[test]
fn surprise_upgrade_preserves_custom_artwork_and_deleted_personas_and_never_readds_deleted_emotion(){
    let f=Fixture::new();let mut c=crate::db::test_database();install_from(&mut c,&f.paths,&f.resources).unwrap();
    c.execute("DELETE FROM settings WHERE key=?1",[SURPRISE_SEED_KEY]).unwrap();
    let gpt:i64=c.query_row("SELECT id FROM personas WHERE name='GPT'",[],|r|r.get(0)).unwrap();
    c.execute("UPDATE pet_artworks SET description='用户说明',enabled=0,revision=8 WHERE persona_id=?1 AND name='惊讶'",[gpt]).unwrap();
    let original:String=c.query_row("SELECT image_file FROM pet_artworks WHERE persona_id=?1 AND name='惊讶'",[gpt],|r|r.get(0)).unwrap();
    c.execute("DELETE FROM pet_artworks WHERE name='惊讶' AND persona_id<>?1",[gpt]).unwrap();
    c.execute("DELETE FROM personas WHERE name='Gemini'",[]).unwrap();
    c.execute("UPDATE pet_artworks SET image_file='custom-base.png' WHERE kind='base' AND persona_id=(SELECT id FROM personas WHERE name='DeepSeek')",[]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,SURPRISE_SEED_KEY,SURPRISE_ARTWORKS).unwrap();
    let actual:(String,bool,i64,String)=c.query_row("SELECT description,enabled,revision,image_file FROM pet_artworks WHERE persona_id=?1 AND name='惊讶'",[gpt],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
    assert_eq!(actual,("用户说明".into(),false,8,original));
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='Gemini'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE name='惊讶' AND persona_id=(SELECT id FROM personas WHERE name='DeepSeek')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    let claude:i64=c.query_row("SELECT id FROM personas WHERE name='Claude'",[],|r|r.get(0)).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE name='惊讶' AND category='chat' AND persona_id=?1",[claude],|r|r.get::<_,i64>(0)).unwrap(),1);
    c.execute("DELETE FROM pet_artworks WHERE name='惊讶' AND persona_id=?1",[claude]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,SURPRISE_SEED_KEY,SURPRISE_ARTWORKS).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE name='惊讶' AND persona_id=?1",[claude],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn walking_upgrade_preserves_disabled_custom_action_and_does_not_reseed() {
    let f=Fixture::new();let mut c=crate::db::test_database();install_from(&mut c,&f.paths,&f.resources).unwrap();
    let id:i64=c.query_row("SELECT id FROM personas WHERE name='GPT'",[],|r|r.get(0)).unwrap();
    c.execute("DELETE FROM settings WHERE key=?1",[WALK_SEED_KEY]).unwrap();
    c.execute("UPDATE pet_artworks SET description='用户保留',enabled=0 WHERE persona_id=?1 AND category='walk_left'",[id]).unwrap();
    c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND category='walk_right'",[id]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,WALK_SEED_KEY,&ARTWORKS[16..]).unwrap();
    assert_eq!(c.query_row("SELECT description,enabled FROM pet_artworks WHERE persona_id=?1 AND category='walk_left'",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).unwrap(),("用户保留".into(),0));
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='walk_right'",[id],|r|r.get::<_,i64>(0)).unwrap(),1);
    c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND category='walk_right'",[id]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,WALK_SEED_KEY,&ARTWORKS[16..]).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='walk_right'",[id],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn emotions_upgrade_preserves_user_edits_and_deleted_roles_and_never_reseeds() {
    let f=Fixture::new();let mut c=crate::db::test_database();install_from(&mut c,&f.paths,&f.resources).unwrap();
    let id:i64=c.query_row("SELECT id FROM personas WHERE name='GPT'",[],|r|r.get(0)).unwrap();
    c.execute("DELETE FROM settings WHERE key=?1",[EMOTION_SEED_KEY]).unwrap();
    c.execute("UPDATE pet_artworks SET description='用户保留',enabled=0 WHERE persona_id=?1 AND name='开心'",[id]).unwrap();
    c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND name='生气'",[id]).unwrap();
    c.execute("DELETE FROM personas WHERE name='Gemini'",[]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,EMOTION_SEED_KEY,CHAT_EMOTIONS).unwrap();
    assert_eq!(c.query_row("SELECT description,enabled FROM pet_artworks WHERE persona_id=?1 AND name='开心'",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).unwrap(),("用户保留".into(),0));
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND name='生气'",[id],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='Gemini'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND name='生气'",[id]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,EMOTION_SEED_KEY,CHAT_EMOTIONS).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND name='生气'",[id],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn error_upgrade_preserves_customization_and_deleted_roles_and_never_reseeds() {
    let f=Fixture::new();let mut c=crate::db::test_database();install_from(&mut c,&f.paths,&f.resources).unwrap();
    c.execute("DELETE FROM settings WHERE key=?1",[ERROR_SEED_KEY]).unwrap();
    c.execute("UPDATE pet_artworks SET name='用户的报错图',description='用户说明',enabled=0 WHERE persona_id=(SELECT id FROM personas WHERE name='GPT') AND category='error'",[]).unwrap();
    c.execute("DELETE FROM pet_artworks WHERE persona_id=(SELECT id FROM personas WHERE name='Claude') AND category='error'",[]).unwrap();
    c.execute("DELETE FROM personas WHERE name='Gemini'",[]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,ERROR_SEED_KEY,ERROR_ARTWORKS).unwrap();
    assert_eq!(c.query_row("SELECT name,description,enabled FROM pet_artworks WHERE persona_id=(SELECT id FROM personas WHERE name='GPT') AND category='error'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?))).unwrap(),("用户的报错图".into(),"用户说明".into(),0));
    let id:i64=c.query_row("SELECT id FROM personas WHERE name='Claude'",[],|r|r.get(0)).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='error'",[id],|r|r.get::<_,i64>(0)).unwrap(),1);
    assert!(crate::pet_creation::chat_choices(&c,id).unwrap().unwrap().iter().any(|a|a.category=="error"));
    assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='Gemini'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND category='error'",[id]).unwrap();
    install_categories_from(&mut c,&f.paths,&f.resources,ERROR_SEED_KEY,ERROR_ARTWORKS).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='error'",[id],|r|r.get::<_,i64>(0)).unwrap(),0);
}

#[test]
fn wait_pet_category_seed_once_preserving_edits_and_deleted_personas(){
 let f=Fixture::new();let mut c=crate::db::test_database();install_from(&mut c,&f.paths,&f.resources).unwrap();
 c.execute("DELETE FROM settings WHERE key=?1",[WAIT_PET_SEED_KEY]).unwrap();
 c.execute("UPDATE pet_artworks SET name='用户的等摸摸',description='专属反应',enabled=0 WHERE persona_id=(SELECT id FROM personas WHERE name='GPT') AND category='wait_pet'",[]).unwrap();
 c.execute("DELETE FROM pet_artworks WHERE persona_id=(SELECT id FROM personas WHERE name='Claude') AND category='wait_pet'",[]).unwrap();
 c.execute("DELETE FROM personas WHERE name='Gemini'",[]).unwrap();
 install_categories_from(&mut c,&f.paths,&f.resources,WAIT_PET_SEED_KEY,WAIT_PET_ARTWORKS).unwrap();
 assert_eq!(c.query_row("SELECT description,enabled FROM pet_artworks WHERE name='用户的等摸摸'",[],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).unwrap(),("专属反应".into(),0));
 let id:i64=c.query_row("SELECT id FROM personas WHERE name='Claude'",[],|r|r.get(0)).unwrap();
 assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='wait_pet'",[id],|r|r.get::<_,i64>(0)).unwrap(),1);
 assert!(crate::pet_creation::chat_choices(&c,id).unwrap().unwrap().iter().any(|a|["wait_pet"].contains(&a.category.as_str())));
 assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE name='Gemini'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 c.execute("DELETE FROM pet_artworks WHERE persona_id=?1 AND category='wait_pet'",[id]).unwrap();
 install_categories_from(&mut c,&f.paths,&f.resources,WAIT_PET_SEED_KEY,WAIT_PET_ARTWORKS).unwrap();
 assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks WHERE persona_id=?1 AND category='wait_pet'",[id],|r|r.get::<_,i64>(0)).unwrap(),0);
}
