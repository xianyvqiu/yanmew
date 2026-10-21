import{useEffect,useLayoutEffect,useRef,useState}from"react";
import{travelApi}from"../api";
import type{PetActivity}from"../types";
export function elapsedTravel(start:string,now:number){const value=Date.parse(start);if(!Number.isFinite(value))return null;const minutes=Math.max(0,Math.floor((now-value)/60000));return `${String(Math.floor(minutes/60)).padStart(2,"0")}:${String(minutes%60).padStart(2,"0")}`;}
export default function PetAwayElapsed({view}:{view:PetActivity|null}){
 const label=useRef<HTMLDivElement>(null);
 const[trip,setTrip]=useState<{key:string;startedAt:string}|null>(null);const[now,setNow]=useState(Date.now);
 const key=`${view?.personaId}:${view?.sessionId}:${view?.contextRevision}`;
 useEffect(()=>{if(view?.activity!=="away"||!view.sessionId){setTrip(null);return;}let alive=true;let sequence=0;const refresh=()=>{const request=++sequence;void travelApi.current({personaId:view.personaId,sessionId:view.sessionId,contextRevision:view.contextRevision}).then(value=>{if(alive&&request===sequence){setNow(Date.now());setTrip(value?.status==="away"?{key,startedAt:value.startedAt}:null);}}).catch(()=>{if(alive&&request===sequence)setTrip(null);});};refresh();const timer=setInterval(()=>setNow(Date.now()),1000);let stop:(()=>void)|undefined;void travelApi.listen(refresh).then(fn=>{if(alive)stop=fn;else fn();}).catch(()=>{});const focus=()=>{setNow(Date.now());refresh();};window.addEventListener("focus",focus);return()=>{alive=false;sequence++;clearInterval(timer);stop?.();window.removeEventListener("focus",focus);};},[key,view?.activity]);
 const elapsed=trip?.key===key?elapsedTravel(trip.startedAt,now):null;
 useLayoutEffect(()=>{const node=label.current;const root=node?.closest(".pet-window");if(!node||!root)return;let shift=parseFloat(node.style.getPropertyValue("--away-shift"))||0;const fit=()=>{const a=node.getBoundingClientRect(),b=root.getBoundingClientRect();if(!a.width||!b.width)return;const dx=a.left<b.left+8?b.left+8-a.left:a.right>b.right-8?b.right-8-a.right:0;if(Math.abs(dx)>.5){shift+=dx;node.style.setProperty("--away-shift",`${shift}px`);}};fit();const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(fit):null;observer?.observe(root);observer?.observe(node);window.addEventListener("resize",fit);return()=>{observer?.disconnect();window.removeEventListener("resize",fit);};},[elapsed,view?.activity]);
 return view?.activity==="away"&&elapsed!==null?<div ref={label} className="pet-away-elapsed" aria-label="已经出门时间"><span>已经出门时间：</span><time>{elapsed}</time></div>:null;
}
