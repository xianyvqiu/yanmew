import {useEffect,useState} from "react";
export type BackgroundViewport={width:number;height:number};
const key="yanmew-chat-background-viewport";
export function readBackgroundViewport():BackgroundViewport{
  const host=document.querySelector<HTMLElement>(".ym-chat-background-host");
  if(host&&host.clientWidth>0&&host.clientHeight>0)return {width:host.clientWidth,height:host.clientHeight};
  try {const value=JSON.parse(localStorage.getItem(key)??"null") as BackgroundViewport|null;if(value&&Number.isFinite(value.width)&&Number.isFinite(value.height)&&value.width>=100&&value.height>=100)return value;}catch{/* 无可用窗口尺寸时使用初始预览画幅。 */}
  return {width:960,height:600};
}
export function rememberBackgroundViewport(element:HTMLElement){
  const save=()=>{if(element.clientWidth<100||element.clientHeight<100)return;try{localStorage.setItem(key,JSON.stringify({width:element.clientWidth,height:element.clientHeight}));}catch{/* 尺寸记忆失败不影响聊天。 */}};
  save();if(typeof ResizeObserver==="undefined")return ()=>{};const observer=new ResizeObserver(save);observer.observe(element);return ()=>observer.disconnect();
}
export function useBackgroundViewport(){
  const[value,setValue]=useState(readBackgroundViewport);
  useEffect(()=>{const changed=()=>setValue(readBackgroundViewport());window.addEventListener("storage",changed);window.addEventListener("focus",changed);changed();return()=>{window.removeEventListener("storage",changed);window.removeEventListener("focus",changed);};},[]);
  return value;
}
