import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import {observeListMotion} from "./listMotion";
export default function InlineDetailCard({title,summary,status,meta,thumbnail,expanded,onToggle,disabled,detailLabel,children}:{
  title:ReactNode;summary:ReactNode;status?:ReactNode;meta?:ReactNode;thumbnail?:ReactNode;expanded:boolean;onToggle:()=>void;disabled?:boolean;detailLabel:string;children:ReactNode;
}) {
  const id=useId(),[present,setPresent]=useState(expanded),cached=useRef(children),body=useRef<HTMLDivElement>(null),button=useRef<HTMLButtonElement>(null);
  if(expanded)cached.current=children;
  useLayoutEffect(()=>{const list=button.current?.closest<HTMLElement>("ul,ol,.ym-object-grid,.ym-memory-grid");return list?observeListMotion(list):undefined;},[]);
  useEffect(()=>{
    if(expanded){setPresent(true);return;}
    const timer=setTimeout(()=>setPresent(false),window.matchMedia?.("(prefers-reduced-motion: reduce)").matches?0:220);
    return()=>clearTimeout(timer);
  },[expanded]);
  useLayoutEffect(()=>{
    if(!body.current)return;
    if(!expanded&&body.current.contains(document.activeElement))button.current?.focus({preventScroll:true});
    body.current.inert=!expanded;
  },[expanded]);
  return <article className="ym-letter-card" data-expanded={expanded}>
    <button ref={button} type="button" className="ym-letter-item" data-thumbnail={!!thumbnail} disabled={disabled} aria-expanded={expanded} aria-controls={id} onClick={onToggle}>
      {thumbnail&&<span className="ym-card-thumbnail">{thumbnail}</span>}<span className="ym-card-copy">
      <span className="ym-letter-card-heading"><strong>{title}</strong>{" "}<span className="ym-page-note">{meta}</span></span>{" "}
      <span className="ym-card-summary" hidden={expanded}>{summary}</span>{" "}{status&&<span className="ym-card-status">{status}</span>}<span className="ym-letter-disclosure">{expanded?"收起":"查看详情"}<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg></span></span>
    </button>
    {(present||expanded)&&<div ref={body} id={id} className="ym-letter-detail" role="region" aria-label={detailLabel} aria-hidden={!expanded} data-closing={!expanded}>{cached.current}</div>}
  </article>;
}
