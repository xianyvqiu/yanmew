import {useEffect,useState} from "react";
import {souvenirsApi} from "../api";
import type {CurrentContext,Souvenir} from "../types";
export default function SouvenirImage({item,context}:{item:Souvenir;context:CurrentContext}){
  const[view,setView]=useState<{dataUrl:string|null;notice:string|null}>();
  useEffect(()=>{let disposed=false;setView(undefined);void souvenirsApi.image(context,item.id).then(v=>{if(!disposed)setView(v);}).catch(()=>{if(!disposed)setView({dataUrl:null,notice:"伴手礼图片暂时无法读取"});});return()=>{disposed=true;};},[item.id,item.imageFile,context.personaId,context.sessionId,context.contextRevision]);
  return view?.dataUrl?<img src={view.dataUrl} alt="伴手礼图片" draggable={false}/>:<span className="ym-souvenir-image-state" title={view?.notice??"正在读取图片"}>{view?"图片暂不可用":"读取中…"}</span>;
}
