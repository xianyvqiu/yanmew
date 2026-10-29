import {useCallback,useEffect,useRef,useState,type ReactNode} from "react";
import {createPortal} from "react-dom";
import {errorMessage} from "../../api";
import {stickerApi,type StickerPage,type StickerQuery,type StickerTag} from "../../stickerApi";
import {CardBrowserPagination} from "../CardBrowserControls";

export function useStickerPage<T>(query:StickerQuery,active=true){
  const [search,setSearch]=useState(""),[page,setPage]=useState(1),[refresh,setRefresh]=useState(0);
  const [data,setData]=useState<StickerPage<T>>({items:[],page:1,pages:1,total:0}),[loading,setLoading]=useState(true),[error,setError]=useState("");
  const key=JSON.stringify(query),seq=useRef(0);
  const [loadedQuery,setLoadedQuery]=useState("");
  useEffect(()=>{setPage(1);},[key,search]);
  useEffect(()=>{
    const token=++seq.current;if(!active){setLoading(false);return;}
    setLoading(true);setError("");
    void stickerApi.query<T>({...JSON.parse(key),query:search,page}).then(value=>{if(token===seq.current){setData(value);setLoadedQuery(key);}}).catch(e=>{if(token===seq.current){setData({items:[],page:1,pages:1,total:0});setError(errorMessage(e));}}).finally(()=>{if(token===seq.current)setLoading(false);});
    return()=>{seq.current++;};
  },[key,search,page,refresh,active]);
  useEffect(()=>{
    let disposed=false,stop:(()=>void)|undefined;
    const refresh=()=>{if(!disposed)setRefresh(n=>n+1);};
    void stickerApi.listen(refresh).then(fn=>{if(disposed)fn();else{stop=fn;refresh();}}).catch(e=>{if(!disposed)setError(errorMessage(e));});
    window.addEventListener("focus",refresh);return()=>{disposed=true;stop?.();window.removeEventListener("focus",refresh);};
  },[]);
  return {...data,loadedQuery,search,setSearch,loading,error,setPage,reload:useCallback(()=>setRefresh(n=>n+1),[])};
}
export function StickerPager({browser,label,disabled=false}:{browser:ReturnType<typeof useStickerPage>;label:string;disabled?:boolean}){
  return <CardBrowserPagination label={label} disabled={disabled||browser.loading} browser={{...browser,order:"newest",setOrder:()=>{}}}/>;
}
export function StickerSearch({browser,label,disabled=false}:{browser:ReturnType<typeof useStickerPage>;label:string;disabled?:boolean}){
  return <label className="ym-field ym-sticker-search">搜索{label}<input type="search" value={browser.search} disabled={disabled} placeholder={label==="标签"?"输入标签名称":label==="表情包库"?"输入标题或简介":"输入标题、介绍或标签"} onChange={e=>browser.setSearch(e.target.value)}/></label>;
}
export function StickerDialog({title,children,actions,onClose,busy=false,closing=false}:{title:string;children:ReactNode;actions?:ReactNode;onClose:()=>void;busy?:boolean;closing?:boolean}){
  const dialog=useRef<HTMLDialogElement>(null);
  useEffect(()=>{const el=dialog.current;if(el?.showModal)el.showModal();else el?.setAttribute("open","");},[]);
  return createPortal(<dialog ref={dialog} className="ym-session-dialog ym-sticker-dialog" data-closing={closing} aria-label={title} onCancel={e=>{e.preventDefault();if(!busy&&!closing)onClose();}}>
    <h2>{title}</h2><div className="ym-session-dialog-body">{children}</div>
    <div className="ym-session-dialog-actions">{actions}<button type="button" className="ym-button" disabled={busy||closing} onClick={onClose}>关闭</button></div>
  </dialog>,document.body);
}
export function StickerImage({id,title}:{id:string;title:string}){
  const [url,setUrl]=useState(""),[error,setError]=useState("");
  useEffect(()=>{let alive=true;setUrl("");setError("");void stickerApi.image(id).then(v=>{if(alive)setUrl(v);}).catch(e=>{if(alive)setError(errorMessage(e));});return()=>{alive=false;};},[id]);
  return <div className="ym-sticker-image">{url?<img src={url} alt={title} loading="lazy"/>:<span role="status">{error||"正在读取图片…"}</span>}</div>;
}
export function TagPicker({selected,onChange,disabled=false,libraryId,create=true}:{selected:Pick<StickerTag,"id"|"name">[];onChange:(value:Pick<StickerTag,"id"|"name">[])=>void;disabled?:boolean;libraryId?:string;create?:boolean}){
  const browser=useStickerPage<StickerTag>({entity:"tags",libraryId}),[name,setName]=useState(""),[saving,setSaving]=useState(false),[error,setError]=useState("");
  const lock=useRef(false);
  const add=async()=>{if(lock.current||!name.trim())return;lock.current=true;setSaving(true);setError("");try{const result=await stickerApi.mutate({op:"saveTag",id:null,revision:null,name:name.trim()});if(result.id&&!selected.some(t=>t.id===result.id))onChange([...selected,{id:result.id,name:name.trim()}]);setName("");browser.reload();}catch(e){setError(errorMessage(e));}finally{lock.current=false;setSaving(false);}};
  return <div className="ym-sticker-tag-picker">
    {selected.length>0&&<div className="ym-sticker-tags" aria-label="已选标签">{selected.map(t=><button type="button" className="ym-button ym-sticker-tag" key={t.id} disabled={disabled||saving} aria-label={`移除标签 ${t.name}`} onClick={()=>onChange(selected.filter(s=>s.id!==t.id))}>{t.name}<span aria-hidden="true"> ×</span></button>)}</div>}
    <StickerSearch browser={browser} label="标签" disabled={disabled||saving}/>
    <div className="ym-sticker-tags">{browser.items.map(t=><button type="button" className="ym-button ym-sticker-tag" key={t.id} disabled={disabled||saving||browser.loading} aria-pressed={selected.some(s=>s.id===t.id)} onClick={()=>onChange(selected.some(s=>s.id===t.id)?selected.filter(s=>s.id!==t.id):[...selected,t])}>{t.name}</button>)}</div>
    {!browser.loading&&!browser.items.length&&<p className="ym-page-note">暂无匹配标签。</p>}
    <StickerPager browser={browser} label="标签" disabled={disabled||saving}/>
    {create&&<div className="ym-sticker-add-tag"><label className="ym-field">新建标签<input value={name} disabled={disabled||saving} onChange={e=>setName(e.target.value)} placeholder="例如：开心、猫猫"/></label><button type="button" className="ym-button" disabled={disabled||saving||!name.trim()} onClick={()=>void add()}>{saving?"正在添加…":"新建并选中"}</button></div>}
    {(error||browser.error)&&<p role="alert" className="ym-page-error">{error||browser.error}</p>}
  </div>;
}
