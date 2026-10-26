use super::*;
fn input(kind: &str) -> Input {
    Input {
        kind: kind.into(),
        name: "小猫".into(),
        prompt: "开心挥手".into(),
        description: "开心时".into(),
        category: "chat".into(),
        enabled: true,
    }
}

fn ready_standby(c: &mut Connection) -> Artwork {
    let mut data=input("expression");
    data.category="standby".into();
    let a=save(c,1,None,None,data).unwrap();
    c.execute("UPDATE pet_artworks SET image_file='standby.png' WHERE id=?1",[&a.id]).unwrap();
    get(c,1,a.id.parse().unwrap()).unwrap()
}

#[test]
fn image_mode_requires_accepted_enabled_standby_and_protects_last_one() {
    let mut c=crate::db::test_database();
    let base=save(&mut c,1,None,None,input("base")).unwrap();
    c.execute("UPDATE pet_artworks SET image_file='base.png' WHERE id=?1",[&base.id]).unwrap();
    assert_eq!(set_enabled(&c,1,true,library(&c,1).unwrap().revision).unwrap_err().field.as_deref(),Some("standby"));
    let standby=ready_standby(&mut c);
    let id=standby.id.parse().unwrap();
    c.execute("UPDATE pet_artworks SET image_file=NULL,preview_file='standby.png' WHERE id=?1",[id]).unwrap();
    assert!(set_enabled(&c,1,true,library(&c,1).unwrap().revision).is_err());
    finish(&mut c,1,id,0,"accept").unwrap();
    set_enabled(&c,1,true,library(&c,1).unwrap().revision).unwrap();
    let before=library(&c,1).unwrap().revision;
    for (category,enabled) in [("standby",false),("chat",true)] {
        let mut data=input("expression");data.category=category.into();data.enabled=enabled;
        assert!(save(&mut c,1,Some(id),Some(1),data).is_err());
        assert_eq!(get(&c,1,id).unwrap().revision,1);
    }
    assert!(finish(&mut c,1,id,1,"delete").is_err());
    assert_eq!(library(&c,1).unwrap().revision,before);
    let second=ready_standby(&mut c);
    finish(&mut c,1,id,1,"delete").unwrap();
    assert!(library(&c,1).unwrap().enabled);
    set_enabled(&c,1,false,library(&c,1).unwrap().revision).unwrap();
    finish(&mut c,1,second.id.parse().unwrap(),0,"delete").unwrap();
}
#[test]
fn creator_is_persona_scoped_and_revision_protected() {
    let mut c = crate::db::test_database();
    let base = save(&mut c, 1, None, None, input("base")).unwrap();
    assert!(set_enabled(&c, 1, true, library(&c, 1).unwrap().revision).is_err());
    let edited = save(
        &mut c,
        1,
        Some(base.id.parse().unwrap()),
        Some(base.revision),
        input("base"),
    )
    .unwrap();
    assert!(save(
        &mut c,
        1,
        Some(base.id.parse().unwrap()),
        Some(base.revision),
        input("base")
    )
    .is_err());
    assert!(get(&c, 2, base.id.parse().unwrap()).is_err());
    assert_eq!(edited.revision, 1);
    assert!(save(&mut c, 1, None, None, input("base")).is_err());
}
#[test]
fn accept_discard_and_delete_keep_formal_image_separate() {
    let mut c = crate::db::test_database();
    let a = save(&mut c, 1, None, None, input("base")).unwrap();
    let id = a.id.parse().unwrap();
    c.execute(
        "UPDATE pet_artworks SET image_file='formal',preview_file='preview' WHERE id=?1",
        [id],
    )
    .unwrap();
    let removed = finish(&mut c, 1, id, 0, "discard").unwrap();
    assert_eq!(removed, vec!["preview"]);
    assert_eq!(
        get(&c, 1, id).unwrap().image_file.as_deref(),
        Some("formal")
    );
    c.execute(
        "UPDATE pet_artworks SET preview_file='new' WHERE id=?1",
        [id],
    )
    .unwrap();
    finish(&mut c, 1, id, 1, "accept").unwrap();
    assert_eq!(get(&c, 1, id).unwrap().image_file.as_deref(), Some("new"));
    ready_standby(&mut c);
    set_enabled(&c, 1, true, library(&c, 1).unwrap().revision).unwrap();
    finish(&mut c, 1, id, 2, "delete").unwrap();
    assert!(!library(&c, 1).unwrap().enabled);
}
fn png(opaque: bool) -> Vec<u8> {
    let mut image = image::RgbaImage::new(8, 8);
    if opaque {
        image
            .pixels_mut()
            .for_each(|p| *p = image::Rgba([100, 0, 0, 255]));
    } else {
        image.put_pixel(0, 0, image::Rgba([100, 0, 0, 255]));
    }
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}
#[test]
fn transparent_input_keeps_frame_and_rejects_opaque() {
    let out = normalize(&png(false)).unwrap();
    crate::accessories::images::validate_png(&out).unwrap();
    let decoded = image::load_from_memory(&out).unwrap().into_rgba8();
    assert!(decoded.get_pixel(2, 2)[3] > 0);
    assert_eq!(decoded.get_pixel(200, 200)[3], 0);
    assert!(normalize(&png(true)).is_err());
}
#[test]
fn upgrade_v11_keeps_old_mode_and_default_accessory_position() {
    let mut c = crate::db::test_database();
    c.execute_batch("ALTER TABLE accessories DROP COLUMN render_layer;ALTER TABLE accessories DROP COLUMN layer_order;DROP TABLE pet_artworks; ALTER TABLE personas DROP COLUMN pet_image_enabled; ALTER TABLE personas DROP COLUMN pet_image_revision; ALTER TABLE accessories DROP COLUMN position_x; ALTER TABLE accessories DROP COLUMN position_y; ALTER TABLE accessories DROP COLUMN scale; PRAGMA user_version=11;").unwrap();
    crate::db::initialize(&mut c).unwrap();
    assert!(!library(&c, 1).unwrap().enabled);
    assert_eq!(library(&c, 1).unwrap().items.len(), 0);
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        crate::import_export::SCHEMA_VERSION
    );
    crate::db::initialize(&mut c).unwrap();
}
#[test]
fn image_selection_only_uses_accepted_and_enabled_rows() {
    struct Random;
    impl crate::pet::RandomSource for Random {
        fn below(&mut self, _: u64) -> AppResult<u64> {
            Ok(0)
        }
    }
    let mut c = crate::db::test_database();
    let a = save(&mut c, 1, None, None, input("base")).unwrap();
    c.execute(
        "UPDATE pet_artworks SET image_file='base' WHERE id=?1",
        [&a.id],
    )
    .unwrap();
    ready_standby(&mut c);
    set_enabled(&c, 1, true, library(&c, 1).unwrap().revision).unwrap();
    let b = save(&mut c, 1, None, None, input("expression")).unwrap();
    let mut random = Random;
    assert_eq!(
        select(&c, 1, "chat", None, &mut random).unwrap(),
        Some(marker(&a.id))
    );
    c.execute(
        "UPDATE pet_artworks SET image_file='expression' WHERE id=?1",
        [&b.id],
    )
    .unwrap();
    assert_eq!(
        select(&c, 1, "chat", None, &mut random).unwrap(),
        Some(marker(&b.id))
    );
    c.execute("UPDATE pet_artworks SET enabled=0 WHERE id=?1", [&b.id])
        .unwrap();
    assert_eq!(
        select(&c, 1, "chat", Some(&marker(&b.id)), &mut random).unwrap(),
        Some(marker(&a.id))
    );
}

