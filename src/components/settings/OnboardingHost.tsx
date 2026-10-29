import {useEffect,useLayoutEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {listen,emitTo} from "@tauri-apps/api/event";
import {errorMessage} from "../../api";
import {getCurrentWindow} from "@tauri-apps/api/window";
import type {GuideState} from "./OnboardingGuide";
import {onboardingSteps} from "./onboardingSteps";
import "./OnboardingGuide.css";
type Rect={left:number;top:number;width:number;height:number};
type View={theme?:string;state:GuideState;area:Rect|null;bounds:Rect;panel:Rect};
export const guideBridge={state:()=>invoke<GuideState|null>("get_onboarding_state"),view:()=>invoke<View|null>("get_onboarding_view"),show:(state:GuideState,area:Rect|null)=>invoke<void>("show_onboarding_overlay",{token:state.token,step:state.step,viewing:state.viewing,area,theme:document.documentElement.dataset.theme??"light"}),action:(state:GuideState,action:string)=>invoke<void>("onboarding_action",{token:state.token,step:state.step,action})};
function visible(element:Element|null):element is HTMLElement{if(!(element instanceof HTMLElement))return false;const r=element.getBoundingClientRect();return r.width>0&&r.height>0;}
export function guideTarget(state:GuideState):HTMLElement|null {
 const step=onboardingSteps[state.step];if(!step)return null;
 if(state.target!=="settings"){const target=document.querySelector(step.selector??"main");return visible(target)?target:document.querySelector<HTMLElement>(".ym-chat-history, .pet-face, main");}
 if(state.viewing)return document.querySelector<HTMLElement>("[data-onboarding-content] .ym-page-card")??document.querySelector<HTMLElement>("[data-onboarding-content]");
 return Array.from(document.querySelectorAll<HTMLElement>("[data-onboarding-page]")).find(e=>e.dataset.onboardingPage===state.page&&visible(e))??document.querySelector<HTMLElement>("[data-onboarding-compact]");
}
/** 每个可信主窗口只管理自己的定位与输入边界；独立层显示提示，不取得业务调用权限。 */
export default function OnboardingHost(){
 const [state,setState]=useState<GuideState|null>(null);
 useEffect(()=>{let alive=true;let version=0;const stops:(()=>void)[]=[];const read=()=>{const id=++version;void guideBridge.state().then(s=>{if(alive&&id===version)setState(s);}).catch(()=>{});};void listen("onboarding-state-changed",read).then(fn=>{if(alive){stops.push(fn);read();}else fn();});read();return()=>{alive=false;stops.forEach(fn=>fn());};},[]);
 useEffect(()=>{
  if(!state)return;const label=getCurrentWindow().label;const active=label===state.target;let frame=0;let disposed=false;let sending=false;let dirty=false;let windowRevision=0;let previous="";const stops:(()=>void)[]=[];
  const send=()=>{
   frame=0;if(!active||disposed||sending)return;dirty=false;
   const target=guideTarget(state);const box=target?.getBoundingClientRect();const viewport=target?.closest(".ym-window-shell__content, .ym-window-shell__sidebar")?.getBoundingClientRect();
   let area:Rect|null=null;if(box){const left=Math.max(0,box.left,viewport?.left??0),top=Math.max(0,box.top,viewport?.top??0),right=Math.min(innerWidth,box.right,viewport?.right??innerWidth),bottom=Math.min(innerHeight,box.bottom,viewport?.bottom??innerHeight);if(right>left&&bottom>top)area={left,top,width:right-left,height:bottom-top};}
   const key=JSON.stringify([area,innerWidth,innerHeight,document.documentElement.dataset.theme,windowRevision]);
   if(key===previous)return;previous=key;sending=true;
   void guideBridge.show(state,area).catch(cause=>{if(!disposed)void emitTo("settings","onboarding-error",{token:state.token,step:state.step,error:errorMessage(cause)});}).finally(()=>{sending=false;if(dirty&&!disposed)measure();});
  };
  // 合并同一帧的定位；网络/原生交接中只保留最新位置，不堆积滚动请求。
  const measure=()=>{if(!active||disposed)return;dirty=true;if(!frame&&!sending)frame=requestAnimationFrame(send);};
  const moved=()=>{windowRevision++;measure();};
  const click=(event:Event)=>{if(!(event.target instanceof Element)||event.target.closest("[data-onboarding-control]"))return;const target=active?guideTarget(state):null;const allowed=active&&!state.busy&&target?.contains(event.target);
   if(!allowed||(state.viewing&&state.target==="settings")){event.preventDefault();event.stopImmediatePropagation();return;}
   if(!state.viewing&&event.type==="click"&&!target?.matches("select"))void guideBridge.action(state,"view");
  };
  const change=(event:Event)=>{if(active&&!state.viewing&&event.target instanceof HTMLSelectElement&&event.target.value===state.page)void guideBridge.action(state,"view");};
  const key=(event:KeyboardEvent)=>{if(event.key==="Escape"){event.preventDefault();event.stopImmediatePropagation();void guideBridge.action(state,"skip");}else if((event.target instanceof HTMLInputElement||event.target instanceof HTMLTextAreaElement||event.target instanceof HTMLSelectElement)&&!(active&&!state.viewing&&event.target===guideTarget(state))){event.preventDefault();event.stopImmediatePropagation();}};
  for(const type of ["click","pointerdown","contextmenu","submit"])document.addEventListener(type,click,true);
  document.addEventListener("change",change,true);document.addEventListener("keydown",key,true);
  const reveal=()=>{if(active&&!state.viewing)guideTarget(state)?.scrollIntoView?.({block:"nearest",inline:"nearest",behavior:"smooth"});measure();};
  const revealTimer=window.setTimeout(reveal,100);
  const changes=new MutationObserver(measure);changes.observe(document.body,{childList:true,subtree:true});changes.observe(document.documentElement,{attributes:true,attributeFilter:["data-theme"]});
  const resize=typeof ResizeObserver==="undefined"?null:new ResizeObserver(measure);resize?.observe(document.body);
  window.addEventListener("resize",measure);document.addEventListener("scroll",measure,true);document.addEventListener("transitionend",measure,true);document.addEventListener("animationend",measure,true);
  for(const subscribe of [getCurrentWindow().onMoved.bind(getCurrentWindow()),getCurrentWindow().onResized.bind(getCurrentWindow())])void subscribe(moved).then(fn=>{if(disposed)fn();else stops.push(fn);});
  void listen<{x:number;y:number;deltaX:number;deltaY:number}>("onboarding-scroll",e=>{
   if(!active)return;let element=document.elementFromPoint(e.payload.x,e.payload.y) as HTMLElement|null;
   while(element&&element!==document.body){const style=getComputedStyle(element);if(/auto|scroll/.test(style.overflowY)&&element.scrollHeight>element.clientHeight){element.scrollBy({left:e.payload.deltaX,top:e.payload.deltaY,behavior:"instant"});return;}element=element.parentElement;}
   document.querySelector<HTMLElement>(".ym-window-shell__content")?.scrollBy({top:e.payload.deltaY,left:e.payload.deltaX,behavior:"instant"});
  }).then(fn=>{if(disposed)fn();else stops.push(fn);});
  measure();return()=>{disposed=true;cancelAnimationFrame(frame);clearTimeout(revealTimer);changes.disconnect();resize?.disconnect();stops.forEach(fn=>fn());for(const type of ["click","pointerdown","contextmenu","submit"])document.removeEventListener(type,click,true);document.removeEventListener("change",change,true);document.removeEventListener("keydown",key,true);window.removeEventListener("resize",measure);document.removeEventListener("scroll",measure,true);document.removeEventListener("transitionend",measure,true);document.removeEventListener("animationend",measure,true);};
 },[state]);
 return null;
}
export function OnboardingLayer(){
 const [view,setView]=useState<View|null>(null);const heading=useRef<HTMLHeadingElement>(null);const live=useRef(view);live.current=view;
 useEffect(()=>{let alive=true;let stop:(()=>void)|undefined;let revision=0;const read=()=>{const id=++revision;void guideBridge.view().then(v=>{if(alive&&id===revision)setView(v);}).catch(()=>{});};void listen("onboarding-view-changed",read).then(fn=>{if(alive){stop=fn;read();}else fn();});read();return()=>{alive=false;stop?.();};},[]);
 useEffect(()=>{const key=(event:KeyboardEvent)=>{const current=live.current;if(!current||!["Escape","ArrowLeft","ArrowRight"].includes(event.key))return;event.preventDefault();event.stopPropagation();void guideBridge.action(current.state,event.key==="Escape"?"skip":event.key==="ArrowLeft"?"previous":"next");};document.addEventListener("keydown",key,true);return()=>document.removeEventListener("keydown",key,true);},[]);
 useLayoutEffect(()=>{document.body.classList.add("ym-onboarding-window");return()=>document.body.classList.remove("ym-onboarding-window");},[]);
 useLayoutEffect(()=>{if(view)document.documentElement.dataset.theme=view.theme??"light";},[view?.theme]);
 useEffect(()=>{heading.current?.focus({preventScroll:true});},[view?.state.step,view?.state.viewing]);
 if(!view)return null;const {state,area,bounds,panel}=view;const action=(a:string)=>void guideBridge.action(state,a);
 return <div className="ym-onboarding-native">
  <div className="ym-onboarding-shade" style={bounds} onWheel={e=>{const unit=e.deltaMode===1?16:e.deltaMode===2?innerHeight:1;void emitTo(state.target,"onboarding-scroll",{x:e.clientX-bounds.left,y:e.clientY-bounds.top,deltaX:e.deltaX*unit,deltaY:e.deltaY*unit});}}/>
  {area&&<div className="ym-onboarding-spotlight" style={area}/>}
  <section className="ym-onboarding" role="dialog" aria-modal="false" aria-labelledby="onboarding-heading" style={panel} key={`${state.step}-${state.viewing}`}>
   <header><span>认识颜喵 · {state.step+1} / {state.total}</span><button className="ym-button" disabled={state.busy} onClick={()=>action("skip")}>跳过教程</button></header>
   <h2 ref={heading} tabIndex={-1} id="onboarding-heading">{state.title}</h2><p>{state.text}</p>
   <small>可滚动真实页面；点击高亮处或按钮继续。← / → 切换，Esc 跳过。</small>{state.error&&<p role="alert" className="ym-page-error">{state.error}</p>}
   <footer><button className="ym-button" disabled={state.busy||state.step===0} onClick={()=>action("previous")}>上一步</button><button className="ym-button" disabled={state.busy} onClick={()=>action("next")}>{state.busy?"正在保存…":!state.viewing?state.target==="settings"?"打开此页":"看看这里":state.step===state.total-1?"完成教程":"下一项"}</button></footer>
  </section>
 </div>;
}
