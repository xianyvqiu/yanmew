import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {errorMessage} from "../../api";
import {onboardingSteps} from "./onboardingSteps";
export {onboardingSteps};
export type GuideState={token:number;step:number;viewing:boolean;target:string;page:string;title:string;text:string;total:number;busy:boolean;error:string};
export const onboardingApi={pending:()=>invoke<boolean>("get_onboarding_pending"),finish:()=>invoke<void>("finish_onboarding"),publish:(state:GuideState|null)=>invoke<void>("set_onboarding_state",{state}),listen:(fn:(action:{token:number;step:number;action:string})=>void)=>listen<{token:number;step:number;action:string}>("onboarding-action",e=>fn(e.payload))};
export function OnboardingDialog({onNavigate,onClose,onTarget}:{onNavigate:(page:string)=>void;onClose:()=>void;onTarget?:(page:string|null)=>void}) {
 const [step,setStep]=useState(0),[viewing,setViewing]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState("");
 const layerFailed=useRef(false);const publications=useRef(Promise.resolve());const token=useRef(Date.now());const saving=useRef(false);const callbacks=useRef({onNavigate,onClose,onTarget});callbacks.current={onNavigate,onClose,onTarget};
 const state=useRef({step,viewing});state.current={step,viewing};
 const item=onboardingSteps[step];
 const finish=async()=>{if(saving.current)return;saving.current=true;setBusy(true);setError("");try{await publications.current.catch(()=>{});await onboardingApi.finish();await onboardingApi.publish(null);callbacks.current.onClose();}catch(e){setError(`未能完成教程：${errorMessage(e)}，请重试。`);}finally{saving.current=false;setBusy(false);}};
 const action=useRef<(value:string)=>void>(()=>{});action.current=value=>{if(saving.current)return;if(value==="skip"){void finish();return;}if(value==="previous"){setViewing(false);setStep(s=>Math.max(0,s-1));return;}if(value==="view"||value==="next"){if(!state.current.viewing){if(!item.target)callbacks.current.onNavigate(item.page);setViewing(true);}else if(step===onboardingSteps.length-1)void finish();else{setViewing(false);setStep(s=>s+1);}}};
 useEffect(()=>{let alive=true;let stop:(()=>void)|undefined;void onboardingApi.listen(value=>{if(value.token===token.current&&value.step===state.current.step)action.current(value.action);}).then(fn=>{if(alive)stop=fn;else fn();}).catch(e=>setError(errorMessage(e)));return()=>{alive=false;stop?.();callbacks.current.onTarget?.(null);publications.current=publications.current.catch(()=>{}).then(()=>onboardingApi.publish(null)).catch(()=>{});};},[]);
 useEffect(()=>{if(saving.current||layerFailed.current)return;callbacks.current.onTarget?.(item.target?null:item.page);const guide:GuideState={token:token.current,step,viewing,target:item.target??"settings",page:item.page,title:item.title,text:viewing?item.detail:item.text,total:onboardingSteps.length,busy,error};let alive=true;publications.current=publications.current.catch(()=>{}).then(()=>onboardingApi.publish(guide)).catch(e=>{if(alive)setError(errorMessage(e));});return()=>{alive=false;};},[step,viewing,busy,error,item]);
 useEffect(()=>{let disposed=false;let stop:(()=>void)|undefined;void listen<{token:number;step:number;error:string}>("onboarding-error",e=>{if(e.payload.token!==token.current||e.payload.step!==state.current.step)return;layerFailed.current=true;setError(`教程层显示失败：${e.payload.error}。可跳过后继续使用。`);publications.current=publications.current.catch(()=>{}).then(()=>onboardingApi.publish(null)).catch(()=>{});}).then(fn=>{if(disposed)fn();else stop=fn;}).catch(()=>{});return()=>{disposed=true;stop?.();};},[]);
 // 原生层失败时，设置页仍提供可见的退出入口，不让用户被教程困住。
 return error?<div role="alert" data-onboarding-control className="ym-page-error">{error}<button className="ym-button" disabled={busy} onClick={()=>void finish()}>跳过教程</button></div>:null;
}
export default function OnboardingGuide({replay,onNavigate,onTarget}:{replay:number;onNavigate:(page:string)=>void;onTarget?:(page:string|null)=>void}){
 const [open,setOpen]=useState(false);const manual=useRef(false);
 useEffect(()=>{let alive=true;void onboardingApi.pending().then(pending=>{if(alive&&!manual.current)setOpen(pending);}).catch(()=>{});return()=>{alive=false;};},[]);
 useEffect(()=>{if(replay>0){manual.current=true;setOpen(true);}},[replay]);
 return open?<OnboardingDialog onNavigate={onNavigate} onTarget={onTarget} onClose={()=>setOpen(false)}/>:null;
}
