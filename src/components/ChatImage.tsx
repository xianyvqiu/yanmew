import {useEffect,useState} from "react";
import {chatApi,errorMessage} from "../api";
import type {ChatImageRecord,CurrentContext} from "../types";
export default function ChatImage({image,context}:{image:ChatImageRecord;context:CurrentContext}) {
  const [url,setUrl]=useState("");const [error,setError]=useState("");const [expanded,setExpanded]=useState(false);
  useEffect(()=>{let disposed=false;setUrl("");setError("");void chatApi.image(context,image.id).then(value=>{if(!disposed)setUrl(value);}).catch(cause=>{if(!disposed)setError(errorMessage(cause));});return()=>{disposed=true;};},[image.id,context.personaId,context.sessionId,context.contextRevision]);
  return <figure className="ym-chat-image" data-kind={image.sourceKind??"image"}>{error?<p role="status">{image.filename}：{error}</p>:url?<button type="button" aria-label={`查看图片 ${image.filename}`} aria-expanded={expanded} onClick={()=>setExpanded(v=>!v)}><img src={url} alt={image.filename} data-expanded={expanded} /></button>:<span>正在读取图片…</span>}{image.sourceKind!=="sticker"&&<figcaption>{image.filename}</figcaption>}</figure>;
}
