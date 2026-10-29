use crate::{
    error::{AppError, AppResult},
    runtime::{Runtime, Store},
};
use base64::Engine;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const MAX_BYTES: usize = 10 * 1024 * 1024;
static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

/// 外观只影响聊天记录正文，不改变人设提示词、宠物或提醒气泡。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appearance {
    pub bubble_style: String,
    pub user_text_color: Option<String>,
    pub assistant_text_color: Option<String>,
}
impl Default for Appearance {
    fn default() -> Self {
        Self { bubble_style: "solid".into(), user_text_color: None, assistant_text_color: None }
    }
}
impl Appearance {
    fn validate(&self) -> AppResult<()> {
        if !["solid", "transparent", "outline", "frosted"].contains(&self.bubble_style.as_str()) {
            return Err(AppError::validation("bubbleStyle", "请选择有效的聊天气泡样式"));
        }
        for color in [&self.user_text_color, &self.assistant_text_color].into_iter().flatten() {
            if color.len() != 7 || !color.starts_with('#') || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit) {
                return Err(AppError::validation("textColor", "文字颜色须为 #RRGGBB，或留空跟随人设颜色"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Background {
    pub persona_id: String,
    pub file: Option<String>,
    pub opacity: i64,
    pub blur: i64,
    #[serde(default)]
    pub appearance: Appearance,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub record: Background,
    pub image_data_url: Option<String>,
    pub image_error: Option<AppError>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Upload {
    pub file_name: String,
    pub base64: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub expected: Background,
    pub opacity: i64,
    pub blur: i64,
    pub image: Option<Upload>,
    pub reset: bool,
    /// 不提交时保留原样式；恢复默认背景不会重置消息外观。
    #[serde(default)]
    pub appearance: Option<Appearance>,
}

fn conflict() -> AppError {
    AppError::new("CONFLICT", "聊天外观已在其他窗口修改，请重新读取后再保存")
}
fn snapshot(store: &Store, id: i64, revision: Option<u64>) -> AppResult<Background> {
    if let Some(revision) = revision {
        if revision != store.context_revision
            || crate::persona::context(&store.connection, revision)?.persona_id != id.to_string()
        {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前人设或会话已改变，请重新打开背景设置",
            ));
        }
    }
    store
        .connection
        .query_row(
            "SELECT chat_bg_file,chat_bg_opacity,chat_bg_blur,chat_bubble_style,chat_user_text_color,chat_assistant_text_color FROM personas WHERE id=?1",
            [id],
            |row| {
                Ok(Background {
                    persona_id: id.to_string(),
                    file: row.get(0)?,
                    opacity: row.get(1)?,
                    blur: row.get(2)?,
                    appearance: Appearance { bubble_style: row.get(3)?, user_text_color: row.get(4)?, assistant_text_color: row.get(5)? },
                })
            },
        )
        .optional()?
        .ok_or_else(AppError::not_found)
}
fn directory(runtime: &Runtime, create: bool) -> AppResult<PathBuf> {
    let root = runtime
        .paths
        .as_ref()
        .map_err(Clone::clone)?
        .data_root
        .canonicalize()
        .map_err(|_| AppError::storage())?;
    let path = root.join("chat_background");
    if create {
        fs::create_dir_all(&path).map_err(|_| AppError::storage())?;
    }
    let path = path.canonicalize().map_err(|_| AppError::storage())?;
    if !path.starts_with(&root) {
        return Err(AppError::new(
            "INVALID_BACKGROUND_PATH",
            "背景资源目录不在应用数据目录内",
        ));
    }
    Ok(path)
}
fn extension(name: &str) -> AppResult<(&'static str, image::ImageFormat)> {
    if name.contains(['/', '\\', ':', '\0']) {
        return Err(AppError::validation(
            "image",
            "请提交图片文件名，不要提交路径",
        ));
    }
    match Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => Ok(("jpg", image::ImageFormat::Jpeg)),
        "png" => Ok(("png", image::ImageFormat::Png)),
        "webp" => Ok(("webp", image::ImageFormat::WebP)),
        _ => Err(AppError::validation(
            "image",
            "背景仅支持 JPG、PNG 和 WebP 图片",
        )),
    }
}
pub(crate) fn validate(name: &str, bytes: &[u8]) -> AppResult<&'static str> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(AppError::validation("image", "图片不能为空且不能超过 10MB"));
    }
    let (extension, format) = extension(name)?;
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| AppError::validation("image", "无法识别图片格式"))?;
    if reader.format() != Some(format) {
        return Err(AppError::validation("image", "图片实际格式与扩展名不一致"));
    }
    // 使用解码器默认资源限制，不仅检查文件头，也不禁用分配限制。
    reader.decode().map_err(|_| {
        AppError::validation(
            "image",
            "图片无法解码或超过解码资源限制，请重新选择有效图片",
        )
    })?;
    Ok(extension)
}
pub(crate) fn managed_file(runtime: &Runtime, name: &str) -> AppResult<PathBuf> {
    extension(name)?;
    if Path::new(name).components().count() != 1 {
        return Err(AppError::validation("image", "背景引用必须是受管文件名"));
    }
    let directory = directory(runtime, false)?;
    let path = directory
        .join(name)
        .canonicalize()
        .map_err(|_| AppError::new("BACKGROUND_MISSING", "背景图片缺失，请重新上传或恢复默认"))?;
    if !path.starts_with(&directory) {
        return Err(AppError::new("INVALID_BACKGROUND_PATH", "背景图片引用无效"));
    }
    Ok(path)
}
pub(crate) fn read_image(runtime: &Runtime, name: &str) -> AppResult<String> {
    let path = managed_file(runtime, name)?;
    let mut file = fs::File::open(path).map_err(|_| AppError::storage())?;
    if file.metadata().map_err(|_| AppError::storage())?.len() > MAX_BYTES as u64 {
        return Err(AppError::validation("image", "背景图片超过 10MB"));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::storage())?;
    let ext = validate(name, &bytes)?;
    let mime = if ext == "jpg" { "jpeg" } else { ext };
    Ok(format!(
        "data:image/{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
pub fn get(runtime: &Runtime, id: i64, revision: Option<u64>) -> AppResult<View> {
    let record = runtime.read(|store, _| snapshot(store, id, revision))?;
    let image = record.file.as_deref().map(|name| read_image(runtime, name));
    runtime.read(|store, _| {
        if snapshot(store, id, revision)? == record {
            Ok(())
        } else {
            Err(conflict())
        }
    })?;
    let (image_data_url, image_error) = match image {
        Some(Ok(url)) => (Some(url), None),
        Some(Err(error)) => (None, Some(error)),
        None => (None, None),
    };
    Ok(View {
        record,
        image_data_url,
        image_error,
    })
}
pub(crate) fn publish(runtime: &Runtime, extension: &str, bytes: &[u8]) -> AppResult<String> {
    let directory = directory(runtime, true)?;
    for _ in 0..16 {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| AppError::storage())?
            .as_nanos();
        let name = format!(
            "background-{time}-{}.{extension}",
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        );
        let path = directory.join(&name);
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(AppError::storage()),
        };
        let result = file.write_all(bytes).and_then(|_| file.sync_all());
        drop(file);
        if result.is_err() {
            let _ = fs::remove_file(path);
            return Err(AppError::storage());
        }
        return Ok(name);
    }
    Err(AppError::storage())
}
pub(crate) fn cleanup_one(runtime: &Runtime, name: &str) {
    // 数据库不可读、文件越界或仍有任一人设引用时，绝不删除。
    let _ = runtime.read(|store, _| {
        let used: bool = store.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM personas WHERE chat_bg_file=?1)",
            [name],
            |r| r.get(0),
        )?;
        if !used {
            if let Ok(path) = managed_file(runtime, name) {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    });
}
pub fn save(
    runtime: &Runtime,
    id: i64,
    revision: Option<u64>,
    input: Input,
) -> AppResult<Background> {
    if let Some(appearance) = &input.appearance { appearance.validate()?; }
    if input.expected.persona_id != id.to_string() {
        return Err(AppError::validation("personaId", "背景不属于目标人设"));
    }
    if !(0..=100).contains(&input.opacity)
        || !(0..=40).contains(&input.blur)
        || (input.reset && input.image.is_some())
    {
        return Err(AppError::validation(
            "background",
            "背景透明度须为 0–100，模糊度须为 0–40；恢复默认不能同时上传",
        ));
    }
    runtime.read(|store, _| {
        if snapshot(store, id, revision)? == input.expected {
            Ok(())
        } else {
            Err(conflict())
        }
    })?;
    let published = if let Some(image) = &input.image {
        if image.base64.len() > MAX_BYTES.div_ceil(3) * 4 {
            return Err(AppError::validation("image", "图片不能超过 10MB"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&image.base64)
            .map_err(|_| AppError::validation("image", "图片数据无效"))?;
        Some(publish(
            runtime,
            validate(&image.file_name, &bytes)?,
            &bytes,
        )?)
    } else {
        None
    };
    let result = runtime.read(|store, _| {
        if snapshot(store, id, revision)? != input.expected {
            return Err(conflict());
        }
        let file = if input.reset {
            None
        } else {
            published.clone().or_else(|| input.expected.file.clone())
        };
        let opacity = if input.reset { 85 } else { input.opacity };
        let blur = if input.reset { 0 } else { input.blur };
        let appearance = input.appearance.clone().unwrap_or_else(|| input.expected.appearance.clone());
        let tx = store.connection.transaction()?;
        tx.execute(
            "UPDATE personas SET chat_bg_file=?1,chat_bg_opacity=?2,chat_bg_blur=?3,chat_bubble_style=?5,chat_user_text_color=?6,chat_assistant_text_color=?7 WHERE id=?4",
            params![file, opacity, blur, id, appearance.bubble_style, appearance.user_text_color, appearance.assistant_text_color],
        )?;
        tx.commit()?;
        Ok(Background {
            persona_id: id.to_string(),
            file,
            opacity,
            blur,
            appearance,
        })
    });
    if result.is_err() {
        if let Some(name) = &published {
            cleanup_one(runtime, name);
        }
    } else if let Some(name) = &input.expected.file {
        if input.reset || published.is_some() {
            cleanup_one(runtime, name);
        }
    }
    result
}
/// 启动时只清理本程序生成的孤立文件，不处理用户自行放入的其他文件。
pub fn cleanup_unused(runtime: &Runtime) {
    if let Ok(directory) = directory(runtime, false) {
        if let Ok(entries) = fs::read_dir(directory) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    let stem = Path::new(name)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");
                    if stem.strip_prefix("background-").is_some_and(|s| {
                        !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || c == '-')
                    }) {
                        cleanup_one(runtime, name);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
