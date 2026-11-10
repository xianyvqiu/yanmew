import { useEffect, useRef, useState } from "react";
import { appApi, errorMessage, type AppInfo, type AppUpdateStatus } from "../../api";
import "./AppInfoPanel.css";

export default function AppInfoPanel() {
  const [info,setInfo]=useState<AppInfo|null>(null),[status,setStatus]=useState<AppUpdateStatus|null>(null);
  const [error,setError]=useState(""),[actionError,setActionError]=useState("");
  const [attempt,setAttempt]=useState(0),[busy,setBusy]=useState(false),[confirmUninstall,setConfirmUninstall]=useState(false);
  const locked=useRef(false),alive=useRef(false),dialog=useRef<HTMLDialogElement>(null);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  useEffect(()=>{
    let disposed=false,generation=0;setError("");
    void appApi.info().then(value=>{if(!disposed)setInfo(value);}).catch(cause=>{if(!disposed)setError(errorMessage(cause));});
    const read=async()=>{const token=++generation;try{const value=await appApi.updateStatus();if(!disposed&&token===generation)setStatus(value);}catch(cause){if(!disposed&&token===generation)setError(errorMessage(cause));}};
    void read();const timer=window.setInterval(()=>{void read();},1000);
    return()=>{disposed=true;generation++;window.clearInterval(timer);};
  },[attempt]);
  useEffect(()=>{if(confirmUninstall)dialog.current?.showModal();else dialog.current?.close();},[confirmUninstall]);
  const act=async(operation:()=>Promise<unknown>)=>{
    if(locked.current)return;locked.current=true;setBusy(true);setActionError("");
    try{await operation();}catch(cause){if(alive.current)setActionError(errorMessage(cause));}
    finally{locked.current=false;if(alive.current)setBusy(false);}
  };
  const updating=status?.phase==="downloading"||status?.phase==="installing";
  const blocked=busy||updating||status?.phase==="checking";
  const link=(kind:"repository"|"community"|"issues"|"releases"|"updateRelease",title:string,url:string)=><a href={url} onClick={event=>{event.preventDefault();void act(()=>appApi.openInformation(kind));}}>{title}</a>;
  return <div className="ym-app-info">
    <section className="ym-page-card" aria-label="关于颜喵">
      <div className="ym-page-heading"><div><h2>关于颜喵</h2><p className="ym-page-note">住在桌面上的 AI 小伙伴。</p></div><button className="ym-button" disabled={busy||updating} onClick={()=>{void act(()=>appApi.exit());}}>退出应用</button></div>
      {info?<dl className="ym-app-info-details"><dt>版本</dt><dd>{info.version}</dd><dt>构建编号</dt><dd>{info.buildId==="00000000-000000"?"开发构建":info.buildId}</dd><dt>运行模式</dt><dd>{info.mode==="installed"?"安装版":"便携版"} · Windows x64</dd><dt>资料目录</dt><dd><span>{info.dataDirectory}</span><button className="ym-button" disabled={busy} onClick={()=>{void act(()=>appApi.openInformation("data"));}}>打开资料目录</button></dd></dl>:!error&&<p role="status">正在读取版本…</p>}
      <nav className="ym-app-info-links" aria-label="颜喵项目链接">{link("repository","主项目仓库","https://github.com/xianyvqiu/yanmew")}{link("community","社区小游戏","https://github.com/xianyvqiu/yanmew-community-games")}{link("issues","问题反馈","https://github.com/xianyvqiu/yanmew/issues")}{link("releases","正式发布","https://github.com/xianyvqiu/yanmew/releases")}</nav>
      {error&&<p role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={()=>setAttempt(value=>value+1)}>重新读取信息</button></p>}
    </section>
    <section className="ym-page-card" aria-label="程序更新"><div className="ym-page-heading"><h2>程序更新</h2><button className="ym-button" disabled={blocked} onClick={()=>{void act(async()=>{const value=await appApi.checkUpdate();if(alive.current)setStatus(value);});}}>{status?.phase==="checking"?"正在检查…":"检查更新"}</button></div>
      <p className="ym-page-note">每次启动颜喵自动检查一次；只有点击更新才会下载。更新会备份并保留资料，完成后自动重新启动。</p>
      <p role="status" className={status?.phase==="error"?"ym-page-error":"ym-page-note"}>{status?.message??"正在读取更新状态…"}</p>
      {status?.checkedAt&&<p className="ym-page-note">上次检查：{new Date(status.checkedAt).toLocaleString()}</p>}
      {updating&&status.total>0&&<div><progress aria-label="更新下载进度" value={status.downloaded} max={status.total}/><p className="ym-page-note">{(status.downloaded/1048576).toFixed(1)} / {(status.total/1048576).toFixed(1)} MB</p></div>}
      {status?.candidate&&<div className="ym-app-update-release"><h3>新版本 {status.candidate.manifest.version} · {status.candidate.manifest.buildId}</h3>{link("updateRelease","查看发布说明与下载",status.candidate.releaseUrl)}<p className="ym-app-update-notes">{status.candidate.notes||"此版本没有附加更新说明。"}</p><p className="ym-page-note">点击后将下载并校验完整包，再正常退出、备份资料、更新和重启。请先完成正在编辑的内容。</p><button className="ym-button" data-variant="primary" disabled={blocked} onClick={()=>{void act(()=>appApi.update());}}>{updating?"正在更新…":"更新并重新启动"}</button></div>}
    </section>
    <section className="ym-page-card" aria-label="卸载颜喵"><h2>卸载颜喵</h2><p className="ym-page-note">卸载会永久删除当前颜喵的全部个人资料、密钥、图片、插件和存档。导出到其他位置的备份保留；便携目录中的其他文件保留。</p><button className="ym-button" data-variant="danger" disabled={blocked||!info} onClick={()=>{setActionError("");setConfirmUninstall(true);}}>卸载颜喵</button></section>
    {actionError&&!confirmUninstall&&<p role="alert" className="ym-page-error">{actionError}</p>}
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="app-uninstall-title" onCancel={event=>{if(busy)event.preventDefault();else setConfirmUninstall(false);}}><h2 id="app-uninstall-title">卸载并删除全部资料？</h2><p>人设、会话、聊天、记忆、喵币、密钥、图片、插件和游戏存档将永久删除，无法恢复。请先导出仍需保留的资料。</p>{confirmUninstall&&actionError&&<p role="alert" className="ym-page-error">{actionError}</p>}<div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={()=>setConfirmUninstall(false)}>取消</button><button className="ym-button" data-variant="danger" disabled={busy} onClick={()=>{void act(()=>appApi.uninstall());}}>确认卸载并删除资料</button></div></dialog>
  </div>;
}
