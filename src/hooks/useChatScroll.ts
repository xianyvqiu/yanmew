import {useLayoutEffect,useRef} from "react";

/** 只跟随接近底部的读者；观察真实内容尺寸，流式片段不启动叠加滚动动画。 */
export function useChatScroll<T extends HTMLElement=HTMLElement>(scope:string,ready:boolean,revision:unknown,stream?:string,firstId?:string){
  const viewport=useRef<T>(null),content=useRef<HTMLDivElement>(null);
  const state=useRef({scope:"",initialized:false,following:true,firstId:"",height:0,top:0,virtualLayout:false});
  const frame=useRef<number>(), layoutFrame=useRef<number>();
  const measure=()=>{
    const box=viewport.current,s=state.current;if(!box||!ready)return;
    const prepended=s.initialized&&firstId&&s.firstId&&/^\d+$/.test(firstId)&&/^\d+$/.test(s.firstId)&&BigInt(firstId)<BigInt(s.firstId);
    if(prepended)box.scrollTop=s.top+box.scrollHeight-s.height;
    else if(!s.initialized||s.following)box.scrollTop=box.scrollHeight;
    s.initialized=true;s.firstId=firstId??"";s.height=box.scrollHeight;s.top=box.scrollTop;
  };
  const latest=useRef(measure);latest.current=measure;
  useLayoutEffect(()=>{
    if(state.current.scope!==scope){state.current={scope,initialized:false,following:true,firstId:"",height:0,top:0,virtualLayout:false};}
    if(!ready){state.current.initialized=false;return;}
    measure();
  },[scope,ready,revision,stream,firstId]);
  useLayoutEffect(()=>{
    const box=viewport.current,body=content.current;if(!box||!body)return;
    const remember=()=>{const s=state.current;if(!s.virtualLayout)s.following=box.scrollHeight-box.scrollTop-box.clientHeight<=96;s.top=box.scrollTop;s.height=box.scrollHeight;};
    const wheel=(event:WheelEvent)=>{
      if(event.ctrlKey)return;
      event.preventDefault();event.stopPropagation();state.current.virtualLayout=false;
      const unit=event.deltaMode===1?20:event.deltaMode===2?box.clientHeight:1;
      box.scrollTop=Math.max(0,Math.min(box.scrollHeight-box.clientHeight,box.scrollTop+event.deltaY*unit));remember();
    };
    const schedule=()=>{if(frame.current!==undefined)cancelAnimationFrame(frame.current);frame.current=requestAnimationFrame(()=>{frame.current=undefined;latest.current();});};
    const observer=typeof ResizeObserver==="undefined"?null:new ResizeObserver(schedule);
    observer?.observe(box);observer?.observe(body);
    box.addEventListener("scroll",remember);box.addEventListener("wheel",wheel,{passive:false});
    return()=>{observer?.disconnect();box.removeEventListener("scroll",remember);box.removeEventListener("wheel",wheel);if(frame.current!==undefined)cancelAnimationFrame(frame.current);};
  },[scope,ready]);
  const onVirtualLayout=()=>{
    state.current.virtualLayout=true;
    if(layoutFrame.current!==undefined)cancelAnimationFrame(layoutFrame.current);
    layoutFrame.current=requestAnimationFrame(()=>{latest.current();state.current.virtualLayout=false;layoutFrame.current=undefined;});
  };
  useLayoutEffect(()=>()=>{if(layoutFrame.current!==undefined)cancelAnimationFrame(layoutFrame.current);},[]);
  return {viewport,content,onVirtualLayout};
}
