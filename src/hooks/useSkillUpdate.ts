import {useEffect,useRef,useState} from "react";
import {companionApi,type SkillPreview} from "../companionApi";
import {errorMessage} from "../api";
type Context={personaId:string;sessionId:string|null;contextRevision:number};
export function useSkillUpdate(context:Context){
 const key=`${context.personaId}:${context.sessionId}:${context.contextRevision}`;
 const [state,setState]=useState<{key:string;items:SkillPreview[]}>({key,items:[]});
 const [error,setError]=useState(""),[busy,setBusy]=useState(false);
 const [actionError,setActionError]=useState("");
 const methods=useRef({refresh:async()=>{},resolve:async(_id:string,_accept:boolean)=>{}});
 useEffect(()=>{
  let alive=true,version=0,locked=false;let stop:(()=>void)|undefined;
  setState({key,items:[]});setError("");setActionError("");setBusy(false);
  const scope=context.sessionId?{...context,sessionId:context.sessionId}:null;
  const read=async()=>{if(!scope||!alive||locked)return;const token=++version;try{
   const items=await companionApi.skillPreviews(scope);if(alive&&version===token){setState({key,items:Array.isArray(items)?items:[]});setError("");}
  }catch(e){if(alive&&version===token)setError(errorMessage(e));}};
  const resolve=async(id:string,accept:boolean)=>{
   if(!scope||!alive||locked)return;locked=true;version++;setBusy(true);setError("");setActionError("");
   try{await companionApi.resolveSkillPreview(scope,id,accept);if(alive)setState(old=>({key,items:old.items.filter(p=>p.id!==id)}));}
   catch(e){if(alive)setActionError(errorMessage(e));}
   finally{locked=false;if(alive){await read();if(alive)setBusy(false);}}
  };
  methods.current={refresh:read,resolve};
  const refresh=()=>{void read();};
  void companionApi.listenSkills(refresh).then(unlisten=>{if(!alive){unlisten();return;}stop=unlisten;void read();}).catch(()=>{void read();});
  window.addEventListener("focus",refresh);
  return()=>{alive=false;version++;stop?.();window.removeEventListener("focus",refresh);};
 },[key]);
 return{preview:state.key===key?state.items[0]:undefined,error:actionError||error,busy,refresh:()=>{setActionError("");return methods.current.refresh();},resolve:(id:string,accept:boolean)=>methods.current.resolve(id,accept)};
}
