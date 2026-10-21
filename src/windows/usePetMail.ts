import{useEffect,useRef,useState}from"react";
import{getPetMailStatus,lettersApi,travelApi,errorMessage}from"../api";
import type{CurrentContext}from"../types";
export function usePetMail(context:CurrentContext,away:boolean){
 const key=`${context.personaId}:${context.sessionId}:${context.contextRevision}`;const current=useRef(key);current.current=key;
 const[status,setStatus]=useState<{key:string;travelId:string|null;unread:number}|null>(null);const[draft,setDraft]=useState("");const[writing,setWriting]=useState(false);const[owner,setOwner]=useState<string|null>(null);const[busy,setBusy]=useState(false);const[error,setError]=useState("");const[notice,setNotice]=useState("");const lock=useRef<string|null>(null);
 useEffect(()=>{setDraft("");setWriting(false);setOwner(null);setBusy(false);setError("");setNotice("");},[key]);
 useEffect(()=>{let alive=true;let sequence=0;const stops:(()=>void)[]=[];if(!away||!context.sessionId){setStatus(null);return;}const read=()=>{const token=++sequence;void getPetMailStatus(context).then(s=>{if(alive&&token===sequence){setStatus({...s,key});setError("");}}).catch(e=>{if(alive&&token===sequence)setError(errorMessage(e));});};
 for(const api of[lettersApi,travelApi])void api.listen(e=>{if(e.sessionId===context.sessionId)read();}).then(stop=>{if(alive)stops.push(stop);else stop();}).catch(()=>{});read();window.addEventListener("focus",read);const timer=setInterval(read,30000);return()=>{alive=false;sequence++;stops.forEach(s=>s());clearInterval(timer);window.removeEventListener("focus",read);};},[key,away]);
 const trip=status?.key===key?status.travelId:null;
 const begin=()=>{if(draft.trim()&&owner!==trip){setWriting(true);setError("这份草稿属于之前的旅行，请先复制或清空，不能转投新旅行。");return;}setOwner(trip);setWriting(true);};
 const send=async()=>{if(lock.current===key||!trip||owner!==trip||!draft.trim())return;lock.current=key;setBusy(true);setError("");const submitted=key;try{await lettersApi.send(context,{travelId:trip,content:draft,replyToId:null});if(current.current===submitted){setDraft("");setWriting(false);setNotice("信已寄出，等它回信吧。");}}catch(e){if(current.current===submitted)setError(errorMessage(e));}finally{if(lock.current===submitted)lock.current=null;if(current.current===submitted)setBusy(false);}};
 return{ready:!!trip,draft,setDraft:(value:string)=>{setDraft(value);if(!value.trim()&&owner!==trip){setOwner(trip);setError("");}},writing,begin,cancel:()=>setWriting(false),busy,error,notice,send,canSend:away&&!!trip&&owner===trip,hasUnread:away&&status?.key===key&&status.unread>0};
}
