import {useEffect,useState} from "react";
import {errorMessage,souvenirsApi} from "../api";
export default function SouvenirImageSettings(){
  const[enabled,setEnabled]=useState<boolean>();const[busy,setBusy]=useState(false);const[error,setError]=useState("");
  useEffect(()=>{let disposed=false,stop:(()=>void)|undefined;const load=()=>{void Promise.resolve().then(()=>souvenirsApi.imageSettings()).then(v=>{if(!disposed)setEnabled(v);}).catch(cause=>{if(!disposed)setError(errorMessage(cause));});};load();void Promise.resolve().then(()=>souvenirsApi.listenImageSettings(v=>{if(!disposed)setEnabled(v);})).then(fn=>{if(disposed)fn();else stop=fn;}).catch(()=>{});window.addEventListener("focus",load);return()=>{disposed=true;stop?.();window.removeEventListener("focus",load);};},[]);
  return <div className="ym-souvenir-image-settings"><label><span><strong>为新伴手礼生成图片</strong><small>默认开启；需要已启用并配置支持透明输出的生图服务，可能消耗服务商额度。旧礼物不补生图，关闭或生成失败时保留文字。</small></span><input type="checkbox" role="switch" aria-label="为新伴手礼生成图片" checked={enabled??false} disabled={busy||enabled===undefined} onChange={event=>{const value=event.target.checked;setBusy(true);setError("");void souvenirsApi.setImageSettings(value).then(setEnabled).catch(cause=>setError(errorMessage(cause))).finally(()=>setBusy(false));}}/></label>{error&&<p className="ym-page-error" role="alert">{error}</p>}</div>;
}
