import {useEffect,useRef,useState} from "react";
import {configurationApi,errorMessage} from "../../api";
import type {Persona} from "../../types";
import {stickerApi,type ChatSticker,type StickerLibrary,type StickerTag} from "../../stickerApi";
import {useCardBrowser} from "../../hooks/useCardBrowser";
import CardBrowserControls,{CardBrowserPagination} from "../CardBrowserControls";
import {StickerDialog,StickerImage,StickerPager,StickerSearch,TagPicker,useStickerPage} from "./StickerLibraryShared";
import "./StickerLibraryPanel.css";
type Editing={kind:"libraries";item:StickerLibrary|null}|{kind:"tags";item:StickerTag|null}|{kind:"stickers";item:ChatSticker};

function LibraryPeople({library,disabled,onBind}:{library:StickerLibrary;disabled:boolean;onBind:(personaId:string,bound:boolean)=>Promise<boolean>}){
  const [people,setPeople]=useState<Persona[]>([]),[bound,setBound]=useState(library.personaIds),[error,setError]=useState("");
  useEffect(()=>{let alive=true;void configurationApi.listPersonas().then(value=>{if(alive)setPeople(value);}).catch(e=>{if(alive)setError(errorMessage(e));});return()=>{alive=false;};},[]);
  const browser=useCardBrowser(people,p=>p.name,p=>p.updatedAt??"",p=>p.id,6);
  return <section className="ym-sticker-people"><h3>绑定人设</h3><p className="ym-page-note">当前绑定：{bound.length?bound.map(id=>people.find(p=>p.id===id)?.name??`人设 ID ${id}`).join("、"):"未绑定人设"}</p><p className="ym-page-note">可供多位伙伴共用；绑定和解除立即生效。</p><CardBrowserControls browser={browser} label="人设" disabled={disabled}/>
    {error&&<p role="alert" className="ym-page-error">{error}</p>}{browser.items.map(persona=><div className="ym-sticker-binding-row" key={persona.id}><strong>{persona.name}</strong><button type="button" className="ym-button" disabled={disabled} aria-pressed={bound.includes(persona.id)} onClick={()=>{const next=!bound.includes(persona.id);void onBind(persona.id,next).then(ok=>{if(ok)setBound(current=>next?[...current,persona.id]:current.filter(id=>id!==persona.id));});}}>{bound.includes(persona.id)?"已绑定 · 解除":"绑定"}</button></div>)}
    <CardBrowserPagination browser={browser} label="人设" disabled={disabled}/></section>;
}
function Editor({editing,onClose,onSaved}:{editing:Editing;onClose:()=>void;onSaved:()=>void}){
  const item=editing.item;
  const [name,setName]=useState(item?("title" in item?item.title:item.name):""),[description,setDescription]=useState(item&&"description" in item?item.description:"");
  const [tags,setTags]=useState<Pick<StickerTag,"id"|"name">[]>(editing.kind==="stickers"?editing.item.tags:[]);
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[deleting,setDeleting]=useState(false);const lock=useRef(false),alive=useRef(true);
  const [closing,setClosing]=useState(false),closeTimer=useRef<number>();
  const close=()=>{if(closeTimer.current!==undefined)return;setClosing(true);closeTimer.current=window.setTimeout(onClose,window.matchMedia?.("(prefers-reduced-motion: reduce)").matches?0:160);};
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;window.clearTimeout(closeTimer.current);};},[]);
  const run=async(input:Record<string,unknown>,shouldClose=true)=>{if(lock.current||closing)return false;lock.current=true;setBusy(true);setError("");try{await stickerApi.mutate(input);if(alive.current){onSaved();if(shouldClose)close();}return true;}catch(e){if(alive.current)setError(errorMessage(e));return false;}finally{lock.current=false;if(alive.current)setBusy(false);}};
  const save=()=>void run(editing.kind==="libraries"?{op:"saveLibrary",id:item?.id??null,revision:item?.revision??null,title:name,description}:editing.kind==="tags"?{op:"saveTag",id:item?.id??null,revision:item?.revision??null,name}:{op:"saveSticker",id:item!.id,revision:item!.revision,title:name,description,tagIds:tags.map(t=>t.id)});
  const label=editing.kind==="libraries"?"表情包库":editing.kind==="tags"?"标签":"表情包";
  return <StickerDialog title={`${item?"编辑":"新建"}${label}`} busy={busy||closing} closing={closing} onClose={close} actions={deleting?<><button type="button" className="ym-button" disabled={busy||closing} onClick={()=>setDeleting(false)}>取消删除</button><button type="button" className="ym-button" data-variant="danger" disabled={busy||closing} onClick={()=>void run({op:"delete",entity:editing.kind,id:item!.id,revision:item!.revision})}>{busy?"正在删除…":"确认删除"}</button></>:<><button type="button" className="ym-button" data-variant="primary" disabled={busy||closing||!name.trim()} onClick={save}>{busy?"正在保存…":"保存"}</button></>}>
    {editing.kind==="stickers"&&<StickerImage id={editing.item.id} title={editing.item.title}/>}
    {error&&<p role="alert" className="ym-page-error">{error}</p>}
    {deleting?<div className="ym-page-error"><p>{editing.kind==="libraries"?"删除这个库及其中的表情包，并解除人设绑定？":editing.kind==="tags"?"删除这个标签，并从所有表情包中移除它？":"删除这个表情包？"}</p><p>已经发送的聊天图片会保留。</p></div>:<div className="ym-sticker-editor-fields">
      <label className="ym-field">{label}{editing.kind==="tags"?"名称":"标题"}<input value={name} disabled={busy} onChange={e=>setName(e.target.value)} placeholder={editing.kind==="libraries"?"例如：猫猫日常":"输入名称"}/></label>
      {editing.kind!=="tags"&&<label className="ym-field">{editing.kind==="libraries"?"库简介":"表情包介绍"}<textarea rows={3} value={description} disabled={busy} onChange={e=>setDescription(e.target.value)} placeholder={editing.kind==="libraries"?"简单介绍这个库的内容和风格":"描述画面、情绪，以及适合什么时候发送，帮助 AI 选择"}/></label>}
      {editing.kind==="stickers"&&<section><h3>标签</h3><p className="ym-page-note">可选择多个已有标签，也可新建标签。</p><TagPicker selected={tags} onChange={setTags} disabled={busy}/></section>}
      {editing.kind==="libraries"&&editing.item&&<LibraryPeople library={editing.item} disabled={busy} onBind={(personaId,bound)=>run({op:"bind",libraryId:item!.id,personaId,bound},false)}/>}
      {item&&<div className="ym-sticker-danger-zone"><button type="button" className="ym-button" data-variant="danger" disabled={busy} onClick={()=>{setError("");setDeleting(true);}}>删除{label}</button></div>}
    </div>}
  </StickerDialog>;
}
async function readFile(file:File){return new Promise<string>((resolve,reject)=>{const reader=new FileReader();reader.onload=()=>resolve(String(reader.result));reader.onerror=()=>reject(new Error("文件读取失败"));reader.onabort=()=>reject(new Error("文件读取已取消"));reader.readAsDataURL(file);});}
export default function StickerLibraryPanel({requestedId}:{requestedId?:string}={}){
  const [people,setPeople]=useState<Persona[]>([]),[peopleError,setPeopleError]=useState(""),[requestError,setRequestError]=useState("");
  useEffect(()=>{let alive=true;const read=()=>{void configurationApi.listPersonas().then(value=>{if(alive){setPeople(value);setPeopleError("");}}).catch(e=>{if(alive)setPeopleError(errorMessage(e));});};read();window.addEventListener("focus",read);return()=>{alive=false;window.removeEventListener("focus",read);};},[]);
  const boundNames=(item:StickerLibrary)=>item.personaIds.length?item.personaIds.map(id=>people.find(p=>p.id===id)?.name??`人设 ID ${id}`).join("、"):"未绑定人设";
  const [view,setView]=useState<"libraries"|"tags">("libraries"),[library,setLibrary]=useState<StickerLibrary|null>(null),[editing,setEditing]=useState<Editing|null>(null);
  const [filters,setFilters]=useState<Pick<StickerTag,"id"|"name">[]>([]),[filterOpen,setFilterOpen]=useState(false),[importing,setImporting]=useState(false),[progress,setProgress]=useState(""),[failures,setFailures]=useState<string[]>([]);
  const input=useRef<HTMLInputElement>(null),alive=useRef(true),lock=useRef(false);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  const libraries=useStickerPage<StickerLibrary>({entity:"libraries"},view==="libraries"&&!library);
  const tags=useStickerPage<StickerTag>({entity:"tags"},view==="tags");
  const stickers=useStickerPage<ChatSticker>({entity:"stickers",libraryId:library?.id,tagIds:filters.map(t=>t.id)},!!library);
  const detailQuery={entity:"libraries" as const,libraryId:library?.id};
  const detail=useStickerPage<StickerLibrary>(detailQuery,!!library);
  useEffect(()=>{if(!library||detail.loading||detail.error||detail.loadedQuery!==JSON.stringify(detailQuery))return;const saved=detail.items[0];if(!saved)setLibrary(null);else if(JSON.stringify(saved)!==JSON.stringify(library))setLibrary(saved);},[library,detail.items,detail.loading,detail.error,detail.loadedQuery]);
  const reload=()=>{libraries.reload();tags.reload();stickers.reload();detail.reload();};
  const openLibrary=(value:StickerLibrary)=>{setLibrary(value);setFilters([]);setFilterOpen(false);setProgress("");setFailures([]);stickers.setSearch("");stickers.setPage(1);};
  useEffect(()=>{if(!requestedId)return;let alive=true;setRequestError("");void stickerApi.query<StickerLibrary>({entity:"libraries",libraryId:requestedId}).then(value=>{if(!alive)return;const target=value.items.find(item=>item.id===requestedId);if(target)openLibrary(target);else setRequestError("该表情包库已不存在，请从列表重新选择。");}).catch(e=>{if(alive)setRequestError(errorMessage(e));});return()=>{alive=false;};},[requestedId]);
  const importFiles=async(files:File[])=>{
    if(lock.current||!library||!files.length)return;lock.current=true;setImporting(true);setFailures([]);const errors:string[]=[];let success=0;
    try{for(let i=0;i<files.length&&alive.current;i++){
      const file=files[i];setProgress(`正在导入 ${i+1} / ${files.length} · ${file.name}`);
      try{if(file.size>16*1024*1024)throw new Error("单张图片不能超过16 MB");if(!/\.(png|jpe?g|webp|gif)$/i.test(file.name))throw new Error("请选择 PNG、JPEG、WebP 或 GIF");const dataUrl=await readFile(file);if(!alive.current)break;await stickerApi.import(library.id,{filename:file.name,dataUrl});success++;}catch(e){errors.push(`${file.name}：${errorMessage(e)}`);}
    }}finally{lock.current=false;if(alive.current){setImporting(false);setProgress(`已导入 ${success} 个表情包${errors.length?`，${errors.length} 个未能导入`:""}`);setFailures(errors);reload();}}
  };
  const current=library?stickers:view==="tags"?tags:libraries;
  return <section className="ym-stickers">
    {(peopleError||requestError)&&<p role="alert" className="ym-page-error">{requestError||`读取绑定人设失败：${peopleError}`}</p>}
    <header className="ym-sticker-heading"><div>{library&&<button type="button" className="ym-button ym-sticker-back" disabled={importing} onClick={()=>{setLibrary(null);setFilters([]);setProgress("");setFailures([]);}}>‹ 全部表情包库</button>}<h2>{library?library.title:"聊天表情包库"}</h2><p className="ym-page-note">{library?library.description||"收藏喜欢的表情，给伙伴更多表达方式。":"建立自己的表情包收藏，再绑定给喜欢的伙伴。"}</p></div>
      <div className="ym-inline-form">{library?<><button type="button" className="ym-button" disabled={importing} onClick={()=>setEditing({kind:"libraries",item:library})}>库设置与绑定</button><button type="button" className="ym-button" data-variant="primary" disabled={importing} onClick={()=>input.current?.click()}>导入表情包</button><input ref={input} hidden type="file" accept="image/png,image/jpeg,image/webp,image/gif" multiple disabled={importing} onChange={e=>{const files=Array.from(e.target.files??[]);e.target.value="";void importFiles(files);}}/></>:<button type="button" className="ym-button" data-variant="primary" onClick={()=>setEditing(view==="tags"?{kind:"tags",item:null}:{kind:"libraries",item:null})}>{view==="tags"?"新建标签":"新建表情包库"}</button>}</div>
    </header>
    {library&&<p className="ym-page-note">当前绑定：{boundNames(library)}</p>}
    {!library&&<div className="ym-sticker-tabs" role="group" aria-label="表情包管理分类"><button type="button" className="ym-button" aria-pressed={view==="libraries"} onClick={()=>setView("libraries")}>表情包库</button><button type="button" className="ym-button" aria-pressed={view==="tags"} onClick={()=>setView("tags")}>标签管理</button></div>}
    <div className="ym-sticker-toolbar"><StickerSearch browser={current} label={library?"表情包":view==="tags"?"标签":"表情包库"} disabled={importing}/>{library&&<button type="button" className="ym-button" disabled={importing} aria-expanded={filterOpen} onClick={()=>setFilterOpen(v=>!v)}>按标签筛选{filters.length?` · ${filters.length}`:""}<span aria-hidden="true"> {filterOpen?"⌃":"⌄"}</span></button>}</div>
    {library&&filterOpen&&<div className="ym-page-card ym-sticker-filter"><h3>同时符合所选标签</h3><TagPicker selected={filters} onChange={value=>{setFilters(value);stickers.setPage(1);}} disabled={importing} libraryId={library.id} create={false}/>{!!filters.length&&<button type="button" className="ym-button" disabled={importing} onClick={()=>setFilters([])}>清除筛选</button>}</div>}
    {library&&!filterOpen&&!!filters.length&&<p className="ym-page-note">同时符合：{filters.map(t=>t.name).join("、")}</p>}
    {progress&&<div className="ym-sticker-import" role="status" aria-live="polite">{importing&&<span className="ym-sticker-spinner" aria-hidden="true"/>}{progress}</div>}
    {!!failures.length&&<div className="ym-page-error" role="alert">{failures.map((text,index)=><p key={index}>{text}</p>)}</div>}
    {current.error&&<p role="alert" className="ym-page-error">{current.error}<button type="button" className="ym-button" onClick={current.reload}>重新加载</button></p>}
    {current.loading&&<p className="ym-page-note" role="status">正在加载…</p>}
    <div className={`ym-sticker-grid${library?" ym-sticker-grid--images":""}`} aria-busy={current.loading||importing}>
      {library?stickers.items.map(item=><button type="button" className="ym-page-card ym-sticker-card ym-sticker-card--image" key={item.id} disabled={importing||current.loading} onClick={()=>setEditing({kind:"stickers",item})}><StickerImage id={item.id} title={item.title}/><div className="ym-sticker-card-caption"><strong>{item.title}</strong>{item.mediaType==="image/gif"&&<span className="ym-sticker-format">GIF</span>}</div><p className="ym-card-summary">{item.description||"点击补充介绍与标签"}</p><div className="ym-sticker-tags">{item.tags.slice(0,3).map(t=><span className="ym-sticker-tag-label" key={t.id}>{t.name}</span>)}{item.tags.length>3&&<span className="ym-sticker-tag-label">+{item.tags.length-3}</span>}</div></button>):view==="tags"?tags.items.map(tag=><button type="button" className="ym-page-card ym-sticker-card" key={tag.id} disabled={current.loading} onClick={()=>setEditing({kind:"tags",item:tag})}><span className="ym-sticker-library-icon" aria-hidden="true">#</span><strong>{tag.name}</strong><span className="ym-page-note">编辑标签</span></button>):libraries.items.map(item=><article className="ym-page-card ym-sticker-card" key={item.id}><button type="button" className="ym-sticker-library-open" disabled={current.loading} onClick={()=>openLibrary(item)}><span className="ym-sticker-library-icon" aria-hidden="true">☺</span><h3>{item.title}</h3><p className="ym-card-summary">{item.description||"还没有填写简介"}</p><span className="ym-page-note">{item.count} 个表情包 · {item.personaIds.length} 位伙伴使用</span></button><p className="ym-page-note">当前绑定：{boundNames(item)}</p><button type="button" className="ym-button" disabled={current.loading} onClick={()=>setEditing({kind:"libraries",item})}>设置与绑定</button></article>)}
    </div>
    {!current.loading&&!current.error&&!current.total&&<div className="ym-sticker-empty"><span aria-hidden="true">( ・ω・ )</span><h3>{current.search||filters.length?"没有找到匹配的内容":library?"这个库还空着":"从喜欢的表情开始"}</h3><p>{current.search||filters.length?"试试其他关键词，或减少筛选标签。":library?"点击“导入表情包”，可一次选择多张图片或 GIF。":view==="tags"?"创建标签，让伙伴更容易找到合适的表情。":"新建一个表情包库，收藏图片和 GIF，再绑定给伙伴。"}</p></div>}
    <StickerPager browser={current} label={library?"表情包":view==="tags"?"标签":"表情包库"} disabled={importing}/>
    {editing&&<Editor key={`${editing.kind}:${editing.item?.id??"new"}`} editing={editing} onClose={()=>setEditing(null)} onSaved={reload}/>}
  </section>;
}
