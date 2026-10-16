use crate::{
    error::{AppError, AppResult},
    platform::contracts::AppPaths,
};
use std::path::Path;

const INSTALLED_MARKER: &str = "yanmew-installed";
const INSTALLED_CONTENT: &str = "YanMew-installed-v1";

fn installed(directory: &Path) -> AppResult<bool> {
    match std::fs::read_to_string(directory.join(INSTALLED_MARKER)) {
        Ok(content) if content.trim() == INSTALLED_CONTENT => Ok(true),
        Ok(_) => Err(AppError::new("INSTALL_MODE_INVALID", "安装模式标记无效，请修复安装；没有另建数据目录")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(AppError::new("INSTALL_MODE_INVALID", "无法读取安装模式标记，请检查目录权限")),
    }
}

fn paths(directory: &Path, installed: bool, local: Option<&Path>) -> AppResult<AppPaths> {
    let data_root = if installed {
        local.ok_or_else(|| AppError::new("DATA_DIRECTORY_UNAVAILABLE", "无法定位当前用户的数据目录"))?.join("YanMew/data")
    } else {
        directory.join("data")
    };
    Ok(AppPaths { data_root, resource_root: directory.join("resources") })
}

fn local_app_data() -> AppResult<std::path::PathBuf> {
    use windows::Win32::{System::Com::CoTaskMemFree, UI::Shell::{SHGetKnownFolderPath, FOLDERID_LocalAppData, KF_FLAG_DEFAULT}};
    let raw = unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }
        .map_err(|_| AppError::new("DATA_DIRECTORY_UNAVAILABLE", "无法定位当前用户的数据目录"))?;
    let result = unsafe { raw.to_string() }.map(std::path::PathBuf::from).map_err(|_| AppError::storage());
    unsafe { CoTaskMemFree(Some(raw.0.cast())); }
    result
}

pub fn application_paths() -> AppResult<AppPaths> {
    let exe = std::env::current_exe().map_err(|_| AppError::storage())?;
    let parent = exe.parent().ok_or_else(AppError::storage)?;
    let is_installed = installed(parent)?;
    let local = if is_installed { Some(local_app_data()?) } else { None };
    paths(parent, is_installed, local.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_data_and_resources_are_independent_of_install_drive() {
        let directory = Path::new(r"D:\Apps\颜喵");
        let local = Path::new(r"C:\Users\测试\AppData\Local");
        let installed = paths(directory, true, Some(local)).unwrap();
        assert_eq!(installed.data_root, local.join("YanMew/data"));
        assert_eq!(installed.resource_root, directory.join("resources"));
        let portable = paths(directory, false, None).unwrap();
        assert_eq!(portable.data_root, directory.join("data"));
        assert_eq!(portable.resource_root, installed.resource_root);
        assert!(paths(directory, true, None).is_err());
    }
    #[test]
    fn marker_is_explicit_and_invalid_marker_does_not_open_another_database() {
        let directory = std::env::temp_dir().join(format!("yanmew-install-path-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&directory).unwrap();
        assert!(!installed(&directory).unwrap());
        let marker = directory.join(INSTALLED_MARKER);
        std::fs::write(&marker, INSTALLED_CONTENT).unwrap();
        assert!(installed(&directory).unwrap());
        std::fs::write(&marker, "unknown-mode").unwrap();
        assert!(installed(&directory).is_err());
        std::fs::remove_file(marker).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
