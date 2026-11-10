#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    if let Ok(exe)=std::env::current_exe() {
        if exe.parent().is_some_and(|root|root.join(yanwenzi_lib::updates::install::PENDING).exists()) {
            yanwenzi_lib::updates::windows::message("上次程序更新尚未完成。请先运行颜喵更新助手恢复旧程序；未启动数据库或后台任务。",true);
            return;
        }
    }
    yanwenzi_lib::run();
}
