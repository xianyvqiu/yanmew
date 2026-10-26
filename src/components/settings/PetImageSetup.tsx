import {useEffect,useRef,useState} from "react";
import {errorMessage,imageGenerationApi,petCreationApi} from "../../api";
import type {PetArtwork,PetCreation} from "../../types";
import {PetArtworkImage} from "./PetCreationPanel";
import {basicPetImages} from "./petImageSetupTemplates";
import "./PetImageSetup.css";

function MethodDialog({busy,onSelect,onClose}:{busy:boolean;onSelect:(method:"manual"|"generate")=>void;onClose:()=>void}){
 const ref=useRef<HTMLDialogElement>(null);
 useEffect(()=>{if(ref.current?.showModal)ref.current.showModal();else ref.current?.setAttribute("open","");},[]);
 return <dialog ref={ref} className="ym-session-dialog ym-pet-method" aria-label="选择表情准备方式" onCancel={e=>{e.preventDefault();if(!busy)onClose();}}><h2>接下来，准备基础表情</h2><p className="ym-page-note">初始图片已保存，选择适合你的方式。</p><div className="ym-pet-method-options"><button className="ym-button" disabled={busy} onClick={()=>onSelect("generate")}><strong>一次性生成所有基础表情</strong><span>以初始图为参考生成预览，检查后接受；使用生图服务，可能消耗额度。</span></button><button className="ym-button" disabled={busy} onClick={()=>onSelect("manual")}><strong>自己上传表情</strong><span>建立全部基础词条，按需上传透明图片，不调用生图服务。</span></button></div><button className="ym-button" disabled={busy} onClick={onClose}>稍后选择</button></dialog>;
}
export default function PetImageSetup({personaId,personaName,onClose}:{personaId:string;personaName:string;onClose:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null),alive=useRef(true),locked=useRef(false),stopped=useRef(false),current=useRef<string|null>(null),reader=useRef<FileReader|null>(null);
 const [method,setMethod]=useState<"choose"|"manual"|"generate"|null>(null);
 const [library,setLibrary]=useState<PetCreation|null>(null),[working,setWorking]=useState(false),[progress,setProgress]=useState(""),[error,setError]=useState("");
 const reload=async()=>{const value=await petCreationApi.get(personaId);if(alive.current)setLibrary(value);return value;};
 useEffect(()=>{alive.current=true;if(dialog.current?.showModal)dialog.current.showModal();else dialog.current?.setAttribute("open","");void reload().then(value=>{if(alive.current&&value.items.some(a=>a.kind==="base"&&a.imageFile))setMethod("choose");}).catch(e=>{if(alive.current)setError(errorMessage(e));});return()=>{alive.current=false;stopped.current=true;reader.current?.abort();if(current.current)void petCreationApi.cancel(personaId,current.current).catch(()=>{});};},[personaId]);
 const guard=()=>{if(!alive.current||stopped.current)throw new Error("已停止操作，已保存的图片和预览保留。");};
 const run=async(job:()=>Promise<void>)=>{if(locked.current)return;locked.current=true;stopped.current=false;setWorking(true);setError("");try{await job();}catch(e){if(alive.current)setError(errorMessage(e));}finally{if(alive.current){await reload().catch(e=>setError(errorMessage(e)));if(alive.current)setWorking(false);}current.current=null;locked.current=false;}};
 const placeholders=async()=>{
  for(const input of basicPetImages){guard();const value=await reload();guard();if(!value.items.some(a=>a.kind==="expression"&&a.category===input.category&&a.name===input.name)){await petCreationApi.save(personaId,null,null,input);guard();}}
 };
 const readFile=async(file:File)=>{
  if(file.size>16*1024*1024)throw new Error("图片不能超过16 MB，请缩小后上传。");
  return new Promise<string>((resolve,reject)=>{const r=new FileReader();reader.current=r;r.onload=()=>resolve(String(r.result));r.onerror=()=>reject(new Error("图片读取失败"));r.onabort=()=>reject(new Error("读取已取消"));r.readAsDataURL(file);});
 };
 const generate=async()=>{
  const config=await imageGenerationApi.get();guard();if(!config.enabled||!config.activeId)throw new Error("初始图已保留。请在提供商中开启并配置支持参考图编辑的生图服务，再生成基础表情。");
  await placeholders();guard();
  for(let i=0;i<basicPetImages.length;i++){
   guard();const input=basicPetImages[i];setProgress(`${i+1} / ${basicPetImages.length} · ${input.name}`);
   const value=await reload();guard();let item=value.items.find(a=>a.kind==="expression"&&a.category===input.category&&a.name===input.name);
   if(item?.imageFile||item?.previewFile)continue;
   if(!item)item=await petCreationApi.save(personaId,null,null,input);guard();current.current=item.id;
   await petCreationApi.generate(personaId,item.id,item.revision);guard();await reload();
  }
  setProgress("基础表情预览已准备好，请检查后接受；不满意的可单独重试。");
 };
 const upload=async(file:File)=>run(async()=>{
  setProgress("正在保存初始参考图…");const data=await readFile(file);guard();
  const value=await reload();guard();let base=value.items.find(a=>a.kind==="base");
  if(!base)base=await petCreationApi.save(personaId,null,null,{kind:"base",name:"初始形象",prompt:"",description:"用户上传的初始参考形象",category:"normal",enabled:true});guard();
  const uploaded=await petCreationApi.upload(personaId,base.id,base.revision,data);guard();await petCreationApi.finish(personaId,base.id,uploaded.revision,"accept");guard();await reload();setProgress("初始图已保存，请选择基础表情的准备方式。");setMethod("choose");
 });
 const selectMethod=(choice:"manual"|"generate")=>{setMethod(choice);void run(choice==="manual"?async()=>{setProgress("正在建立基础表情词条…");await placeholders();setProgress("词条已准备好，请逐项上传透明图片，检查后接受。启用图片模式至少需要一张待机图。");}:generate);};
 const uploadExpression=(item:PetArtwork,file:File)=>run(async()=>{setProgress(`正在上传 · ${item.name}`);const data=await readFile(file);guard();await petCreationApi.upload(personaId,item.id,item.revision,data);guard();setProgress(`${item.name}已上传，请检查预览后接受。`);});
 const accept=(item:PetArtwork)=>run(async()=>{await petCreationApi.finish(personaId,item.id,item.revision,"accept");});
 const retry=(item:PetArtwork)=>run(async()=>{setProgress(`正在重新生成 · ${item.name}`);current.current=item.id;await petCreationApi.generate(personaId,item.id,item.revision);});
 const enable=()=>run(async()=>{let value=await reload();for(const item of value.items.filter(a=>a.previewFile)){guard();await petCreationApi.finish(personaId,item.id,item.revision,"accept");}value=await reload();guard();await petCreationApi.enable(personaId,true,value.revision);onClose();});
 const base=library?.items.find(a=>a.kind==="base"),items=library?.items.filter(a=>a.kind==="expression")??[];
 const standby=items.some(a=>a.category==="standby"&&a.enabled&&(a.imageFile||a.previewFile));
 return <dialog ref={dialog} className="ym-session-dialog ym-pet-setup" aria-label="图片人设基础表情" onCancel={e=>{e.preventDefault();if(!working)onClose();}}>
  <header className="ym-page-heading"><div><h2>{personaName} · 图片形象</h2><p className="ym-page-note">上传初始图后，选择生成 {basicPetImages.length} 张基础表情，或自己逐项上传。生成可能消耗服务额度；上传分支不调用模型。</p></div></header>
  {working&&<div role="status" aria-live="polite" className="ym-pet-setup-progress"><span className="ym-setup-spinner" aria-hidden="true"/><div><strong>正在准备表情，请稍候</strong><p>{progress}</p></div><button className="ym-button" onClick={()=>{stopped.current=true;reader.current?.abort();if(current.current)void petCreationApi.cancel(personaId,current.current).catch(()=>{});}}>{method==="generate"?"停止生成":"停止准备"}</button></div>}
  {error&&<p className="ym-page-error" role="alert">{error}</p>}
  <fieldset disabled={working||!library} className="ym-pet-setup-fields"><legend>初始图与表情预览</legend>
   <label className="ym-field">上传初始图片<input type="file" accept="image/png,image/webp" onChange={event=>{const file=event.target.files?.[0];event.target.value="";if(file)void upload(file);}}/></label>
   {base&&(base.imageFile||base.previewFile)&&<div className="ym-pet-setup-base"><PetArtworkImage personaId={personaId} item={base} preview={!!base.previewFile}/>{base.previewFile&&<button className="ym-button" onClick={()=>void accept(base)}>接受初始图</button>}</div>}
   {base?.imageFile&&<div className="ym-inline-form"><button className="ym-button" onClick={()=>setMethod("choose")}>选择表情准备方式</button>{method==="generate"&&<button className="ym-button" onClick={()=>void run(generate)}>生成尚缺的基础表情</button>}</div>}
   {!working&&progress&&<p role="status" className="ym-page-note">{progress}</p>}
   <div className="ym-pet-setup-grid">{items.map(item=><article className="ym-page-card" key={item.id}><h3>{item.name}</h3>{item.imageFile||item.previewFile?<PetArtworkImage personaId={personaId} item={item} preview={!!item.previewFile}/>:<div className="ym-pet-setup-placeholder"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="4"/><circle cx="9" cy="9" r="2"/><path d="m4 18 5-5 4 4 3-3 5 5"/></svg><span>待上传 · {item.name}</span></div>}<p className="ym-page-note">{item.previewFile?"等待接受":item.imageFile?"已接受":method==="manual"?"上传透明 PNG / WebP 图片":"尚无图片"}</p>{method==="manual"&&<label className="ym-field">上传 {item.name} 图片<input type="file" accept="image/png,image/webp" onChange={event=>{const file=event.target.files?.[0];event.target.value="";if(file)void uploadExpression(item,file);}}/></label>}<div className="ym-inline-form">{item.previewFile&&<button className="ym-button" onClick={()=>void accept(item)}>接受 {item.name}</button>}{method==="generate"&&<button className="ym-button" disabled={!base?.imageFile} onClick={()=>void retry(item)}>重试 {item.name}</button>}</div></article>)}</div>
   <footer className="ym-inline-form"><button className="ym-button" data-variant="primary" disabled={!base?.imageFile||!standby} onClick={()=>void enable()}>{items.some(a=>a.previewFile)?"接受全部预览并启用图片模式":"启用图片模式"}</button><button className="ym-button" onClick={onClose}>关闭，保留当前图片</button></footer>
  </fieldset>
  {method==="choose"&&<MethodDialog busy={working} onSelect={selectMethod} onClose={()=>setMethod(null)}/>}
 </dialog>;
}
