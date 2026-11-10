use super::*;
use std::{fs::{self,File,OpenOptions}, os::windows::{fs::OpenOptionsExt,process::CommandExt}, path::PathBuf, process::{Command,Stdio}, sync::mpsc, thread, time::{Duration,Instant}};
use ::windows::{core::{w,HSTRING},Win32::{Foundation::CloseHandle,System::{Com::*,Diagnostics::ToolHelp::*,Threading::*},UI::{Shell::*,WindowsAndMessaging::*}}};

pub fn message(text:&str,error:bool){unsafe{MessageBoxW(None,&HSTRING::from(text),w!("颜喵"),if error{MB_OK|MB_ICONERROR}else{MB_OK|MB_ICONINFORMATION});}}
pub fn wait_ready(request:&Path)->Result<()> {
    let ready=request.with_extension("ready");let start=Instant::now();
    while !ready.exists(){if start.elapsed()>Duration::from_secs(30){return Err("主程序未成功交接退出，已取消操作；程序和资料没有替换或删除".into());}thread::sleep(Duration::from_millis(100));}
    install::no_links(&ready)?;
    if fs::read(ready).map_err(|_|"交接信号不可读")?!=b"ready"{return Err("退出交接信号无效".into());}Ok(())
}
pub fn choose_folder()->Result<Option<PathBuf>> {unsafe{
    let initialized=CoInitializeEx(None,COINIT_APARTMENTTHREADED).is_ok();
    let result=(||{let dialog:IFileOpenDialog=CoCreateInstance(&FileOpenDialog,None,CLSCTX_INPROC_SERVER).map_err(|_|"无法打开目录选择窗口")?;
        dialog.SetOptions(FOS_PICKFOLDERS|FOS_FORCEFILESYSTEM|FOS_PATHMUSTEXIST).map_err(|_|"无法配置目录选择")?;
        dialog.SetTitle(w!("选择旧版颜喵目录（yanwenzi.exe所在文件夹）")).map_err(|_|"无法设置窗口标题")?;
        if dialog.Show(None).is_err(){return Ok(None);}
        let item=dialog.GetResult().map_err(|_|"无法读取所选目录")?;let raw=item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|_|"所选目录不是本地文件夹")?;
        let path=raw.to_string().map_err(|_|"目录名称无效")?;CoTaskMemFree(Some(raw.0.cast()));Ok(Some(PathBuf::from(path)))})();
    if initialized{CoUninitialize();}result
}}
pub fn data_root(root:&Path)->Result<PathBuf>{
    match fs::read_to_string(root.join("yanmew-installed")) {
        Ok(marker) if marker.trim()=="YanMew-installed-v1"=>unsafe{let raw=SHGetKnownFolderPath(&FOLDERID_LocalAppData,KF_FLAG_DEFAULT,None).map_err(|_|"无法定位安装版资料目录")?;let text=raw.to_string().map_err(|_|"资料目录无效")?;CoTaskMemFree(Some(raw.0.cast()));Ok(PathBuf::from(text).join("YanMew/data"))},
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(root.join("data")),
        _=>Err("安装模式标记无效，未触碰任何资料".into()),
    }
}
pub fn running(root:&Path)->Result<bool>{unsafe{
    let snapshot=CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS,0).map_err(|_|"无法检查旧程序进程")?;
    let mut entry=PROCESSENTRY32W{dwSize:std::mem::size_of::<PROCESSENTRY32W>() as u32,..Default::default()};
    let mut more=Process32FirstW(snapshot,&mut entry).is_ok();let expected=root.join("yanwenzi.exe").to_string_lossy().to_lowercase();let mut found=false;
    while more {
        if let Ok(handle)=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,entry.th32ProcessID){
            let mut buffer=vec![0u16;32768];let mut len=buffer.len() as u32;
            if QueryFullProcessImageNameW(handle,PROCESS_NAME_WIN32,::windows::core::PWSTR(buffer.as_mut_ptr()),&mut len).is_ok(){let path=String::from_utf16_lossy(&buffer[..len as usize]);if path.trim_start_matches(r"\\?\").eq_ignore_ascii_case(expected.trim_start_matches(r"\\?\")){found=true;}}
            let _=CloseHandle(handle);
        }
        more=Process32NextW(snapshot,&mut entry).is_ok();
    }let _=CloseHandle(snapshot);Ok(found)
}}
pub fn wait_and_lock(root:&Path,progress:&impl Fn(&str))->Result<File>{
    let data=data_root(root)?;install::no_links(&data)?;
    let start=Instant::now();
    while running(root)? {progress("请从托盘正常退出旧版颜喵；退出后会自动继续，不会强制结束进程。");if start.elapsed()>Duration::from_secs(300){return Err("旧版仍在运行，尚未修改程序或资料；退出后可重试".into());}thread::sleep(Duration::from_millis(500));}
    fs::create_dir_all(&data).map_err(|_|"无法访问资料目录")?;install::no_links(&data.join("instance.lock"))?;
    OpenOptions::new().read(true).write(true).create(true).truncate(false).share_mode(0).open(data.join("instance.lock")).map_err(|_|"资料目录仍被其他进程占用，请正常退出后重试".into())
}
pub fn launch(root:&Path)->Result<()> {
    Command::new(root.join("yanwenzi.exe")).current_dir(root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn().map_err(|_|"更新已完成，但无法自动启动；请打开目录中的yanwenzi.exe")?;Ok(())
}
pub struct Progress {sender:mpsc::Sender<Option<String>>,worker:Option<thread::JoinHandle<()>>}
impl Progress {
    pub fn new()->Self {
        let (sender,receiver)=mpsc::channel::<Option<String>>();
        let worker=thread::spawn(move||unsafe{
            let Ok(window)=CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!("颜喵更新助手"),WS_OVERLAPPED|WS_CAPTION|WS_VISIBLE, CW_USEDEFAULT,CW_USEDEFAULT,640,210,None,None,None,None)else{return;};
            let label=CreateWindowExW(WINDOW_EX_STYLE::default(),w!("STATIC"),w!("正在检查更新，请稍候…"),WS_CHILD|WS_VISIBLE,24,30,580,115,Some(window),None,None,None).ok();
            loop {match receiver.recv_timeout(Duration::from_millis(50)){Ok(Some(text))=>{if let Some(label)=label{let _=SetWindowTextW(label,&HSTRING::from(text));}},Ok(None)|Err(mpsc::RecvTimeoutError::Disconnected)=>break,Err(mpsc::RecvTimeoutError::Timeout)=>{}}
                let mut msg=MSG::default();while PeekMessageW(&mut msg,None,0,0,PM_REMOVE).as_bool(){let _=TranslateMessage(&msg);DispatchMessageW(&msg);}
            }let _=DestroyWindow(window);
        });Self{sender,worker:Some(worker)}
    }
    pub fn set(&self,text:&str){let _=self.sender.send(Some(text.into()));}
}
impl Drop for Progress {fn drop(&mut self){let _=self.sender.send(None);if let Some(worker)=self.worker.take(){let _=worker.join();}}}

