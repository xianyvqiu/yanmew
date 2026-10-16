//! 饰品图片受管文件服务：先发布不可变PNG，再提交引用；不调用网络或修改钱包。
use crate::{
    error::{AppError, AppResult},
    platform::contracts::AppPaths,
    runtime::Runtime,
};
use base64::Engine;
use image::{ImageDecoder, ImageFormat, RgbaImage};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_EDGE: u32 = 4096;
const IMAGE_SIZE: u32 = 256;
const PREFIX: &str = "assets/accessories/";
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageView {
    pub data_url: Option<String>,
    pub notice: Option<String>,
}
fn invalid() -> AppError {
    AppError::new(
        "INVALID_ACCESSORY_IMAGE",
        "饰品图片须为有效静态透明PNG/WebP，四边留有透明空间且有可见内容",
    )
}
fn unsafe_path() -> AppError {
    AppError::new("INVALID_ACCESSORY_PATH", "饰品图片引用不在受管资源目录内")
}

/// PNG/WebP解码前限制边长、分配和输入体积；显式拒绝动画，不能只看扩展名。
pub(crate) fn decode(bytes: &[u8]) -> AppResult<RgbaImage> { decode_transparent(bytes,true) }
pub(crate) fn decode_transparent(bytes:&[u8],require_border:bool)->AppResult<RgbaImage> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(invalid());
    }
    let format = image::guess_format(bytes).map_err(|_| invalid())?;
    let animated = match format {
        ImageFormat::Png => image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(|_| invalid())?
            .is_apng()
            .map_err(|_| invalid())?,
        ImageFormat::WebP => image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
            .map_err(|_| invalid())?
            .has_animation(),
        _ => return Err(invalid()),
    };
    if animated {
        return Err(invalid());
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_EDGE);
    limits.max_image_height = Some(MAX_EDGE);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader.into_decoder().map_err(|_| invalid())?;
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || width > MAX_EDGE
        || height > MAX_EDGE
        || u64::from(width) * u64::from(height) > 16_777_216
        || !decoder.color_type().has_alpha()
    {
        return Err(invalid());
    }
    let rgba = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| invalid())?
        .into_rgba8();
    // 要求四边透明，避免把不透明底图或紧贴边界的整张画面当作饰品。
    let border_clear = (0..width)
        .all(|x| rgba.get_pixel(x, 0)[3] == 0 && rgba.get_pixel(x, height - 1)[3] == 0)
        && (0..height)
            .all(|y| rgba.get_pixel(0, y)[3] == 0 && rgba.get_pixel(width - 1, y)[3] == 0);
    if (require_border && !border_clear) || !rgba.pixels().any(|pixel| pixel[3] > 0) || !rgba.pixels().any(|pixel| pixel[3] == 0) {
        return Err(invalid());
    }
    Ok(rgba)
}

/// 裁去透明留白，等比居中；留一像素边距使保存结果也满足透明边界校验。
pub(crate) fn normalize(bytes: &[u8]) -> AppResult<Vec<u8>> {
    let rgba = decode(bytes)?;
    let (mut left, mut top, mut right, mut bottom) = (rgba.width(), rgba.height(), 0, 0);
    for (x, y, pixel) in rgba.enumerate_pixels() {
        if pixel[3] > 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }
    let cropped =
        image::imageops::crop_imm(&rgba, left, top, right - left + 1, bottom - top + 1).to_image();
    let fitted = image::DynamicImage::ImageRgba8(cropped)
        .resize(
            IMAGE_SIZE - 2,
            IMAGE_SIZE - 2,
            image::imageops::FilterType::Lanczos3,
        )
        .into_rgba8();
    let mut canvas = RgbaImage::new(IMAGE_SIZE, IMAGE_SIZE);
    image::imageops::overlay(
        &mut canvas,
        &fitted,
        i64::from((IMAGE_SIZE - fitted.width()) / 2),
        i64::from((IMAGE_SIZE - fitted.height()) / 2),
    );
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|_| invalid())?;
    Ok(output.into_inner())
}

fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        } // Windows reparse point，包括目录联接。
    }
    false
}
fn directory(paths: &AppPaths, staging: bool, create: bool) -> AppResult<PathBuf> {
    let metadata = fs::symlink_metadata(&paths.data_root).map_err(|_| AppError::storage())?;
    if linked(&metadata) || !metadata.is_dir() {
        return Err(unsafe_path());
    }
    let root = paths
        .data_root
        .canonicalize()
        .map_err(|_| AppError::storage())?;
    let mut current = root.clone();
    let parts: &[&str] = if staging {
        &["assets", "accessories", "staging"]
    } else {
        &["assets", "accessories"]
    };
    for part in parts {
        current.push(part);
        if create {
            match fs::create_dir(&current) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return Err(AppError::storage()),
            }
        }
        let metadata = fs::symlink_metadata(&current).map_err(|_| AppError::storage())?;
        if linked(&metadata)
            || !metadata.is_dir()
            || !current
                .canonicalize()
                .map_err(|_| AppError::storage())?
                .starts_with(&root)
        {
            return Err(unsafe_path());
        }
    }
    Ok(current)
}
fn filename(relative: &str) -> AppResult<&str> {
    let name = relative.strip_prefix(PREFIX).ok_or_else(unsafe_path)?;
    if name.contains(['/', '\\', ':', '\0'])
        || !name.ends_with(".png")
        || name.len() > 180
        || Path::new(name).components().count() != 1
        || name.trim() != name
    {
        return Err(unsafe_path());
    }
    Ok(name)
}
pub(crate) fn managed_file(paths: &AppPaths, relative: &str) -> AppResult<PathBuf> {
    let directory = directory(paths, false, false)?;
    let path = directory.join(filename(relative)?);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| AppError::new("ACCESSORY_IMAGE_UNAVAILABLE", "饰品图片缺失"))?;
    if linked(&metadata)
        || !metadata.is_file()
        || !path
            .canonicalize()
            .map_err(|_| unsafe_path())?
            .starts_with(&directory)
    {
        return Err(unsafe_path());
    }
    Ok(path)
}
/// 校验已经持久化的256像素透明PNG，不改变内容。
pub(crate) fn validate_png(bytes: &[u8]) -> AppResult<()> {
    if image::guess_format(bytes).map_err(|_| invalid())? != ImageFormat::Png {
        return Err(invalid());
    }
    let rgba = decode(bytes)?;
    if rgba.dimensions() != (IMAGE_SIZE, IMAGE_SIZE) {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn read(paths: &AppPaths, relative: &str) -> AppResult<String> {
    let path = managed_file(paths, relative)?;
    let mut file = fs::File::open(path).map_err(|_| AppError::storage())?;
    if file.metadata().map_err(|_| AppError::storage())?.len() > MAX_BYTES as u64 {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::storage())?;
    validate_png(&bytes)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// 只接受归一化PNG；文件先刷新并发布，调用方才可开始引用它的数据库事务。
pub(crate) fn publish(paths: &AppPaths, png: &[u8]) -> AppResult<String> {
    if image::guess_format(png).map_err(|_| invalid())? != ImageFormat::Png
        || decode(png)?.dimensions() != (IMAGE_SIZE, IMAGE_SIZE)
    {
        return Err(invalid());
    }
    let staging = directory(paths, true, true)?;
    let final_directory = directory(paths, false, false)?;
    for _ in 0..16 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| AppError::storage())?
            .as_nanos();
        let name = format!(
            "accessory-{nanos}-{}-{}.png",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        );
        let temporary = staging.join(&name);
        let target = final_directory.join(&name);
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(AppError::storage()),
        };
        let written = file.write_all(png).and_then(|_| file.sync_all());
        drop(file); // Windows移动前释放文件句柄。
        if written.is_err() || target.exists() || fs::rename(&temporary, &target).is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(AppError::storage());
        }
        return Ok(format!("{PREFIX}{name}"));
    }
    Err(AppError::storage())
}

