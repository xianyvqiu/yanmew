import {useEffect,useRef} from "react";

/** 只在打开或更换编辑对象时定位，输入和后台刷新不抢回焦点。 */
export function useRevealForm(open=true,identity?:string) {
  const ref=useRef<HTMLFormElement>(null);
  useEffect(()=>{
    if(!open)return;
    const reveal=()=>{
      const form=ref.current;if(!form)return;
      form.scrollIntoView?.({block:"start",behavior:window.matchMedia?.("(prefers-reduced-motion: reduce)").matches?"auto":"smooth"});
      form.querySelector<HTMLElement>('input:not([type="hidden"]):not(:disabled),textarea:not(:disabled),select:not(:disabled)')?.focus({preventScroll:true});
    };
    if(typeof requestAnimationFrame!=="function"){reveal();return;}
    const frame=requestAnimationFrame(reveal);return()=>cancelAnimationFrame(frame);
  },[open,identity]);
  return ref;
}
