use crate::{error::{AppError,AppResult},updates::{self,install,network,Candidate},runtime::Runtime};
use serde::Serialize;
use std::sync::{Mutex,atomic::{AtomicBool,Ordering}};
use tauri::{AppHandle,Manager,State,WebviewWindow};

#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct Snapshot {phase:String,message:String,checked_at:Option<String>,downloaded:u64,total:u64,candidate:Option<Candidate>}
impl Default for Snapshot {fn default()->Self{Self{phase:"idle".into(),message:"每次启动会自动检查一次，也可以手动检查。".into(),checked_at:None,downloaded:0,total:0,candidate:None}}}
#[derive(Default)]
pub struct Service {snapshot:Mutex<Snapshot>,busy:AtomicBool}
fn error(message:impl Into<String>)->AppError {AppError::new("APP_UPDATE_FAILED",message)}
fn authorize(window:&WebviewWindow)->AppResult<()> {if window.label()!="settings"{Err(AppError::new("PERMISSION_DENIED","请从设置中的关于颜喵操作"))}else{Ok(())}}
fn set(app:&AppHandle,phase:&str,message:&str){if let Ok(mut state)=app.state::<Service>().snapshot.lock(){state.phase=phase.into();state.message=message.into();}}
struct Busy<'a>(&'a AtomicBool);impl Drop for Busy<'_>{fn drop(&mut self){self.0.store(false,Ordering::Release);}}

