import {useEffect,useLayoutEffect,useRef,useState} from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {errorMessage,webSearchApi} from "../api";
import {communityGamesApi,communityRepository,type CommunityGame,type CommunityReadme} from "../communityGamesApi";
import {useCardBrowser} from "../hooks/useCardBrowser";
import CardBrowserControls,{CardBrowserPagination} from "./CardBrowserControls";
import "./GamePlugins.css";
import "./CommunityGamesPanel.css";

export function readmeUrl(value:string,base:string,image=false):string|undefined {
  try {
    const url=new URL(value,base);
    if(url.protocol!=="https:"||url.username||url.password||value.length>2048)return;
    if(image&&!(url.hostname==="raw.githubusercontent.com"&&url.pathname.startsWith("/xianyvqiu/yanmew-community-games/")))return;
    return url.href;
  }catch{return;}
}
export default function CommunityGamesPanel({onBack}:{onBack:()=>void}) {
  const [games,setGames]=useState<CommunityGame[]>([]),[loading,setLoading]=useState(true),[error,setError]=useState(""),[notice,setNotice]=useState("");
  const [selected,setSelected]=useState<CommunityGame|null>(null),[readme,setReadme]=useState<CommunityReadme|null>(null),[readmeError,setReadmeError]=useState("");
  const [downloading,setDownloading]=useState("");
  const mounted=useRef(false),loadToken=useRef(0),downloadLock=useRef(false),dialog=useRef<HTMLDialogElement>(null),root=useRef<HTMLElement>(null);
  const browser=useCardBrowser(games,g=>`${g.name} ${g.description} ${g.author} ${g.id}`,g=>g.publishedAt,g=>g.id);
  const load=async()=>{
    const token=++loadToken.current;setLoading(true);setError("");
    try{const rows=await communityGamesApi.list();if(mounted.current&&token===loadToken.current)setGames(rows);}
    catch(cause){if(mounted.current&&token===loadToken.current)setError(errorMessage(cause));}
    finally{if(mounted.current&&token===loadToken.current)setLoading(false);}
  };
  useLayoutEffect(()=>{const content=root.current?.closest(".ym-window-shell__content");if(content)content.scrollTop=0;},[]);
  useEffect(()=>{mounted.current=true;void load();return()=>{mounted.current=false;loadToken.current++;};},[]);
  useEffect(()=>{
    let disposed=false;setReadme(null);setReadmeError("");
    if(selected){dialog.current?.showModal();void communityGamesApi.readme(selected.id,selected.version).then(value=>{if(!disposed)setReadme(value);}).catch(cause=>{if(!disposed)setReadmeError(errorMessage(cause));});}
    else dialog.current?.close();
    return()=>{disposed=true;};
  },[selected]);
  const open=async(url:string,detail=false)=>{try{await webSearchApi.open(url);}catch(cause){if(mounted.current)(detail?setReadmeError:setError)(errorMessage(cause));}};
  const download=async(game:CommunityGame)=>{
    if(downloadLock.current)return;downloadLock.current=true;setDownloading(game.id);setError("");setNotice("");
    try{const url=await communityGamesApi.download(game.id,game.version);if(!mounted.current)return;await webSearchApi.open(url);if(mounted.current)setNotice("已打开下载链接。下载完成后，返回小游戏插件页从本地安装。");}
    catch(cause){if(mounted.current)setError(errorMessage(cause));}
    finally{downloadLock.current=false;if(mounted.current)setDownloading("");}
  };
  return <section ref={root} className="ym-page-card ym-community-games" aria-label="社区小游戏">
    <div className="ym-page-heading"><h2>社区小游戏</h2><div className="ym-game-tools"><button className="ym-button" onClick={onBack}>返回小游戏插件</button><button className="ym-button" disabled={loading} onClick={()=>void load()}>刷新列表</button></div></div>
    <p className="ym-page-note">浏览社区作品，点击卡片查看介绍。下载完成后从本地安装，权限由你确认。</p>
    {!selected&&error&&<p className="ym-page-error" role="alert">{error}</p>}{!selected&&notice&&<p className="ym-page-note" role="status">{notice}</p>}
    {loading&&<p role="status">正在读取社区小游戏…</p>}
    {!loading&&!error&&!games.length&&<p className="ym-page-empty">社区暂时还没有已发布的小游戏。</p>}
    <button className="ym-button" onClick={()=>void open(communityRepository)}>查看社区仓库</button>
    {!!games.length&&<><CardBrowserControls browser={browser} label="社区小游戏" timeLabel="发布时间"/>
      <div className="ym-game-grid">{browser.items.map(game=><article className="ym-game-card ym-community-card" key={game.id}>
        <button className="ym-community-card-open" aria-label={`查看 ${game.name} 详情`} onClick={()=>{setError("");setNotice("");setSelected(game);}}><strong>{game.name}</strong><span>{game.description}</span><small>{game.author} · {game.version}</small></button>
        <button className="ym-button" disabled={!!downloading} onClick={()=>void download(game)} aria-label={`下载 ${game.name}`}>{downloading===game.id?"正在打开下载…":"下载"}</button>
      </article>)}</div><CardBrowserPagination browser={browser} label="社区小游戏"/></>}
    <dialog ref={dialog} className="ym-session-dialog ym-community-dialog" aria-labelledby="community-game-title" onCancel={event=>{event.preventDefault();setSelected(null);}}>
      <h2 id="community-game-title">{selected?.name??"小游戏详情"}</h2>
      <div className="ym-session-dialog-body"><p className="ym-page-note">{selected?.author} · {selected?.version}</p>
        {selected&&error&&<p className="ym-page-error" role="alert">{error}</p>}{selected&&notice&&<p className="ym-page-note" role="status">{notice}</p>}
        {!readme&&!readmeError&&<p role="status">正在读取游戏介绍…</p>}{readmeError&&<p className="ym-page-error" role="alert">{readmeError}</p>}
        {readme&&<div className="ym-community-readme"><Markdown remarkPlugins={[remarkGfm]} skipHtml urlTransform={(value,key)=>readmeUrl(value,readme.baseUrl,key==="src")??""} components={{
          a:({href,children})=>href?<a href={href} onClick={event=>{event.preventDefault();void open(href,true);}}>{children}</a>:<span>{children}</span>,
          img:({src,alt})=>src?<img src={src} alt={alt??""} loading="lazy" referrerPolicy="no-referrer" crossOrigin="anonymous"/>:<span>{alt??"图片"}</span>,
        }}>{readme.markdown}</Markdown></div>}
      </div>
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={!selected||!!downloading} onClick={()=>{if(selected)void download(selected);}}>{downloading?"正在打开下载…":"下载"}</button><button className="ym-button" onClick={()=>setSelected(null)}>关闭</button></div>
    </dialog>
  </section>;
}
