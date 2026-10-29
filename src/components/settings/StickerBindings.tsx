import {useRef,useState} from "react";
import {errorMessage} from "../../api";
import {stickerApi,type StickerLibrary} from "../../stickerApi";
import {StickerPager,StickerSearch,useStickerPage} from "./StickerLibraryShared";
import "./StickerLibraryPanel.css";
export default function StickerBindings({personaId}:{personaId:string}){
  const browser=useStickerPage<StickerLibrary>({entity:"libraries"}),[busy,setBusy]=useState(false),[error,setError]=useState("");const lock=useRef(false);
  const bind=async(library:StickerLibrary)=>{if(lock.current)return;lock.current=true;setBusy(true);setError("");try{await stickerApi.mutate({op:"bind",libraryId:library.id,personaId,bound:!library.personaIds.includes(personaId)});browser.reload();}catch(e){setError(errorMessage(e));}finally{lock.current=false;setBusy(false);}};
  return <section className="ym-page-card ym-sticker-bindings"><h3>聊天表情包库</h3><p className="ym-page-note">绑定后，伙伴可以在聊天消息中选择发送这些表情包。可绑定多个库。</p>
    <StickerSearch browser={browser} label="表情包库" disabled={busy}/>{(error||browser.error)&&<p role="alert" className="ym-page-error">{error||browser.error}</p>}
    {browser.items.map(library=><div className="ym-sticker-binding-row" key={library.id}><div><strong>{library.title}</strong><p className="ym-page-note">{library.description||`${library.count} 个表情包`}</p></div><button type="button" className="ym-button" disabled={busy||browser.loading} aria-pressed={library.personaIds.includes(personaId)} onClick={()=>void bind(library)}>{library.personaIds.includes(personaId)?"已绑定 · 解除":"绑定"}</button></div>)}
    {!browser.loading&&!browser.total&&!browser.error&&<p className="ym-page-note">还没有表情包库，可到全局设置的“聊天表情包库”新建。</p>}
    <StickerPager browser={browser} label="表情包库" disabled={busy}/>
  </section>;
}
