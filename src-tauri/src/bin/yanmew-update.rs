#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// 独立助手直接复用更新模块，不链接桌宠运行时及其C/C++运行库。
#[cfg(windows)]
#[path="../updates/mod.rs"]
mod updates;
#[cfg(windows)]
fn main(){
    use updates::{self as update,install,network,windows as native};
    use std::{fs,path::PathBuf};
    let args:Vec<_>=std::env::args_os().collect();
    let result=(||->update::Result<()>{
        if args.len()==3&&args[1]=="--uninstall-confirmed" {
            let request=PathBuf::from(&args[2]);install::no_links(&request)?;
            if fs::metadata(&request).map_err(|_|"卸载请求不存在")?.len()>65536{return Err("卸载请求过大".into());}
            let root:PathBuf=serde_json::from_slice(&fs::read(&request).map_err(|_|"无法读取卸载请求")?).map_err(|_|"卸载请求无效")?;
            native::wait_ready(&request)?;
            let root=install::target(&root)?;
            let progress=native::Progress::new();native::uninstall(&root,&|text|progress.set(text))?;drop(progress);native::message("已执行卸载。安装版请继续按正式卸载向导完成；便携版的外部备份和其他文件保留。",false);return Ok(());
        }
        let request=if args.len()==3&&args[1]=="--apply" {
            let path=PathBuf::from(&args[2]);install::no_links(&path)?;
            if fs::metadata(&path).map_err(|_|"更新请求不存在")?.len()>65536{return Err("更新请求过大".into());}
            let request=serde_json::from_slice::<native::ApplyRequest>(&fs::read(&path).map_err(|_|"更新请求不可读")?).map_err(|_|"更新请求格式无效")?;
            native::wait_ready(&path)?;Some(request)
        }else if args.len()==1{None}else{return Err("更新助手参数不支持，请直接双击使用".into());};
        let root=if let Some(request)=&request{install::target(&request.target)?}else{let Some(folder)=native::choose_folder()?else{return Ok(());};install::target(&folder)?};
        let progress=native::Progress::new();
        if root.join(install::PENDING).exists(){let lock=native::wait_and_lock(&root,&|text|progress.set(text))?;install::recover(&root)?;drop(lock);progress.set("已恢复上次未完成的程序替换，正在检查正式版本…");}
        let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|_|"无法启动更新网络服务")?;
        runtime.block_on(async{
            let client=network::client()?;
            let Some(candidate)=network::latest(&client).await? else{return Err("暂无可公开访问的正式Release：维护者尚未发布，或仓库尚未公开。本地程序及资料没有改动。".into());};
            let (version,build)=if root.join(update::PACKAGE_MANIFEST).exists(){let package:update::Package=serde_json::from_slice(&fs::read(root.join(update::PACKAGE_MANIFEST)).map_err(|_|"无法读取当前版本")?).map_err(|_|"当前程序清单损坏")?;(package.version,package.build_id)}else{("0.0.0".into(),"00000000-000000".into())};
            if !update::newer(&candidate.manifest.version,&candidate.manifest.build_id,&version,&build)?{return Err("当前目录已经是这个正式版本或更新版本，没有修改程序。".into());}
            let work=install::workspace()?;
            let archive=if let Some(request)=&request {
                if serde_json::to_value(&request.candidate).ok()!=serde_json::to_value(&candidate).ok(){return Err("Release信息已变化，请重新检查更新；旧程序与资料未修改".into());}request.archive.clone()
            }else{
                let path=work.join("portable.zip");network::download(&client,&candidate,&path,|n,total|progress.set(&format!("正在下载新版：{} / {} MB",n/1048576,total/1048576))).await?;path
            };
            progress.set("正在校验并解压完整更新包…");
            let stage=install::extract(&archive,&work.join("expanded"),&candidate.manifest)?;
            let lock=native::wait_and_lock(&root,&|text|progress.set(text))?;
            install::recover(&root)?;
            let backup=install::apply(&stage,&root,&native::data_root(&root)?,|text|progress.set(text))?;
            drop(lock);native::launch(&root)?;progress.set("更新成功，颜喵已重新启动。");
            native::message(&format!("更新成功，资料已保留。\n更新前完整备份：{}",backup.display()),false);Ok(())
        })
    })();
    if let Err(error)=result{native::message(&error,true);}
}
#[cfg(not(windows))]
fn main(){eprintln!("颜喵更新助手仅支持Windows x64。");}