pub async fn check(app:AppHandle)->AppResult<Snapshot>{
    let service=app.state::<Service>();
    if service.busy.swap(true,Ordering::AcqRel){return service.snapshot.lock().map(|s|s.clone()).map_err(|_|error("无法读取更新状态"));}
    let _busy=Busy(&service.busy);set(&app,"checking","正在检查GitHub正式发布版本…");
    if let Ok(mut state)=service.snapshot.lock(){state.candidate=None;state.downloaded=0;state.total=0;}
    let result=async {network::latest(&network::client()?).await}.await;
    let mut state=service.snapshot.lock().map_err(|_|error("无法保存更新状态"))?;state.checked_at=Some(chrono::Utc::now().to_rfc3339());
    match result {
        Ok(None)=>{state.phase="no_release".into();state.message="暂无可公开访问的正式发布版本：可能尚未发布Release，或仓库尚未公开。".into();},
        Ok(Some(candidate))=>match updates::newer(&candidate.manifest.version,&candidate.manifest.build_id,updates::VERSION,updates::BUILD_ID){
            Ok(true)=>{state.phase="available".into();state.message="发现新构建，可下载并更新。".into();state.candidate=Some(candidate);},
            Ok(false)=>{state.phase="up_to_date".into();state.message="当前版本不低于最新正式发布版本。".into();},
            Err(message)=>{state.phase="error".into();state.message=message;},
        },
        Err(message)=>{state.phase="error".into();state.message=message;},
    }Ok(state.clone())
}
#[tauri::command]
pub fn get_app_update_status(window:WebviewWindow,service:State<'_,Service>)->AppResult<Snapshot>{authorize(&window)?;service.snapshot.lock().map(|s|s.clone()).map_err(|_|error("无法读取更新状态"))}
#[tauri::command]
pub async fn check_app_update(window:WebviewWindow,app:AppHandle)->AppResult<Snapshot>{authorize(&window)?;check(app).await}

#[tauri::command]
pub async fn install_app_update(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,confirmed_by_user:bool)->AppResult<()> {
    authorize(&window)?;if !confirmed_by_user{return Err(AppError::new("PERMISSION_DENIED","更新需要用户主动确认"));}
    let service=app.state::<Service>();if service.busy.swap(true,Ordering::AcqRel){return Err(error("更新检查或下载正在进行"));}let _busy=Busy(&service.busy);
    let candidate=service.snapshot.lock().map_err(|_|error("无法读取更新版本"))?.candidate.clone().ok_or_else(||error("请先检查并确认存在新版本"))?;
    set(&app,"downloading","正在下载完整更新包；完成校验前不会退出颜喵…");
    let result:updates::Result<()>=async {
        let work=install::workspace()?;let archive=work.join("portable.zip");
        network::download(&network::client()?,&candidate,&archive,|received,total|{if let Ok(mut state)=service.snapshot.lock(){state.downloaded=received;state.total=total;}}).await?;
        set(&app,"downloading","下载完成，正在校验包内文件…");
        let expected=candidate.manifest.clone();let verify_archive=archive.clone();let expanded=work.join("verified");
        tauri::async_runtime::spawn_blocking(move||install::extract(&verify_archive,&expanded,&expected)).await.map_err(|_|"解包校验任务未完成")??;
        #[cfg(windows)] {
            let executable=std::env::current_exe().map_err(|_|"无法定位当前程序")?;let root=executable.parent().ok_or("程序目录无效")?.to_path_buf();
            let request=updates::windows::ApplyRequest{target:root.clone(),archive,candidate};let path=work.join("request.json");
            std::fs::write(&path,serde_json::to_vec(&request).map_err(|_|"无法生成更新请求")?).map_err(|_|"无法保存更新请求")?;
            // 先验证并启动已校验的助手；助手等待本实例正常退出，不强杀进程。
            updates::windows::spawn_helper(&root,"--apply",&path)?;
            runtime.shutdown().map_err(|_|"退出前保存失败；助手会等待，旧程序和资料尚未替换")?;
            std::fs::write(path.with_extension("ready"),b"ready").map_err(|_|"无法交接更新，助手不会替换程序；请重新启动颜喵后重试")?;
            set(&app,"installing","正在正常退出，更新助手将自动备份、替换并重新启动…");app.exit(0);Ok(())
        }
        #[cfg(not(windows))] {let _=runtime;Err("自动更新暂仅支持Windows x64".into())}
    }.await;
    if let Err(message)=result {set(&app,"error",&message);return Err(error(message));}Ok(())
}
#[tauri::command]
pub fn open_app_information_link(window:WebviewWindow,app:AppHandle,kind:String)->AppResult<()> {
    authorize(&window)?;
    let value=match kind.as_str(){"repository"=>updates::REPOSITORY,"releases"=>updates::RELEASES,"issues"=>"https://github.com/xianyvqiu/yanmew/issues","community"=>"https://github.com/xianyvqiu/yanmew-community-games",
        "updateRelease"=>{let state=app.state::<Service>();let url=state.snapshot.lock().map_err(|_|error("无法读取发布链接"))?.candidate.as_ref().map(|c|c.release_url.clone()).ok_or_else(||error("请先检查新版本"))?;return crate::platform::windows::links::open(&url);},
        "data"=>{
            #[cfg(windows)] {let exe=std::env::current_exe().map_err(|_|error("无法定位资料目录"))?;let data=updates::windows::data_root(exe.parent().ok_or_else(||error("程序目录无效"))?).map_err(error)?;return crate::platform::windows::links::open(&data.to_string_lossy());}
            #[cfg(not(windows))] {return Err(error("当前平台不支持打开资料目录"));}
        },_=>return Err(AppError::validation("kind","不支持此信息入口"))};
    crate::platform::windows::links::open(value)
}
#[tauri::command]
pub fn uninstall_app(window:WebviewWindow,app:AppHandle,runtime:State<'_,Runtime>,confirmed_by_user:bool)->AppResult<()> {
    authorize(&window)?;if !confirmed_by_user{return Err(AppError::new("PERMISSION_DENIED","卸载需要确认永久删除全部资料"));}
    #[cfg(windows)] {
        let exe=std::env::current_exe().map_err(|_|error("无法定位程序"))?;let root=exe.parent().ok_or_else(||error("程序目录无效"))?;
        let work=install::workspace().map_err(error)?;let request=work.join("uninstall.json");
        std::fs::write(&request,serde_json::to_vec(root).map_err(|_|error("无法准备卸载请求"))?).map_err(|_|error("无法保存卸载请求"))?;
        updates::windows::spawn_helper(root,"--uninstall-confirmed",&request).map_err(error)?;
        runtime.shutdown()?;
        std::fs::write(request.with_extension("ready"),b"ready").map_err(|_|error("卸载交接失败，资料未删除；请重新启动后重试"))?;
        app.exit(0);Ok(())
    }
    #[cfg(not(windows))] {let _=(app,runtime);Err(error("卸载暂仅支持Windows"))}
}
