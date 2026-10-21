import { useLayoutEffect, useRef, type ReactNode } from "react";

/** 动画作用于现有DOM，不使用key重建表单或打断后台任务。 */
export default function MotionRegion({motionKey,children}:{motionKey:string;children:ReactNode}) {
    const ref=useRef<HTMLDivElement>(null);
    useLayoutEffect(()=>{
        if (typeof matchMedia!=="undefined"&&matchMedia("(prefers-reduced-motion: reduce)").matches) return;
        const animation=ref.current?.animate?.([{opacity:.85,transform:"translateY(5px)"},{opacity:1,transform:"translateY(0)"}],{duration:200,easing:"cubic-bezier(.22,1,.36,1)"});
        return()=>animation?.cancel();
    },[motionKey]);
    useLayoutEffect(()=>{
        const root=ref.current;if(!root||typeof matchMedia!=="undefined"&&matchMedia("(prefers-reduced-motion: reduce)").matches)return;
        const seen=new WeakSet<Element>();const running=new Set<Animation>();
        const reveal=(nodes:Element[])=>nodes.forEach((node,index)=>{if(seen.has(node))return;seen.add(node);const animation=(node as HTMLElement).animate?.([{opacity:.75,transform:"translateY(9px)"},{opacity:1,transform:"translateY(0)"}],{duration:280,delay:Math.min(index,4)*60,easing:"cubic-bezier(.22,1,.36,1)"});if(animation){running.add(animation);animation.onfinish=()=>running.delete(animation);}});
        const observer=typeof IntersectionObserver!=="undefined"?new IntersectionObserver(entries=>{const visible=entries.filter(e=>e.isIntersecting).map(e=>e.target);reveal(visible);visible.forEach(node=>observer?.unobserve(node));},{threshold:.05}):null;
        const scan=()=>{const nodes=Array.from(root.querySelectorAll(".ym-object-grid>*,.ym-persona-grid>*,.ym-provider-grid>*"));for(const node of nodes)if(!seen.has(node)){if(observer)observer.observe(node);else reveal([node]);}};
        scan();const mutation=new MutationObserver(records=>{if(records.some(r=>Array.from(r.addedNodes).some(n=>n.nodeType===1)))scan();});mutation.observe(root,{childList:true,subtree:true});
        return()=>{mutation.disconnect();observer?.disconnect();running.forEach(a=>a.cancel());};
    },[motionKey]);
    return <div ref={ref} className="ym-motion-region">{children}</div>;
}
