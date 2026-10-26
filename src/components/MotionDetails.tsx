import {Children,cloneElement,isValidElement,useEffect,useLayoutEffect,useRef,useState,type ReactElement,type ReactNode} from "react";
import {observeListMotion} from "./listMotion";
/** 保留原生摘要语义，退出动效结束后再收回详细内容。 */
export default function MotionDetails({children,className}:{children:ReactNode;className?:string}){
  const[expanded,setExpanded]=useState(false),[present,setPresent]=useState(false);const root=useRef<HTMLDetailsElement>(null),body=useRef<HTMLDivElement>(null);
  const nodes=Children.toArray(children);const summary=nodes[0];
  useEffect(()=>{if(expanded){setPresent(true);return;}const timer=setTimeout(()=>setPresent(false),window.matchMedia?.("(prefers-reduced-motion: reduce)").matches?0:220);return()=>clearTimeout(timer);},[expanded]);
  useLayoutEffect(()=>{if(body.current){if(!expanded&&body.current.contains(document.activeElement))root.current?.querySelector("summary")?.focus({preventScroll:true});body.current.inert=!expanded;}},[expanded]);
  useLayoutEffect(()=>{const parent=root.current?.parentElement;return parent?observeListMotion(parent):undefined;},[]);
  return <details ref={root} className={className} open={expanded||present} onClick={event=>{const target=event.target as Element;const clicked=target.closest("summary");if(clicked?.parentElement===root.current&&!target.closest("a,button,input,select")){event.preventDefault();setExpanded(v=>!v);}}}>
    {isValidElement(summary)?cloneElement(summary as ReactElement<{"aria-expanded":boolean}>,{"aria-expanded":expanded}):summary}
    <div ref={body} className="ym-motion-details-body" aria-hidden={!expanded} data-closing={!expanded}>{nodes.slice(1)}</div>
  </details>;
}
