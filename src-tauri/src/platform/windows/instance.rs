//! 每个实际 data 目录持有一个 Windows 文件写锁；重复启动只通知已有宠窗。
use std::{fs::{File, OpenOptions}, io::{self, Write}, os::windows::fs::OpenOptionsExt, path::Path, sync::OnceLock, time::Duration};
use windows::{core::PCWSTR, Win32::{Foundation::{HWND, LPARAM, WPARAM}, UI::WindowsAndMessaging::{AllowSetForegroundWindow, GetWindowThreadProcessId, PostMessageW, RegisterWindowMessageW}}};

pub struct Instance { file: File }

pub fn wake_message() -> u32 {
    static MESSAGE: OnceLock<u32> = OnceLock::new();
    *MESSAGE.get_or_init(|| {
        let name: Vec<u16> = "YanMew.ShowDataInstance.v1\0".encode_utf16().collect();
        unsafe { RegisterWindowMessageW(PCWSTR(name.as_ptr())) }
    })
}

fn claim(path: &Path) -> io::Result<File> {
    // 允许另一进程读取唤醒地址，但拒绝第二个写入者；崩溃后由系统释放。
    OpenOptions::new().create(true).truncate(true).read(true).write(true).share_mode(1).open(path)
}

pub fn acquire(data: &Path) -> io::Result<Option<Instance>> {
    std::fs::create_dir_all(data)?;
    if wake_message() == 0 { return Err(io::Error::other("无法注册宠窗唤醒消息")); }
    let path = data.join("instance.lock");
    match claim(&path) {
        Ok(file) => Ok(Some(Instance { file })),
        Err(error) if error.raw_os_error() == Some(32) => {
            // 首次启动可能仍在初始化数据库/资源；此进程绝不打开数据库。
            for _ in 0..100 {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if !text.ends_with('\n') { std::thread::sleep(Duration::from_millis(100)); continue; }
                    let parts: Vec<_> = text.split_whitespace().collect();
                    if let [pid, handle] = parts.as_slice() {
                        if let (Ok(pid), Ok(handle)) = (pid.parse::<u32>(), handle.parse::<usize>()) {
                            let hwnd = HWND(handle as *mut _);
                            let mut actual_pid = 0;
                            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut actual_pid)); }
                            if pid == actual_pid && pid != 0 {
                                unsafe {
                                    let _ = AllowSetForegroundWindow(pid);
                                    PostMessageW(Some(hwnd), wake_message(), WPARAM(0), LPARAM(0)).map_err(io::Error::other)?;
                                }
                                return Ok(None);
                            }
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(io::Error::other("这份数据的颜喵仍在启动或退出，请稍后再打开；没有启动第二个实例"))
        }
        Err(error) => Err(error),
    }
}

impl Instance {
    pub fn publish(&self, window: &tauri::WebviewWindow) -> io::Result<()> {
        let hwnd = window.hwnd().map_err(io::Error::other)?;
        let mut file = &self.file;
        file.write_all(format!("{} {}\n", std::process::id(), hwnd.0 as usize).as_bytes())?;
        file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_lock_is_per_directory_readable_and_released_on_drop() {
        let root = std::env::temp_dir().join(format!("yanmew-instance-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        std::fs::create_dir_all(&root).unwrap();
        let a=root.join("a");let b=root.join("b");
        let first=claim(&a).unwrap();
        let mut writer=&first; writer.write_all(b"123 456").unwrap(); writer.flush().unwrap();
        assert_eq!(claim(&a).unwrap_err().raw_os_error(),Some(32));
        assert_eq!(std::fs::read_to_string(&a).unwrap(),"123 456");
        let independent=claim(&b).unwrap();
        drop(first); let reopened=claim(&a).unwrap();
        drop(reopened);drop(independent);std::fs::remove_file(a).unwrap();std::fs::remove_file(b).unwrap();std::fs::remove_dir(root).unwrap();
    }
}
