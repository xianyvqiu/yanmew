//! 默认浏览器打开已由业务层校验的 HTTP/HTTPS 来源；不拼接 shell 命令。
use crate::error::{AppError, AppResult};
#[cfg(windows)]
pub fn open(url: &str) -> AppResult<()> {
    use windows::{
        core::{w, HSTRING, PCWSTR},
        Win32::{
            Foundation::HWND,
            UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        },
    };
    let target = HSTRING::from(url);
    let result = unsafe {
        ShellExecuteW(
            Some(HWND::default()),
            w!("open"),
            PCWSTR(target.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        Err(AppError::new(
            "OPEN_URL_FAILED",
            "无法打开默认浏览器，请复制来源链接手动访问",
        ))
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
pub fn open(_url: &str) -> AppResult<()> {
    Err(AppError::new(
        "PLATFORM_UNSUPPORTED",
        "当前平台尚未接入默认浏览器打开功能",
    ))
}
