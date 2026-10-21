//! 人设级图片桌宠：原业务状态不变，正式图片与待确认预览分开持久化。
use crate::{
    error::{AppError, AppResult},
    runtime::Runtime,
};
use base64::Engine;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
const COLUMNS:&str="id,persona_id,kind,name,prompt,description,category,enabled,image_file,preview_file,revision,created_at,updated_at";
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    pub id: String,
    pub persona_id: String,
    pub kind: String,
    pub name: String,
    pub prompt: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    pub image_file: Option<String>,
    pub preview_file: Option<String>,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub enabled: bool,
    pub revision: u64,
    pub items: Vec<Artwork>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub name: String,
    pub prompt: String,
    pub description: String,
    pub category: String,
    pub enabled: bool,
    pub kind: String,
}
fn conflict() -> AppError {
    AppError::new(
        "CONFIG_CONFLICT",
        "创作条目或参考形象已改变，请刷新后重试；原图片保留",
    )
}
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Artwork> {
    Ok(Artwork {
        id: r.get::<_, i64>(0)?.to_string(),
        persona_id: r.get::<_, i64>(1)?.to_string(),
        kind: r.get(2)?,
        name: r.get(3)?,
        prompt: r.get(4)?,
        description: r.get(5)?,
        category: r.get(6)?,
        enabled: r.get(7)?,
        image_file: r.get(8)?,
        preview_file: r.get(9)?,
        revision: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}
pub fn require_current(c: &Connection, persona: i64) -> AppResult<()> {
    let exists: bool = c.query_row(
        "SELECT EXISTS(SELECT 1 FROM personas WHERE id=?1 AND is_active=1)",
        [persona],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(AppError::new(
            "CONTEXT_CHANGED",
            "当前人设已切换，请重新打开创作页面",
        ));
    }
    Ok(())
}
pub fn library(c: &Connection, persona: i64) -> AppResult<Library> {
    let (enabled, revision) = c
        .query_row(
            "SELECT pet_image_enabled,pet_image_revision FROM personas WHERE id=?1",
            [persona],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    let mut q = c.prepare(&format!(
        "SELECT {COLUMNS} FROM pet_artworks WHERE persona_id=?1 ORDER BY kind,id DESC"
    ))?;
    let items = q
        .query_map([persona], row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Library {
        enabled,
        revision,
        items,
    })
}
pub fn get(c: &Connection, persona: i64, id: i64) -> AppResult<Artwork> {
    c.query_row(
        &format!("SELECT {COLUMNS} FROM pet_artworks WHERE persona_id=?1 AND id=?2"),
        params![persona, id],
        row,
    )
    .optional()?
    .ok_or_else(AppError::not_found)
}
fn changed(c: &Connection, persona: i64) -> AppResult<()> {
    c.execute(
        "UPDATE personas SET pet_image_revision=pet_image_revision+1 WHERE id=?1",
        [persona],
    )?;
    Ok(())
}
pub fn save(
    c: &mut Connection,
    persona: i64,
    id: Option<i64>,
    revision: Option<u64>,
    input: Input,
) -> AppResult<Artwork> {
    require_current(c, persona)?;
    if input.name.trim().is_empty()
        || input.name.chars().count() > 80
        || input.prompt.chars().count() > 8000
        || input.description.chars().count() > 1000
        || !["base", "expression"].contains(&input.kind.as_str())
        || ![
            "normal", "standby", "click", "hungry", "eating", "sleepy", "bored", "thinking", "chat", "craving", "craving_favorite", "eating_favorite", "peek_left", "peek_right", "rest_sit", "rest_lie", "walk_left", "walk_right", "lifted",
        ]
        .contains(&input.category.as_str())
    {
        return Err(AppError::validation(
            "artwork",
            "请填写名称，生成要求最多8000字，使用说明最多1000字，并选择有效类别",
        ));
    }
    let tx = c.transaction()?;
    let now = chrono::Utc::now().to_rfc3339();
    let mut protected_standby = false;
    let id = if let Some(id) = id {
        let old = get(&tx, persona, id)?;
        if Some(old.revision) != revision || old.kind != input.kind {
            return Err(conflict());
        }
        protected_standby = is_standby(&old);
        tx.execute("UPDATE pet_artworks SET name=?1,prompt=?2,description=?3,category=?4,enabled=?5,revision=revision+1,updated_at=?6 WHERE id=?7",params![input.name.trim(),input.prompt,input.description,input.category,input.enabled,now,id])?;
        id
    } else {
        if input.kind == "base"
            && library(&tx, persona)?
                .items
                .iter()
                .any(|i| i.kind == "base")
        {
            return Err(AppError::validation(
                "kind",
                "已有初始形象，请在原条目中替换",
            ));
        }
        tx.execute("INSERT INTO pet_artworks(persona_id,kind,name,prompt,description,category,enabled,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",params![persona,input.kind,input.name.trim(),input.prompt,input.description,input.category,input.enabled,now])?;
        tx.last_insert_rowid()
    };
    // 旧库缺图仍可编辑；已有待机图的图片模式不能失去最后一个可用待机条目。
    if protected_standby && library(&tx, persona)?.enabled {
        require_standby(&tx, persona)?;
    }
    changed(&tx, persona)?;
    let result = get(&tx, persona, id)?;
    tx.commit()?;
    Ok(result)
}
fn is_standby(a: &Artwork) -> bool {
    a.kind == "expression" && a.category == "standby" && a.enabled && a.image_file.is_some()
}
fn require_standby(c: &Connection, persona: i64) -> AppResult<()> {
    if !library(c, persona)?.items.iter().any(is_standby) {
        return Err(AppError::validation("standby", "图片模式需要至少一张已接受且允许使用的待机图片；请先补充待机图，或关闭图片模式后再删除、停用或更改最后一张待机图的类别"));
    }
    Ok(())
}
pub fn set_enabled(c: &Connection, persona: i64, enabled: bool, revision: u64) -> AppResult<()> {
    require_current(c, persona)?;
    let l = library(c, persona)?;
    if revision != l.revision {
        return Err(conflict());
    }
    if enabled
        && !l
            .items
            .iter()
            .any(|a| a.kind == "base" && a.image_file.is_some())
    {
        return Err(AppError::validation("enabled", "请先上传并接受初始形象"));
    }
    if enabled { require_standby(c, persona)?; }
    c.execute("UPDATE personas SET pet_image_enabled=?1,pet_image_revision=pet_image_revision+1 WHERE id=?2",params![enabled,persona])?;
    Ok(())
}
/// 保持原图的透明画布与构图，统一尺寸，不裁掉单张图片的留白。
pub fn normalize(bytes: &[u8]) -> AppResult<Vec<u8>> {
    let rgba = crate::accessories::images::decode_transparent(bytes, false).map_err(|_| {
        AppError::new(
            "INVALID_PET_IMAGE",
            "桌宠图片须为静态PNG/WebP，包含可见内容和真实透明区域，不能是不透明底图或完全透明图片",
        )
    })?;
    let resized = image::DynamicImage::ImageRgba8(rgba)
        .resize(254, 254, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let mut canvas = image::RgbaImage::new(256, 256);
    image::imageops::overlay(
        &mut canvas,
        &resized,
        i64::from((256 - resized.width()) / 2),
        i64::from((256 - resized.height()) / 2),
    );
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|_| AppError::storage())?;
    Ok(out.into_inner())
}
pub fn preview(
    runtime: &Runtime,
    persona: i64,
    id: i64,
    revision: u64,
    bytes: &[u8],
) -> AppResult<Artwork> {
    preview_guarded(runtime, persona, id, revision, bytes, None)
}
fn preview_guarded(
    runtime: &Runtime,
    persona: i64,
    id: i64,
    revision: u64,
    bytes: &[u8],
    guard: Option<(u64, u64, &crate::state::Cancellation)>,
) -> AppResult<Artwork> {
    let png = normalize(bytes)?;
    let paths = runtime.paths.as_ref().map_err(Clone::clone)?;
    runtime.read(|s, _| {
        require_current(&s.connection, persona)?;
        if get(&s.connection, persona, id)?.revision != revision {
            return Err(conflict());
        }
        Ok(())
    })?;
    let mut published = None;
    let result = runtime.read(|s, _| {
        require_current(&s.connection, persona)?;
        if let Some((library_revision, config_revision, cancel)) = guard {
            cancel.check()?;
            if library(&s.connection, persona)?.revision != library_revision
                || crate::image_generation::revision(&s.connection)? != config_revision
            {
                return Err(conflict());
            }
        }
        let tx = s.connection.transaction()?;
        let old = get(&tx, persona, id)?;
        if old.revision != revision {
            return Err(conflict());
        }
        // 发布与引用提交共用存储锁，清理服务不能在此间删除未挂接的新文件。
        let file = crate::accessories::images::publish(paths, &png)?;
        published = Some(file.clone());
        tx.execute(
            "UPDATE pet_artworks SET preview_file=?1,revision=revision+1,updated_at=?2 WHERE id=?3",
            params![file, chrono::Utc::now().to_rfc3339(), id],
        )?;
        changed(&tx, persona)?;
        let result = get(&tx, persona, id)?;
        tx.commit()?;
        Ok((result, old.preview_file))
    });
    match result {
        Ok((a, old)) => {
            if let Some(old) = old {
                crate::accessories::images::cleanup_one(runtime, &old, &[]);
            }
            Ok(a)
        }
        Err(e) => {
            if let Some(file) = published {
                crate::accessories::images::cleanup_one(runtime, &file, &[]);
            }
            Err(e)
        }
    }
}
pub fn finish(
    c: &mut Connection,
    persona: i64,
    id: i64,
    revision: u64,
    action: &str,
) -> AppResult<Vec<String>> {
    require_current(c, persona)?;
    let tx = c.transaction()?;
    let old = get(&tx, persona, id)?;
    if old.revision != revision {
        return Err(conflict());
    }
    let mut removed = vec![];
    match action {
        "accept" => {
            let preview = old
                .preview_file
                .as_ref()
                .ok_or_else(|| AppError::validation("preview", "没有待确认的预览"))?;
            tx.execute("UPDATE pet_artworks SET image_file=?1,preview_file=NULL,revision=revision+1,updated_at=?2 WHERE id=?3",params![preview,chrono::Utc::now().to_rfc3339(),id])?;
            removed.extend(old.image_file);
        }
        "discard" => {
            tx.execute(
                "UPDATE pet_artworks SET preview_file=NULL,revision=revision+1,updated_at=?2 WHERE id=?1",
                params![id,chrono::Utc::now().to_rfc3339()],
            )?;
            removed.extend(old.preview_file);
        }
        "delete" => {
            tx.execute("DELETE FROM pet_artworks WHERE id=?1", [id])?;
            if is_standby(&old) && library(&tx, persona)?.enabled { require_standby(&tx, persona)?; }
            if old.kind == "base" {
                tx.execute(
                    "UPDATE personas SET pet_image_enabled=0 WHERE id=?1",
                    [persona],
                )?;
            }
            removed.extend(old.image_file);
            removed.extend(old.preview_file);
        }
        _ => return Err(AppError::validation("action", "无效创作操作")),
    };
    changed(&tx, persona)?;
    tx.commit()?;
    Ok(removed)
}
pub fn image(runtime: &Runtime, persona: i64, id: i64, preview: bool) -> AppResult<String> {
    let a = runtime.read(|s, _| get(&s.connection, persona, id))?;
    let file = if preview {
        a.preview_file
    } else {
        a.image_file
    }
    .ok_or_else(AppError::not_found)?;
    let result =
        crate::accessories::images::read(runtime.paths.as_ref().map_err(Clone::clone)?, &file)?;
    runtime.read(|s, _| {
        let a = get(&s.connection, persona, id)?;
        if (if preview {
            a.preview_file
        } else {
            a.image_file
        })
        .as_ref()
            != Some(&file)
        {
            return Err(conflict());
        }
        Ok(())
    })?;
    Ok(result)
}
pub async fn generate(
    runtime: Runtime,
    persona: i64,
    id: i64,
    revision: u64,
) -> AppResult<Artwork> {
    let lease = runtime
        .requests
        .start(format!("pet-art:{persona}:{id}"), persona)?;
    let (art, base, config, library_revision) = runtime.read(|s, p| {
        require_current(&s.connection, persona)?;
        let a = get(&s.connection, persona, id)?;
        if a.revision != revision {
            return Err(conflict());
        }
        if a.kind != "expression" || a.prompt.trim().is_empty() {
            return Err(AppError::validation(
                "prompt",
                "请先上传初始形象，并为表情填写生成要求",
            ));
        }
        let lib = library(&s.connection, persona)?;
        let base = lib
            .items
            .into_iter()
            .find(|a| a.kind == "base")
            .and_then(|a| a.image_file)
            .ok_or_else(|| AppError::validation("base", "请先接受初始形象"))?;
        let cfg = crate::image_generation::snapshot(&s.connection, p)?
            .ok_or_else(|| AppError::validation("image", "请先在提供商中开启生图并配置模型"))?;
        Ok((a, base, cfg, lib.revision))
    })?;
    let paths = runtime.paths.as_ref().map_err(Clone::clone)?;
    let url = crate::accessories::images::read(paths, &base)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(url.split_once(',').ok_or_else(AppError::storage)?.1)
        .map_err(|_| AppError::storage())?;
    let result = crate::image_generation::openai_compatible::edit(
        &runtime.ai.clone()?,
        &config,
        &bytes,
        &art.prompt,
        &lease.cancel,
    )
    .await?;
    lease.cancel.check()?;
    // 最终检查与预览发布在同一个存储串行区间内复核，配置变化不会成为正式结果。
    runtime.read(|s, _| {
        require_current(&s.connection, persona)?;
        if library(&s.connection, persona)?.revision != library_revision
            || crate::image_generation::revision(&s.connection)? != config.revision
        {
            return Err(conflict());
        }
        Ok(())
    })?;
    let saved = preview_guarded(
        &runtime,
        persona,
        id,
        revision,
        &result,
        Some((library_revision, config.revision, &lease.cancel)),
    )?;
    Ok(saved)
}
pub fn marker(id: &str) -> String {
    format!("@pet-image:{id}")
}
pub fn choices(c: &Connection, persona: i64) -> AppResult<Option<Vec<Artwork>>> {
    let enabled: bool = c.query_row(
        "SELECT pet_image_enabled FROM personas WHERE id=?1",
        [persona],
        |r| r.get(0),
    )?;
    if !enabled {
        return Ok(None);
    }
    let l = library(c, persona)?;
    Ok(Some(
        l.items
            .into_iter()
            .filter(|a| a.image_file.is_some() && (a.enabled || a.kind == "base"))
            .collect(),
    ))
}
/// 停靠专属动作由窗口状态选择，不混入普通聊天的AI表情候选。
pub fn chat_choices(c:&Connection,persona:i64)->AppResult<Option<Vec<Artwork>>> {
    Ok(choices(c,persona)?.map(|items|items.into_iter().filter(|a|!matches!(a.category.as_str(),"peek_left"|"peek_right"|"rest_sit"|"rest_lie"|"walk_left"|"walk_right"|"lifted")).collect()))
}
pub fn select(
    c: &Connection,
    persona: i64,
    category: &str,
    preferred: Option<&str>,
    random: &mut dyn crate::pet::RandomSource,
) -> AppResult<Option<String>> {
    let Some(items) = choices(c, persona)? else {
        return Ok(None);
    };
    let base = items.iter().find(|a| a.kind == "base");
    let selected = if let Some(preferred) = preferred {
        items.iter().find(|a| marker(&a.id) == preferred).or(base)
    } else {
        let candidates = items
            .iter()
            .filter(|a| a.kind == "expression" && (a.category == category || (category.ends_with("_favorite") && !items.iter().any(|x|x.kind=="expression"&&x.category==category) && a.category==category.trim_end_matches("_favorite"))))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            base
        } else {
            candidates
                .get(random.below(candidates.len() as u64)? as usize)
                .copied()
                .or(base)
        }
    };
    Ok(selected.map(|a| marker(&a.id)))
}
#[cfg(test)]
mod tests;
