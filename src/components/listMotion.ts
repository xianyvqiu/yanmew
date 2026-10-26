const lists=new WeakMap<HTMLElement,{users:number;stop:()=>void}>();
/** 原位展开改变布局后，仅用位移动画让后续卡片平滑到达新位置。 */
export function observeListMotion(list:HTMLElement){
  const existing=lists.get(list);if(existing){existing.users++;return ()=>release();}
  let previous=new Map<HTMLElement,number>(),frame=0;const active=new Map<HTMLElement,Animation>();
  const children=()=>Array.from(list.children).filter((node):node is HTMLElement=>node instanceof HTMLElement);
  const measure=()=>{frame=0;const reduced=window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;const next=new Map<HTMLElement,number>();
    for(const node of children()){
      let offset=0;const animation=active.get(node);if(animation){const transform=getComputedStyle(node).transform;if(transform&&transform!=="none"){try{offset=new DOMMatrixReadOnly(transform).m42;}catch{/* 从上次布局位置开始。 */}}animation.cancel();active.delete(node);}
      const top=node.getBoundingClientRect().top-list.getBoundingClientRect().top;next.set(node,top);const old=previous.get(node);
      if(!reduced&&old!==undefined&&Math.abs(old+offset-top)>1&&node.animate){const a=node.animate([{transform:`translateY(${old+offset-top}px)`},{transform:"translateY(0)"}],{duration:280,easing:"cubic-bezier(.22,1,.36,1)"});active.set(node,a);a.onfinish=()=>{if(active.get(node)===a)active.delete(node);};}
    }previous=next;
  };
  const schedule=()=>{if(!frame)frame=requestAnimationFrame(measure);};measure();
  const mutations=new MutationObserver(schedule);mutations.observe(list,{childList:true,subtree:true,characterData:true,attributes:true,attributeFilter:["open"]});
  const resize=typeof ResizeObserver!=="undefined"?new ResizeObserver(schedule):null;resize?.observe(list);
  lists.set(list,{users:1,stop:()=>{mutations.disconnect();resize?.disconnect();cancelAnimationFrame(frame);active.forEach(a=>a.cancel());}});
  return ()=>release();
  function release(){const state=lists.get(list);if(state&&--state.users===0){state.stop();lists.delete(list);}}
}