#[test]
fn accessory_placement_keeps_wallet_and_worn_state() {
    let c = crate::db::test_database();
    c.execute("INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(1,'会话','2026-10-16','2026-10-16')",[]).unwrap();
    c.execute("INSERT INTO accessories(session_id,name,display_text,description,source,created_at,is_worn) VALUES(1,'帽子','☆','帽子','crafted','2026-10-16',1)",[]).unwrap();
    let coins = crate::currency::get_balance(&c).unwrap();
    let updated = crate::accessories::placement(&c, 1, 1, Some(125), Some(-500), 1500).unwrap();
    assert_eq!(
        (updated.position_x, updated.position_y, updated.scale),
        (Some(125), Some(-500), 1500)
    );
    assert!(updated.is_worn);
    assert_eq!(crate::currency::get_balance(&c).unwrap(), coins);
    assert!(crate::accessories::placement(&c, 1, 1, Some(1600), Some(0), 1000).is_err());
    assert!(crate::accessories::placement(&c, 1, 1, Some(0), None, 1000).is_err());
    let reset = crate::accessories::placement(&c, 1, 1, None, None, 1000).unwrap();
    assert_eq!(reset.position_x, None);
}

#[test]
fn published_previews_survive_cleanup_and_stale_upload_keeps_previous_images() {
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = Directory(
        std::env::temp_dir().join(format!(
            "yanmew-pet-check-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )),
    );
    let mut runtime = crate::runtime::test_runtime("http://127.0.0.1:1/v1");
    let paths = crate::platform::contracts::AppPaths::portable(root.0.clone());
    std::fs::create_dir_all(&paths.data_root).unwrap();
    runtime.paths = Ok(paths);
    let art = runtime
        .read(|s, _| save(&mut s.connection, 1, None, None, input("expression")))
        .unwrap();
    let id = art.id.parse().unwrap();
    let first = preview(&runtime, 1, id, 0, &png(false)).unwrap();
    runtime
        .read(|s, _| finish(&mut s.connection, 1, id, first.revision, "accept"))
        .unwrap();
    let second = preview(&runtime, 1, id, first.revision + 1, &png(false)).unwrap();
    crate::accessories::images::cleanup_unused(&runtime, &[]);
    assert!(image(&runtime, 1, id, false).is_ok());
    assert!(image(&runtime, 1, id, true).is_ok());
    assert!(preview(&runtime, 1, id, first.revision, &png(false)).is_err());
    let current = runtime.read(|s, _| get(&s.connection, 1, id)).unwrap();
    assert_eq!(current.image_file, second.image_file);
    assert_eq!(current.preview_file, second.preview_file);
}