fn clear_autostart(root:&Path)->Result<()> {unsafe{
    use ::windows::Win32::System::Registry::*;
    let mut key=HKEY::default();
    if RegOpenKeyExW(HKEY_CURRENT_USER,w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),Some(0),KEY_QUERY_VALUE|KEY_SET_VALUE,&mut key).is_err(){return Ok(());}
    let mut size=0u32;let status=RegQueryValueExW(key,w!("YanMew"),None,None,None,Some(&mut size));
    if status.is_err(){let _=RegCloseKey(key);return Ok(());}
    if size>65536{let _=RegCloseKey(key);return Err("自启登记格式异常，未删除".into());}
    let mut bytes=vec![0u8;size as usize];let result=RegQueryValueExW(key,w!("YanMew"),None,None,Some(bytes.as_mut_ptr()),Some(&mut size));
    let text=String::from_utf16_lossy(&bytes.chunks_exact(2).map(|b|u16::from_le_bytes([b[0],b[1]])).take_while(|c|*c!=0).collect::<Vec<_>>());
    let exe=root.join("yanwenzi.exe").to_string_lossy().trim_start_matches(r"\\?\").to_lowercase();
    let owned=result.is_ok()&&(text.to_lowercase()==exe||text.to_lowercase()==format!("\"{exe}\""));
    if owned {let result=RegDeleteValueW(key,w!("YanMew"));let _=RegCloseKey(key);if result.is_err(){return Err("无法删除当前颜喵的自启项".into());}
        let mut approved=HKEY::default();if RegOpenKeyExW(HKEY_CURRENT_USER,w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run"),Some(0),KEY_SET_VALUE,&mut approved).is_ok(){let _=RegDeleteValueW(approved,w!("YanMew"));let _=RegCloseKey(approved);}
    }else{let _=RegCloseKey(key);}Ok(())
}}
pub fn uninstall(root:&Path,progress:&impl Fn(&str))->Result<()> {
    let root=install::target(root)?;let data=data_root(&root)?;
    let lock=wait_and_lock(&root,progress)?;
    if root.join("yanmew-installed").exists(){drop(lock);let exe=root.join("uninstall.exe");install::no_links(&exe)?;if !exe.is_file(){return Err("安装版卸载程序缺失，请从Windows应用设置修复后卸载".into());}Command::new(exe).current_dir(root).spawn().map_err(|_|"无法打开正式卸载程序")?;return Ok(());}
    let files=install::old_files(&root)?;for file in &files{install::no_links(&root.join(file))?;}install::no_links(&data)?;
    clear_autostart(&root)?;drop(lock);
    // 完整路径已由target和data_root固定，递归删除仅针对本便携版data。
    if data.exists(){fs::remove_dir_all(&data).map_err(|_|"资料目录清理失败，卸载未完成，请保留当前目录")?;}
    for file in files {let path=root.join(file);if path.exists(){fs::remove_file(path).map_err(|_|"程序文件仍被占用，卸载未完成")?;}}
    progress("颜喵程序和资料已清理。目录中的其他文件与外部备份保留。");Ok(())
}

#[derive(Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ApplyRequest {pub target:PathBuf,pub archive:PathBuf,pub candidate:Candidate}
pub fn spawn_helper(root:&Path,argument:&str,value:&Path)->Result<()> {
    let helper=root.join(HELPER);install::no_links(&helper)?;
    let package:Package=serde_json::from_reader(File::open(root.join(PACKAGE_MANIFEST)).map_err(|_|"缺少当前程序清单，请使用完整新版便携包")?).map_err(|_|"当前程序清单损坏")?;install::validate_package(&package)?;
    let entry=package.files.iter().find(|file|file.path==HELPER).ok_or("当前程序缺少更新助手")?;
    if hash_file(&helper)?!=entry.sha256.to_ascii_lowercase(){return Err("更新助手校验失败，请使用完整便携包".into());}
    let work=install::workspace()?;let copy=work.join(HELPER);fs::copy(helper,&copy).map_err(|_|"无法准备独立更新助手")?;
    Command::new(copy).arg(argument).arg(value).current_dir(work).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000).spawn().map_err(|_|"无法启动更新助手，旧程序与资料未修改")?;Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn portable_uninstall_preserves_foreign_files_and_external_backups(){
        let base=install::workspace().unwrap();let root=base.join("便携卸载测试");assert!(root.starts_with(&base));
        fs::create_dir_all(root.join("resources")).unwrap();fs::create_dir_all(root.join("data/plugins/game")).unwrap();
        fs::write(root.join("yanwenzi.exe"),b"MZdummy").unwrap();fs::write(root.join(HELPER),b"MZhelper").unwrap();fs::write(root.join("data/plugins/game/save.json"),b"save").unwrap();fs::write(root.join("用户文件.txt"),b"keep").unwrap();fs::write(root.join("resources/用户文件.txt"),b"keep resource").unwrap();fs::write(base.join("用户导出备份.zip"),b"backup").unwrap();
        let package=Package{schema_version:1,product:"com.yanmew.pet".into(),version:"0.1.0".into(),build_id:"20261110-170000".into(),files:["yanwenzi.exe",HELPER].into_iter().map(|name|PackageFile{path:name.into(),size:fs::metadata(root.join(name)).unwrap().len(),sha256:hash_file(&root.join(name)).unwrap()}).collect()};
        fs::write(root.join(PACKAGE_MANIFEST),serde_json::to_vec(&package).unwrap()).unwrap();
        uninstall(&root,&|_|{}).unwrap();assert!(!root.join("data").exists());assert!(!root.join("yanwenzi.exe").exists());assert!(!root.join(HELPER).exists());assert!(root.join("用户文件.txt").exists());assert!(root.join("resources/用户文件.txt").exists());assert!(base.join("用户导出备份.zip").exists());fs::remove_dir_all(base).unwrap();
    }
}
