use super::*;

struct Fixture {
    runtime: Runtime,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "yanmew-background-test-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let mut runtime = crate::runtime::test_runtime("http://localhost");
        runtime.paths = Ok(crate::platform::contracts::AppPaths::portable(root.clone()));
        Self { runtime, root }
    }
    fn record(&self) -> Background {
        get(&self.runtime, 1, None).unwrap().record
    }
    fn input(&self, image: Option<Upload>) -> Input {
        Input {
            expected: self.record(),
            opacity: 60,
            blur: 4,
            image,
            reset: false,
            appearance: None,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.runtime.read(|store, _| {
            store.connection = rusqlite::Connection::open_in_memory()?;
            Ok(())
        });
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn upload(format: image::ImageFormat) -> Upload {
    let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        2,
        2,
        image::Rgb([120, 60, 30]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).unwrap();
    Upload {
        file_name: format!("背景.{}", format.extensions_str()[0]),
        base64: base64::engine::general_purpose::STANDARD.encode(bytes.into_inner()),
    }
}

#[test]
fn appearance_persists_is_scoped_and_survives_background_reset() {
    let f = Fixture::new();
    f.runtime.read(|s,_| { s.connection.execute("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(2,'另一只','设定','设定')",[])?; Ok(()) }).unwrap();
    let mut input = f.input(None);
    let expected = Appearance { bubble_style: "frosted".into(), user_text_color: Some("#123456".into()), assistant_text_color: Some("#AbCdEf".into()) };
    input.appearance = Some(expected.clone());
    save(&f.runtime,1,None,input).unwrap();
    assert_eq!(f.record().appearance, expected);
    assert_eq!(get(&f.runtime,2,None).unwrap().record.appearance, Appearance::default());
    let mut input = f.input(None); input.reset = true;
    assert_eq!(save(&f.runtime,1,None,input).unwrap().appearance, expected);
    assert_eq!(f.runtime.read(|s,_| Ok(crate::persona::list(&s.connection)?.remove(0).chat_appearance)).unwrap(), expected);
}

#[test]
fn invalid_appearance_is_rejected_and_concurrent_style_changes_conflict() {
    let f = Fixture::new();
    let original = f.record().appearance;
    for (style,color) in [("unknown","#123456"),("solid","red"),("outline","#12345g")] {
        let mut input=f.input(None);
        input.appearance=Some(Appearance { bubble_style:style.into(),user_text_color:Some(color.into()),assistant_text_color:None });
        assert_eq!(save(&f.runtime,1,None,input).unwrap_err().code,"VALIDATION_ERROR");
        assert_eq!(f.record().appearance,original);
    }
    let stale=f.input(None);
    let mut input=f.input(None); input.appearance=Some(Appearance { bubble_style:"outline".into(),..Appearance::default() });
    save(&f.runtime,1,None,input).unwrap();
    assert_eq!(save(&f.runtime,1,None,stale).unwrap_err().code,"CONFLICT");
}

#[test]
fn accepts_all_three_real_formats_and_rejects_bad_input_without_publishing() {
    let f = Fixture::new();
    for format in [
        image::ImageFormat::Jpeg,
        image::ImageFormat::Png,
        image::ImageFormat::WebP,
    ] {
        let saved = save(&f.runtime, 1, Some(0), f.input(Some(upload(format)))).unwrap();
        let view = get(&f.runtime, 1, Some(0)).unwrap();
        assert_eq!(view.record, saved);
        assert!(view.image_error.is_none());
        assert!(view.image_data_url.unwrap().starts_with("data:image/"));
    }
    let before = f.record();
    let mut wrong_format = upload(image::ImageFormat::Png);
    wrong_format.file_name = "伪装.jpg".into();
    let bad = [
        wrong_format,
        Upload {
            file_name: "破损.png".into(),
            base64: "iVBORw0KGgo=".into(),
        },
        Upload {
            file_name: "非法.svg".into(),
            base64: "YQ==".into(),
        },
        Upload {
            file_name: "../越界.png".into(),
            base64: "YQ==".into(),
        },
        Upload {
            file_name: "大图.png".into(),
            base64: "A".repeat(MAX_BYTES.div_ceil(3) * 4 + 1),
        },
    ];
    for image in bad {
        assert!(save(&f.runtime, 1, None, f.input(Some(image))).is_err());
    }
    assert_eq!(f.record(), before);
    assert_eq!(
        fs::read_dir(f.root.join("chat_background"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn saves_to_disk_and_restores_same_image_and_metadata_after_reopening_database() {
    let f = Fixture::new();
    let database = f.root.join("test.db");
    f.runtime
        .read(|store, _| {
            store
                .connection
                .execute("VACUUM INTO ?1", [database.to_str().unwrap()])?;
            store.connection = rusqlite::Connection::open(&database)?;
            store.connection.execute_batch("PRAGMA foreign_keys=ON")?;
            Ok(())
        })
        .unwrap();
    let saved = save(
        &f.runtime,
        1,
        None,
        f.input(Some(upload(image::ImageFormat::Png))),
    )
    .unwrap();
    let original = get(&f.runtime, 1, None).unwrap().image_data_url;
    f.runtime
        .read(|store, _| {
            store.connection = rusqlite::Connection::open(&database)?;
            store.connection.execute_batch("PRAGMA foreign_keys=ON")?;
            Ok(())
        })
        .unwrap();
    let after = get(&f.runtime, 1, None).unwrap();
    assert_eq!(after.record, saved);
    assert_eq!(after.image_data_url, original);
}

#[test]
fn background_updates_do_not_modify_persona_prompt_baseline_or_context() {
    let f = Fixture::new();
    f.runtime.read(|store, _| {
        store.connection.execute("UPDATE personas SET prompt='当前',initial_prompt='初始',last_evolved_at='a' WHERE id=1", [])?; Ok(())
    }).unwrap();
    save(
        &f.runtime,
        1,
        Some(0),
        f.input(Some(upload(image::ImageFormat::Png))),
    )
    .unwrap();
    f.runtime
        .read(|store, _| {
            let persona = crate::persona::list(&store.connection)?.remove(0);
            assert_eq!(persona.prompt, "当前");
            assert_eq!(persona.initial_prompt, "初始");
            assert_eq!(persona.last_evolved_at.as_deref(), Some("a"));
            assert_eq!(store.context_revision, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn stale_snapshots_or_chat_context_never_overwrite_new_background() {
    let f = Fixture::new();
    let stale = f.input(Some(upload(image::ImageFormat::Png)));
    let saved = save(
        &f.runtime,
        1,
        None,
        f.input(Some(upload(image::ImageFormat::Jpeg))),
    )
    .unwrap();
    assert_eq!(
        save(&f.runtime, 1, None, stale).unwrap_err().code,
        "CONFLICT"
    );
    let stale_context = f.input(Some(upload(image::ImageFormat::Png)));
    f.runtime
        .read(|store, _| {
            store.context_revision = 1;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        save(&f.runtime, 1, Some(0), stale_context)
            .unwrap_err()
            .code,
        "CONTEXT_CHANGED"
    );
    f.runtime.read(|store, _| {
        store.connection.execute("INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(2,'另一只','设定','设定')", [])?; Ok(())
    }).unwrap();
    assert!(get(&f.runtime, 2, Some(1)).is_err());
    assert!(get(&f.runtime, 2, None).is_ok());
    assert_eq!(f.record(), saved);
    assert_eq!(
        fs::read_dir(f.root.join("chat_background"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn failed_transaction_preserves_old_reference_file_and_cleans_unreferenced_new_file() {
    let f = Fixture::new();
    let old = save(
        &f.runtime,
        1,
        None,
        f.input(Some(upload(image::ImageFormat::Png))),
    )
    .unwrap();
    let next = f.input(Some(upload(image::ImageFormat::Jpeg)));
    f.runtime.read(|store, _| { store.connection.execute_batch("CREATE TRIGGER fail_bg BEFORE UPDATE OF chat_bg_file ON personas BEGIN SELECT RAISE(ABORT,'blocked'); END;")?; Ok(()) }).unwrap();
    assert!(save(&f.runtime, 1, None, next).is_err());
    assert_eq!(f.record(), old);
    assert!(get(&f.runtime, 1, None).unwrap().image_data_url.is_some());
    assert_eq!(
        fs::read_dir(f.root.join("chat_background"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn reset_uses_defaults_and_preserves_files_referenced_by_another_persona() {
    let f = Fixture::new();
    let old = save(
        &f.runtime,
        1,
        None,
        f.input(Some(upload(image::ImageFormat::Png))),
    )
    .unwrap();
    f.runtime.read(|store, _| { store.connection.execute("INSERT INTO personas(id,name,prompt,initial_prompt,chat_bg_file) VALUES(2,'另一只','设定','设定',?1)", [&old.file])?; Ok(()) }).unwrap();
    let mut input = f.input(None);
    input.reset = true;
    let reset = save(&f.runtime, 1, None, input).unwrap();
    assert_eq!(reset.file, None);
    assert_eq!(reset.opacity, 85);
    assert_eq!(reset.blur, 0);
    assert!(get(&f.runtime, 2, None).unwrap().image_data_url.is_some());
    let input = Input {
        expected: get(&f.runtime, 2, None).unwrap().record,
        opacity: 0,
        blur: 0,
        image: None,
        reset: true,
        appearance: None,
    };
    save(&f.runtime, 2, None, input).unwrap();
    assert_eq!(
        fs::read_dir(f.root.join("chat_background"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn missing_or_unsafe_files_are_local_errors_and_cleanup_keeps_unmanaged_files() {
    let f = Fixture::new();
    let directory = directory(&f.runtime, true).unwrap();
    fs::write(directory.join("background-123-1.png"), b"orphan").unwrap();
    fs::write(directory.join("user-picture.png"), b"keep").unwrap();
    cleanup_unused(&f.runtime);
    assert!(!directory.join("background-123-1.png").exists());
    assert!(directory.join("user-picture.png").exists());
    for name in ["missing.png", "../outside.png"] {
        f.runtime
            .read(|store, _| {
                store
                    .connection
                    .execute("UPDATE personas SET chat_bg_file=?1 WHERE id=1", [name])?;
                Ok(())
            })
            .unwrap();
        let view = get(&f.runtime, 1, None).unwrap();
        assert!(view.image_data_url.is_none());
        assert!(view.image_error.is_some());
    }
    assert!(directory.join("user-picture.png").exists());
}
