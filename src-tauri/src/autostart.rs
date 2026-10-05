//! Windows 登录自启适配。登记属于操作系统，不进入 SQLite 或用户备份。
use crate::error::{AppError, AppResult};
use serde::Serialize;
use tauri::{AppHandle, WebviewWindow};

/// 固定名称不随后续 productName 修改，防止升级产生重复启动项。
#[cfg(windows)]
pub const ENTRY_NAME: &str = "YanMew";

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    supported: bool,
    enabled: bool,
    needs_repair: bool,
    blocked_by_system: bool,
}

fn authorize(label: &str) -> AppResult<()> {
    if label != "settings" {
        return Err(AppError::new(
            "PERMISSION_DENIED",
            "只有设置窗口可以管理开机自启",
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_autostart_status(window: WebviewWindow, app: AppHandle) -> AppResult<Status> {
    authorize(window.label())?;
    tauri::async_runtime::spawn_blocking(move || platform::get(&app))
        .await
        .map_err(|_| failure())?
}

#[tauri::command]
pub async fn set_autostart(
    window: WebviewWindow,
    app: AppHandle,
    enabled: bool,
) -> AppResult<Status> {
    authorize(window.label())?;
    tauri::async_runtime::spawn_blocking(move || platform::set(&app, enabled))
        .await
        .map_err(|_| failure())?
}

fn failure() -> AppError {
    AppError::new(
        "AUTOSTART_ERROR",
        "无法读取或更新开机自启，请检查 Windows 启动应用设置与系统权限后重试",
    )
}

/// 只认本应用生成的完整命令；不解析或执行注册表中的任意命令。
#[cfg(any(windows, test))]
fn status_for(
    command: Option<&str>,
    executable: &str,
    approval: Option<&[u8]>,
) -> AppResult<Status> {
    let blocked = match approval {
        None => false,
        Some(bytes) if bytes.len() >= 12 => match bytes[0] {
            2 | 6 => false,
            3 | 7 => true,
            _ => return Err(failure()),
        },
        Some(_) => return Err(failure()),
    };
    let matches = command.is_some_and(|value| {
        let value = value.trim().replace('/', "\\").to_lowercase();
        let executable = executable.replace('/', "\\").to_lowercase();
        value == format!("\"{executable}\"")
            || (!executable.chars().any(char::is_whitespace) && value == executable)
    });
    Ok(Status {
        supported: true,
        enabled: command.is_some() && matches && !blocked,
        needs_repair: command.is_some() && !matches,
        blocked_by_system: command.is_some() && blocked,
    })
}

#[cfg(windows)]
mod platform {
    use super::*;
    use tauri_plugin_autostart::ManagerExt;
    use windows::{
        core::{w, PCWSTR},
        Win32::{
            Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS},
            System::Registry::*,
        },
    };

    const RUN: PCWSTR = w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run");
    const APPROVED: PCWSTR =
        w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
    const ENTRY: PCWSTR = w!("YanMew");

    // 设置窗只有一个，但多个 IPC 仍须串行，避免读取/写入互相覆盖状态。
    static ACCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn read_value(key: PCWSTR, flags: REG_ROUTINE_FLAGS) -> AppResult<Option<Vec<u8>>> {
        let mut length = 0;
        let code = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key,
                ENTRY,
                flags,
                None,
                None,
                Some(&mut length),
            )
        };
        if code == ERROR_FILE_NOT_FOUND || code == ERROR_PATH_NOT_FOUND {
            return Ok(None);
        }
        if code != ERROR_SUCCESS || length > 65536 {
            return Err(failure());
        }
        let mut bytes = vec![0u8; length as usize];
        let code = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key,
                ENTRY,
                flags,
                None,
                Some(bytes.as_mut_ptr().cast()),
                Some(&mut length),
            )
        };
        if code != ERROR_SUCCESS {
            return Err(failure());
        }
        bytes.truncate(length as usize);
        Ok(Some(bytes))
    }

    fn current_executable() -> AppResult<String> {
        std::env::current_exe()
            .map_err(|_| failure())?
            .into_os_string()
            .into_string()
            .map_err(|_| failure())
    }

    fn read_status() -> AppResult<Status> {
        let raw = read_value(RUN, RRF_RT_REG_SZ)?;
        let command = raw
            .map(|bytes| {
                if bytes.len() % 2 != 0 {
                    return Err(failure());
                }
                let units: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|p| u16::from_le_bytes([p[0], p[1]]))
                    .collect();
                let text = String::from_utf16(&units).map_err(|_| failure())?;
                let text = text.trim_end_matches('\0').to_string();
                if text.contains('\0') {
                    return Err(failure());
                }
                Ok(text)
            })
            .transpose()?;
        let approval = if command.is_some() {
            read_value(APPROVED, RRF_RT_REG_BINARY)?
        } else {
            None
        };
        status_for(
            command.as_deref(),
            &current_executable()?,
            approval.as_deref(),
        )
    }

    pub fn get(_: &AppHandle) -> AppResult<Status> {
        let _guard = ACCESS.lock().map_err(|_| failure())?;
        read_status()
    }

    pub fn set(app: &AppHandle, enabled: bool) -> AppResult<Status> {
        let _guard = ACCESS.lock().map_err(|_| failure())?;
        let before = read_status()?;
        if enabled {
            // 尊重用户在任务管理器的禁用；先由用户在系统设置中恢复。
            if before.blocked_by_system {
                return Ok(before);
            }
            app.autolaunch().enable().map_err(|_| failure())?;
            // auto-launch 0.5 不加引号；统一写安全的完整命令，支持空格/中文目录。
            let command: Vec<u16> = format!("\"{}\"", current_executable()?)
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let code = unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    RUN,
                    ENTRY,
                    REG_SZ.0,
                    Some(command.as_ptr().cast()),
                    (command.len() * 2) as u32,
                )
            };
            if code != ERROR_SUCCESS {
                return Err(failure());
            }
        } else if before.enabled || before.needs_repair || before.blocked_by_system {
            app.autolaunch().disable().map_err(|_| failure())?;
        }
        let after = read_status()?;
        if (enabled && !after.enabled && !after.blocked_by_system)
            || (!enabled && (after.enabled || after.needs_repair || after.blocked_by_system))
        {
            return Err(failure());
        }
        Ok(after)
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub fn get(_: &AppHandle) -> AppResult<Status> {
        Ok(Status {
            supported: false,
            enabled: false,
            needs_repair: false,
            blocked_by_system: false,
        })
    }
    pub fn set(_: &AppHandle, _: bool) -> AppResult<Status> {
        Err(AppError::new("UNSUPPORTED", "当前平台尚未开放开机自启"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_settings_can_manage_autostart() {
        assert!(authorize("settings").is_ok());
        for label in ["main", "chat", "memory", "game-test"] {
            assert!(authorize(label).is_err());
        }
    }

    #[test]
    fn missing_current_moved_and_unquoted_paths() {
        let exe = r"C:\我的 应用\yanwenzi.exe";
        assert!(!status_for(None, exe, None).unwrap().enabled);
        assert!(
            status_for(Some(&format!("\"{exe}\" ")), exe, None)
                .unwrap()
                .enabled
        );
        assert!(
            status_for(Some(r#""C:\旧目录\yanwenzi.exe""#), exe, None)
                .unwrap()
                .needs_repair
        );
        assert!(status_for(Some(exe), exe, None).unwrap().needs_repair);
        assert!(
            status_for(Some(r"c:\APP\yanwenzi.exe "), r"C:\app\yanwenzi.exe", None)
                .unwrap()
                .enabled
        );
        assert!(
            status_for(Some(&format!("\"{exe}\" --unexpected")), exe, None)
                .unwrap()
                .needs_repair
        );
    }

    #[test]
    fn system_disabled_and_malformed_approval_are_not_reported_enabled() {
        let command = r#""C:\app\yanwenzi.exe""#;
        for state in [2, 3, 6, 7] {
            let mut approval = [0; 12];
            approval[0] = state;
            let result =
                status_for(Some(command), r"C:\app\yanwenzi.exe", Some(&approval)).unwrap();
            assert_eq!(result.enabled, state == 2 || state == 6);
            assert_eq!(result.blocked_by_system, state == 3 || state == 7);
        }
        assert!(status_for(Some(command), r"C:\app\yanwenzi.exe", Some(&[3])).is_err());
        assert!(status_for(Some(command), r"C:\app\yanwenzi.exe", Some(&[0; 12])).is_err());
    }
}
