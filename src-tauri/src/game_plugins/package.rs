//! 包校验共用于安装、加载和作者命令行工具；解压前完成全部检查。
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
pub const MAX_PACKAGE: usize = 64 * 1024 * 1024;
pub const MAX_EXPANDED: usize = 128 * 1024 * 1024;
pub const MAX_FILE: usize = 32 * 1024 * 1024;
pub const MAX_FILES: usize = 4096;
#[derive(Clone)]
pub struct Package {
    pub manifest: Manifest,
    pub sha: String,
    pub bytes: Arc<Vec<u8>>,
    pub files: Arc<BTreeMap<String, Vec<u8>>>,
}
pub fn invalid() -> AppError {
    AppError::new(
        "PLUGIN_PACKAGE_INVALID",
        "插件包损坏、资源不支持或路径不安全",
    )
}
pub fn mime(path: &str) -> Option<&'static str> {
    Some(match path.rsplit('.').next()? {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => return None,
    })
}
pub fn validate(bytes: Vec<u8>) -> AppResult<Package> {
    if bytes.is_empty() || bytes.len() > MAX_PACKAGE {
        return Err(invalid());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).map_err(|_| invalid())?;
    if archive.len() > MAX_FILES {
        return Err(invalid());
    }
    let mut files = BTreeMap::new();
    let mut names = HashSet::new();
    let mut expanded = 0usize;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|_| invalid())?;
        let name = file.name().to_owned();
        let clean = if file.is_dir() {
            name.trim_end_matches('/')
        } else {
            &name
        };
        manifest::path(clean)?;
        if !names.insert(clean.to_ascii_lowercase())
            || file.enclosed_name().is_none()
            || file.unix_mode().is_some_and(|m| {
                m & 0o170000 != 0 && m & 0o170000 != 0o100000 && m & 0o170000 != 0o040000
            })
            || file.is_symlink()
        {
            return Err(invalid());
        }
        if file.is_dir() {
            continue;
        }
        if mime(&name).is_none() || file.size() > MAX_FILE as u64 {
            return Err(invalid());
        }
        let size = usize::try_from(file.size()).map_err(|_| invalid())?;
        expanded = expanded.checked_add(size).ok_or_else(invalid)?;
        if expanded > MAX_EXPANDED {
            return Err(invalid());
        }
        let mut data = Vec::with_capacity(size);
        file.by_ref()
            .take((MAX_FILE + 1) as u64)
            .read_to_end(&mut data)
            .map_err(|_| invalid())?;
        if data.len() != size {
            return Err(invalid());
        }
        files.insert(name, data);
    }
    let raw = files.get("manifest.json").ok_or_else(invalid)?;
    if raw.len() > 64 * 1024 {
        return Err(invalid());
    }
    let manifest: Manifest = serde_json::from_slice(raw).map_err(|_| invalid())?;
    manifest.validate()?;
    if !files.contains_key(&manifest.entry)
        || manifest
            .migration
            .as_ref()
            .is_some_and(|p| !files.contains_key(p))
    {
        return Err(invalid());
    }
    let sha = format!("{:x}", Sha256::digest(&bytes));
    Ok(Package {
        manifest,
        sha,
        bytes: Arc::new(bytes),
        files: Arc::new(files),
    })
}
pub fn root(paths: &crate::platform::contracts::AppPaths) -> PathBuf {
    paths.data_root.join("plugins")
}
/// 数据库内路径只允许安装器生成的 versions/<sha>，不接受绝对路径或任意文件。
pub fn version_path(root: &Path, relative: &str) -> AppResult<PathBuf> {
    let Some(sha) = relative.strip_prefix("versions/") else {
        return Err(invalid());
    };
    if sha.len() != 64
        || !sha
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(invalid());
    }
    let path = root.join("versions").join(sha);
    for p in [root.to_path_buf(), root.join("versions"), path.clone()] {
        if std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_symlink() || is_reparse(&m))
        {
            return Err(invalid());
        }
    }
    Ok(path)
}
#[cfg(windows)]
fn is_reparse(m: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    m.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_: &std::fs::Metadata) -> bool {
    false
}
pub fn load(root: &Path, p: &Plugin) -> AppResult<Package> {
    let dir = version_path(root, p.package_path.as_deref().ok_or_else(invalid)?)?;
    let file = dir.join("package.yanmew-game");
    let meta = std::fs::symlink_metadata(&file).map_err(|_| invalid())?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || is_reparse(&meta)
        || meta.len() > MAX_PACKAGE as u64
    {
        return Err(invalid());
    }
    let pkg = validate(std::fs::read(file).map_err(|_| invalid())?)?;
    if pkg.sha != p.package_sha256
        || pkg.manifest.id != p.plugin_id
        || pkg.manifest.version != p.installed_version
        || pkg.manifest.storage_version != p.storage_version
    {
        return Err(invalid());
    }
    Ok(pkg)
}
/// 发布新不可变目录在数据库提交之前；不覆盖旧版本。残留目录由启动清理。
pub fn publish(root: &Path, pkg: &Package) -> AppResult<String> {
    std::fs::create_dir_all(root.join("versions")).map_err(|_| AppError::storage())?;
    let relative = format!("versions/{}", pkg.sha);
    let final_dir = version_path(root, &relative)?;
    if final_dir.exists() {
        let raw = std::fs::read(final_dir.join("package.yanmew-game")).map_err(|_| invalid())?;
        if raw.as_slice() != pkg.bytes.as_slice() {
            return Err(invalid());
        }
        return Ok(relative);
    }
    let staged = root.join(format!("staging-{}", pkg.sha));
    if staged.exists() {
        safe_remove(root, &staged)?;
    }
    std::fs::create_dir(&staged).map_err(|_| AppError::storage())?;
    let result = (|| {
        let mut f = std::fs::File::create(staged.join("package.yanmew-game"))
            .map_err(|_| AppError::storage())?;
        f.write_all(&pkg.bytes).map_err(|_| AppError::storage())?;
        f.sync_all().map_err(|_| AppError::storage())?;
        for (name, data) in pkg.files.iter() {
            let p = staged.join(name);
            std::fs::create_dir_all(p.parent().ok_or_else(invalid)?)
                .map_err(|_| AppError::storage())?;
            let mut f = std::fs::File::create(p).map_err(|_| AppError::storage())?;
            f.write_all(data).map_err(|_| AppError::storage())?;
            f.sync_all().map_err(|_| AppError::storage())?;
        }
        drop(f); // Windows 重命名目录前关闭包文件句柄。
        std::fs::rename(&staged, &final_dir).map_err(|_| AppError::storage())?;
        Ok(relative)
    })();
    if result.is_err() {
        let _ = safe_remove(root, &staged);
    }
    result
}
fn safe_remove(root: &Path, target: &Path) -> AppResult<()> {
    for p in [root, target] {
        let m = std::fs::symlink_metadata(p).map_err(|_| AppError::storage())?;
        if m.file_type().is_symlink() || is_reparse(&m) {
            return Err(invalid());
        }
    }
    let root = root.canonicalize().map_err(|_| AppError::storage())?;
    let target = target.canonicalize().map_err(|_| AppError::storage())?;
    if target == root || !target.starts_with(&root) {
        return Err(invalid());
    }
    // 不递归进入 junction / symlink；发现异常时停止清理，绝不跟随外部目录。
    fn check(p: &Path) -> AppResult<()> {
        let m = std::fs::symlink_metadata(p).map_err(|_| AppError::storage())?;
        if m.file_type().is_symlink() || is_reparse(&m) {
            return Err(invalid());
        }
        if m.is_dir() {
            for entry in std::fs::read_dir(p).map_err(|_| AppError::storage())? {
                check(&entry.map_err(|_| AppError::storage())?.path())?;
            }
        }
        Ok(())
    }
    check(&target)?;
    std::fs::remove_dir_all(target).map_err(|_| AppError::storage())
}
pub fn cleanup(root: &Path, referenced: &HashSet<String>) -> AppResult<()> {
    if !root.exists() {
        return Ok(());
    }
    for e in std::fs::read_dir(root).map_err(|_| AppError::storage())? {
        let e = e.map_err(|_| AppError::storage())?;
        if e.file_name().to_string_lossy().starts_with("staging-") {
            safe_remove(root, &e.path())?;
        }
    }
    let versions = root.join("versions");
    if versions.exists() {
        for e in std::fs::read_dir(&versions).map_err(|_| AppError::storage())? {
            let e = e.map_err(|_| AppError::storage())?;
            let relative = format!("versions/{}", e.file_name().to_string_lossy());
            version_path(root, &relative)?;
            if !referenced.contains(&relative) {
                safe_remove(root, &e.path())?;
            }
        }
    }
    let profiles = root.join("webview-profiles");
    if profiles.exists() {
        if let Ok(entries) = std::fs::read_dir(&profiles) {
            for e in entries.flatten() {
                let _ = cleanup_profile(root, &e.file_name().to_string_lossy());
            }
        }
    }
    Ok(())
}

/// 仅删除宿主生成的实例缓存；失败在下次启动再尝试，绝不触碰正式存档。
pub fn cleanup_profile(root: &Path, label: &str) -> AppResult<()> {
    let suffix = label
        .strip_prefix("game-")
        .or_else(|| label.strip_prefix("migration-"))
        .ok_or_else(invalid)?;
    if suffix.len() != 32 || !suffix.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    let target = root.join("webview-profiles").join(label);
    if target.exists() {
        safe_remove(root, &target)?;
    }
    Ok(())
}
