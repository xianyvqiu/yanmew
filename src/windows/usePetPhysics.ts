import {useEffect, useRef, type RefObject} from "react";
import type {PetVelocity, Toss} from "./usePetDesktop";

/** Web Animations 不改 DOM 尺寸与样式属性，保持身体锚点、命中区和按钮稳定。 */
export function usePetPhysics(root:RefObject<HTMLDivElement>, dragging:boolean, velocity:PetVelocity|undefined|null, toss:Toss|null, enabled:boolean) {
  const previous=useRef<Animation|null>(null), phase=useRef<string>();
  const x=velocity?.x??0, y=velocity?.y??0;
  const flightX=toss?.velocity?.x??0, flightY=toss?.velocity?.y??0;
  useEffect(()=>{
    const element=root.current?.querySelector<HTMLElement>(".pet-expression");
    if(!element||typeof element.animate!=="function")return;
    const reduced=window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const nextPhase=enabled?(dragging?"drag":toss?.phase??"rest"):"rest";
    const landed=nextPhase==="landed"&&phase.current!=="landed";
    const active=nextPhase==="drag"||nextPhase==="up"||nextPhase==="down";
    const vx=dragging?x:flightX,vy=dragging?y:flightY;
    const angle=active&&!reduced?Math.max(-30,Math.min(30,dragging?vx/65:Math.atan2(vx,Math.abs(vy)+240)*180/Math.PI*.6)):0;
    const stretch=dragging&&enabled&&!reduced?Math.min(0.045,Math.hypot(vx,vy)/40000):0;
    const current=getComputedStyle(element);
    const start={rotate:current.rotate==="none"?"0deg":current.rotate,scale:current.scale==="none"?"1 1":current.scale};
    const frames:Keyframe[]=landed&&!reduced?[
      start,{rotate:"0deg",scale:"1.12 .84",offset:.18},{rotate:"0deg",scale:".95 1.07",offset:.48},
      {rotate:"0deg",scale:"1.025 .98",offset:.72},{rotate:"0deg",scale:"1 1"},
    ]:[start,{rotate:`${angle}deg`,scale:`${1-stretch} ${1+stretch}`}];
    const old=previous.current;
    previous.current=element.animate(frames,{duration:reduced?1:landed?620:active?150:280,easing:landed?"ease-out":"cubic-bezier(.22,1.25,.36,1)",fill:"forwards"});
    old?.cancel();phase.current=nextPhase;
  },[root,dragging,x,y,toss?.phase,flightX,flightY,enabled]);
  useEffect(()=>()=>{previous.current?.cancel();},[]);
}
