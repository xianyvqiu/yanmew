use super::*;

fn source(format: ImageFormat) -> Vec<u8> {
    let mut rgba = RgbaImage::new(12, 8);
    for y in 2..6 {
        for x in 2..10 {
            rgba.put_pixel(x, y, image::Rgba([240, 100, 120, 255]));
        }
    }
    encode(rgba, format)
}
fn encode(image: RgbaImage, format: ImageFormat) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, format)
        .unwrap();
    bytes.into_inner()
}
struct Fixture {
    runtime: Runtime,
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "yanmew-accessory-{}-{}",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let mut runtime = crate::runtime::test_runtime("http://localhost");
        runtime.paths = Ok(AppPaths::portable(root.clone()));
        Self { runtime, root }
    }
    fn insert(&self, relative: &str) {
        self.runtime.read(|store,_| {
            store.connection.execute("INSERT INTO accessories(id,session_id,name,display_text,description,source,image_file,created_at) VALUES(1,1,'星星','★','闪亮','crafted',?1,'a')",[relative])?;
            Ok(())
        }).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn png_webp_fit_without_stretch_and_keep_transparent_border() {
    for format in [ImageFormat::Png, ImageFormat::WebP] {
        let bytes = normalize(&source(format)).unwrap();
        let rgba = decode(&bytes).unwrap();
        assert_eq!(rgba.dimensions(), (256, 256));
        assert_eq!(rgba.get_pixel(0, 0)[3], 0);
        let visible_rows = (0..256)
            .filter(|y| (0..256).any(|x| rgba.get_pixel(x, *y)[3] > 0))
            .count();
        assert!(visible_rows < 150); // 原内容2:1，不能拉成方形。
    }
}
#[test]
fn reject_empty_fake_opaque_invisible_no_margin_and_oversized_images() {
    for bytes in [
        Vec::new(),
        b"not png".to_vec(),
        vec![0; MAX_BYTES + 1],
        encode(RgbaImage::new(8, 8), ImageFormat::Png),
        encode(
            RgbaImage::from_pixel(8, 8, image::Rgba([1, 2, 3, 255])),
            ImageFormat::Png,
        ),
        encode(RgbaImage::new(4097, 1), ImageFormat::Png),
    ] {
        assert!(normalize(&bytes).is_err());
    }
    let mut no_margin = RgbaImage::new(8, 8);
    no_margin.put_pixel(0, 3, image::Rgba([1, 2, 3, 255]));
    assert!(normalize(&encode(no_margin, ImageFormat::Png)).is_err());
    let mut rgb = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3])))
        .write_to(&mut rgb, ImageFormat::Jpeg)
        .unwrap();
    assert!(normalize(&rgb.into_inner()).is_err());
}
#[test]
fn publish_read_fallback_and_context_scope() {
    let f = Fixture::new();
    let file = publish(
        f.runtime.paths.as_ref().unwrap(),
        &normalize(&source(ImageFormat::Png)).unwrap(),
    )
    .unwrap();
    f.insert(&file);
    let view = get(&f.runtime, 1, 1, 0, 1).unwrap();
    assert!(view.data_url.unwrap().starts_with("data:image/png;base64,"));
    assert!(view.notice.is_none());
    assert_eq!(
        get(&f.runtime, 1, 1, 1, 1).unwrap_err().code,
        "CONTEXT_CHANGED"
    );
    assert!(get(&f.runtime, 1, 1, 0, 2).is_err());
    fs::remove_file(f.root.join(&file)).unwrap();
    let view = get(&f.runtime, 1, 1, 0, 1).unwrap();
    assert!(view.data_url.is_none());
    assert!(view.notice.unwrap().contains("Emoji"));
    f.runtime
        .read(|store, _| {
            assert_eq!(
                super::super::get(&store.connection, 1, 1)?.display_text,
                "★"
            );
            assert_eq!(crate::currency::get_balance(&store.connection)?, 0);
            Ok(())
        })
        .unwrap();
}
#[test]
fn wrong_paths_and_linked_resources_are_never_read_or_deleted() {
    let f = Fixture::new();
    for path in [
        "../secret.png",
        "C:/secret.png",
        "https://example.com/a.png",
        "assets/accessories/../a.png",
        "assets/accessories/a\\b.png",
        "assets/accessories/a.png:stream",
    ] {
        assert!(managed_file(f.runtime.paths.as_ref().unwrap(), path).is_err());
    }
    // 文件符号链接在Windows通常需要管理员权限；目录联接不需要。
    #[cfg(windows)]
    {
        let outside = f.root.with_extension("outside");
        fs::create_dir(&outside).unwrap();
        let original = outside.join("accessory-1-2-3.png");
        fs::write(&original, b"keep").unwrap();
        fs::create_dir_all(f.root.join("assets")).unwrap();
        let link = f.root.join("assets/accessories");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link.to_string_lossy().replace('/', "\\"))
            .arg(outside.to_string_lossy().replace('/', "\\"))
            .output()
            .unwrap();
        assert!(status.status.success(), "联接测试准备失败：{} {}", String::from_utf8_lossy(&status.stdout), String::from_utf8_lossy(&status.stderr));
        assert!(directory(f.runtime.paths.as_ref().unwrap(), false, false).is_err());
        cleanup_startup(&f.runtime);
        assert!(original.exists());
        fs::remove_dir(&link).unwrap(); // 删除联接本身，不递归删除目标。
        fs::remove_file(original).unwrap();
        fs::remove_dir(outside).unwrap();
    }
}
#[test]
fn cleanup_keeps_database_and_active_preview_references_and_unknown_files() {
    let f = Fixture::new();
    let png = normalize(&source(ImageFormat::Png)).unwrap();
    let used = publish(f.runtime.paths.as_ref().unwrap(), &png).unwrap();
    f.insert(&used);
    let protected = publish(f.runtime.paths.as_ref().unwrap(), &png).unwrap();
    let orphan = publish(f.runtime.paths.as_ref().unwrap(), &png).unwrap();
    let unknown = f.root.join("assets/accessories/user-file.png");
    fs::write(&unknown, b"keep").unwrap();
    cleanup_unused(&f.runtime, &[protected.clone()]);
    assert!(f.root.join(&used).exists());
    assert!(f.root.join(&protected).exists());
    assert!(!f.root.join(&orphan).exists());
    assert!(unknown.exists());
    f.runtime
        .read(|store, _| {
            super::super::delete(&mut store.connection, 1, 1)?;
            Ok(())
        })
        .unwrap();
    cleanup_one(&f.runtime, &used, &[]);
    assert!(!f.root.join(used).exists());
    cleanup_startup(&f.runtime);
    assert!(!f.root.join(protected).exists());
}
#[test]
fn unreadable_database_preserves_orphans() {
    let f = Fixture::new();
    let file = publish(
        f.runtime.paths.as_ref().unwrap(),
        &normalize(&source(ImageFormat::Png)).unwrap(),
    )
    .unwrap();
    *f.runtime.store.lock().unwrap() = Err(AppError::storage());
    cleanup_startup(&f.runtime);
    assert!(f.root.join(file).exists());
}
