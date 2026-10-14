//! Windows系统保存窗口及同目录文件发布；不通过shell写文件。
use crate::{
    error::{AppError, AppResult},
    platform::contracts::FileTransferPort,
};
use std::path::{Path, PathBuf};
use windows::{
    core::{w, HSTRING, PCWSTR},
    Win32::{
        Foundation::HWND,
        Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{
            Common::COMDLG_FILTERSPEC, FileOpenDialog, FileSaveDialog, IFileDialog,
            IFileOpenDialog, IFileSaveDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
            FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
        },
    },
};

pub struct BackupFiles {
    pub owner: isize,
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn dialog_error() -> AppError {
    AppError::new("FILE_DIALOG_FAILED", "无法打开系统文件窗口，请重试")
}
fn choose(owner: isize, name: Option<String>) -> AppResult<Option<PathBuf>> {
    // 使用独立STA线程，不改变Tauri阻塞线程池的COM初始化方式。
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|_| dialog_error())?;
        let _apartment = Apartment;
        let opening = name.is_none();
        let dialog: IFileDialog = if opening {
            let open: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
                    .map_err(|_| dialog_error())?;
            windows::core::Interface::cast(&open).map_err(|_| dialog_error())?
        } else {
            let save: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
                    .map_err(|_| dialog_error())?;
            windows::core::Interface::cast(&save).map_err(|_| dialog_error())?
        };
        dialog
            .SetOptions(
                dialog.GetOptions().map_err(|_| dialog_error())?
                    | FOS_FORCEFILESYSTEM
                    | FOS_PATHMUSTEXIST
                    | if opening {
                        FOS_FILEMUSTEXIST
                    } else {
                        FOS_OVERWRITEPROMPT
                    },
            )
            .map_err(|_| dialog_error())?;
        dialog
            .SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: w!("颜喵备份 ZIP"),
                pszSpec: w!("*.zip"),
            }])
            .map_err(|_| dialog_error())?;
        dialog
            .SetDefaultExtension(w!("zip"))
            .map_err(|_| dialog_error())?;
        if let Some(name) = name {
            dialog
                .SetFileName(&HSTRING::from(name))
                .map_err(|_| dialog_error())?;
        }
        if let Err(error) = dialog.Show(Some(HWND(owner as *mut _))) {
            if error.code().0 == 0x800704C7u32 as i32 {
                return Ok(None);
            }
            return Err(dialog_error());
        }
        let item = dialog.GetResult().map_err(|_| dialog_error())?;
        let raw = item
            .GetDisplayName(SIGDN_FILESYSPATH)
            .map_err(|_| dialog_error())?;
        let path = raw
            .to_string()
            .map(PathBuf::from)
            .map_err(|_| dialog_error());
        CoTaskMemFree(Some(raw.0.cast()));
        path.map(Some)
    }
}
impl FileTransferPort for BackupFiles {
    fn save_backup(&self, suggested_name: &str) -> AppResult<Option<PathBuf>> {
        let owner = self.owner;
        let name = suggested_name.to_owned();
        std::thread::Builder::new()
            .name("yanmew-backup-dialog".into())
            .spawn(move || choose(owner, Some(name)))
            .map_err(|_| dialog_error())?
            .join()
            .map_err(|_| dialog_error())?
    }
    fn open_backup(&self) -> AppResult<Option<PathBuf>> {
        let owner = self.owner;
        std::thread::Builder::new()
            .name("yanmew-backup-open".into())
            .spawn(move || choose(owner, None))
            .map_err(|_| dialog_error())?
            .join()
            .map_err(|_| dialog_error())?
    }
    fn publish(&self, temporary: &Path, destination: &Path) -> AppResult<()> {
        let source = HSTRING::from(temporary.as_os_str());
        let target = HSTRING::from(destination.as_os_str());
        unsafe {
            MoveFileExW(
                PCWSTR(source.as_ptr()),
                PCWSTR(target.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|_| {
            AppError::new(
                "BACKUP_PUBLISH_FAILED",
                "备份文件保存失败，原备份已保留；请检查文件占用与磁盘权限",
            )
        })
    }
}
