import {useEffect,useRef,useState} from "react";
import {travelApi} from "../api";
import type {PetActivity} from "../types";

/** 只提示已观察到的同会话正常返回，取消、切换及首次打开不补弹历史。 */
export function usePetReturnNotice(view:PetActivity|null,enabled:boolean){
 const previous=useRef<{key:string;away:boolean}>();
 const [visible,setVisible]=useState(false);
 const key=view?.sessionId?`${view.personaId}:${view.sessionId}:${view.contextRevision}`:"";
 const away=view?.activity==="away";
 useEffect(()=>{
  const before=previous.current;previous.current={key,away};setVisible(false);
  if(!enabled||!key||away||!before?.away||before.key!==key||!view)return;
  let disposed=false,timer:ReturnType<typeof setTimeout>|undefined;
  const context={personaId:view.personaId,sessionId:view.sessionId,contextRevision:view.contextRevision};
  void travelApi.list(context,null,1).then(items=>{
   if(disposed||items[0]?.status!=="returned")return;
   setVisible(true);timer=setTimeout(()=>{if(!disposed)setVisible(false);},5000);
  }).catch(()=>{});
  return()=>{disposed=true;if(timer)clearTimeout(timer);};
 },[key,away,enabled]);
 return visible;
}