/// 解码在锁外完成，返回前复核上下文与该饰品的图片引用；缺图只提示视觉兜底。
pub(crate) fn get(
    runtime: &Runtime,
    persona: i64,
    session: i64,
    revision: u64,
    id: i64,
) -> AppResult<ImageView> {
    let record = runtime.read(|store, _| {
        super::require_current(store, persona, session, revision)?;
        super::get(&store.connection, session, id)
    })?;
    let image = record.image_file.as_deref().map(|relative| {
        runtime
            .paths
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|paths| read(paths, relative))
    });
    runtime.read(|store, _| {
        super::require_current(store, persona, session, revision)?;
        if super::get(&store.connection, session, id)?.image_file != record.image_file {
            return Err(AppError::new("CONFLICT", "饰品图片已改变，请刷新后重试"));
        }
        Ok(())
    })?;
    Ok(match image {
        Some(Ok(data_url)) => ImageView {
            data_url: Some(data_url),
            notice: None,
        },
        Some(Err(_)) => ImageView {
            data_url: None,
            notice: Some(
                "图片缺失、损坏或无法读取，使用此饰品保存的图标、Emoji、颜文字或符号组合显示"
                    .into(),
            ),
        },
        None => ImageView {
            data_url: None,
            notice: None,
        },
    })
}

/// 清理只针对宿主生成的文件；数据库不可读或任一饰品/活动候选引用时保留。
pub(crate) fn cleanup_one(runtime: &Runtime, relative: &str, protected: &[String]) {
    if !filename(relative).is_ok_and(generated_name) {
        return;
    }
    if protected.iter().any(|path| path == relative) {
        return;
    }
    let _ = runtime.read(|store, _| {
        if store.accessory_crafts.references(relative) {
            return Ok(());
        }
        let used: bool = store.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM accessories WHERE image_file=?1 UNION ALL SELECT 1 FROM pet_artworks WHERE image_file=?1 OR preview_file=?1)",
            [relative],
            |row| row.get(0),
        )?;
        if !used {
            if let Ok(paths) = &runtime.paths {
                if let Ok(path) = managed_file(paths, relative) {
                    let _ = fs::remove_file(path);
                }
            }
        }
        Ok(())
    });
}
fn generated_name(name: &str) -> bool {
    name.strip_prefix("accessory-")
        .and_then(|s| s.strip_suffix(".png"))
        .is_some_and(|s| {
            let parts = s.split('-').collect::<Vec<_>>();
            parts.len() == 3
                && parts
                    .iter()
                    .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        })
}
/// 启动时没有活动预览；级联删除后调用方应传入所有活动预览引用以保护候选。
pub(crate) fn cleanup_unused(runtime: &Runtime, protected: &[String]) {
    let Ok(paths) = &runtime.paths else {
        return;
    };
    if let Ok(directory) = directory(paths, false, false) {
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                if let Some(name) = entry
                    .file_name()
                    .to_str()
                    .filter(|name| generated_name(name))
                {
                    cleanup_one(runtime, &format!("{PREFIX}{name}"), protected);
                }
            }
        }
    }
}

/// 只在启动后台任务之前调用，数据库不可读时保留全部文件。
pub(crate) fn cleanup_startup(runtime: &Runtime) {
    if runtime.read(|_, _| Ok(())).is_err() {
        return;
    }
    cleanup_unused(runtime, &[]);
    let Ok(paths) = &runtime.paths else {
        return;
    };
    if let Ok(staging) = directory(paths, true, false) {
        if let Ok(entries) = fs::read_dir(staging) {
            for entry in entries.flatten() {
                if entry.file_name().to_str().is_some_and(generated_name)
                    && fs::symlink_metadata(entry.path()).is_ok_and(|m| m.is_file() && !linked(&m))
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
