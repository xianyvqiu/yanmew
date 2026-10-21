import { useEffect, useState, type RefObject } from "react";
import { useAccessories } from "../hooks/useAccessories";
import type { CurrentContext } from "../types";
import AccessoryImage from "./AccessoryVisual";

/** 饰品不参与本体排版；默认头顶或用户保存的相对位置共用此层。 */
export default function AccessoryLayer({ context, face, canvas }: { context: CurrentContext; face: RefObject<HTMLDivElement>; canvas: RefObject<HTMLDivElement> }) {
  const list = useAccessories(context, true);
  const items=[...list.items].sort((a,b)=>(a.layerOrder??0)-(b.layerOrder??0)||(BigInt(a.id)<BigInt(b.id)?-1:1));
  return <>{items.map(item=><WornAccessory key={item.id} context={context} face={face} canvas={canvas} item={item} listError={list.error}/>)}</>;
}
function WornAccessory({context,face,canvas,item,listError}:{context:CurrentContext;face:RefObject<HTMLDivElement>;canvas:RefObject<HTMLDivElement>;item:import("../types").Accessory;listError:string}){
  const [size, setSize] = useState<{width:number;height:number;left?:number;top?:number;canvasTop?:number}>({ width: 0, height: 0 });
  useEffect(() => {
    let frame = 0;
    const measure = () => {
      frame = 0;
      const expression = face.current?.querySelector<HTMLElement>(".pet-expression");
      if (!expression || !canvas.current) return;
      const r = expression.getBoundingClientRect(); const c = canvas.current.getBoundingClientRect();
      const warning=(item?.positionX!=null||expression.classList.contains("pet-expression-image"))?canvas.current.querySelector<HTMLElement>(".pet-error"):null;
      const safeTop=warning?Math.max(48,warning.getBoundingClientRect().bottom-c.top+8):44;
      if(item?.positionX!=null&&item.positionY!=null){
        const image=expression.classList.contains("pet-expression-image"); const rw=image?r.width:120,rh=image?r.height:56;
        const side=Math.max(0,Math.min(144,48*(item.scale??1000)/1000,c.width-8,c.height-safeTop-4));
        const x=Math.max(c.left+4+side/2,Math.min(c.right-4-side/2,r.left+r.width/2+item.positionX*rw/1000));
        const y=Math.max(c.top+safeTop+side/2,Math.min(c.bottom-4-side/2,r.top+r.height/2+item.positionY*rh/1000));
        const parent=face.current!.getBoundingClientRect();
        const left=x-parent.left,top=y-parent.top;
        setSize(old=>old.width===side&&old.height===side&&old.left===left&&old.top===top&&old.canvasTop===y-c.top-side/2?old:{width:side,height:side,left,top,canvasTop:y-c.top-side/2});return;
      }
      const font = parseFloat(getComputedStyle(face.current!).fontSize) || 32;
      const width = Math.max(0, Math.min(r.width*(item?.scale??1000)/1000, c.width - 8));
      const height = Math.max(0, Math.min(font * 1.2*(item?.scale??1000)/1000, r.top - c.top - safeTop - 4));
      const canvasTop=r.top-c.top-height-4;
      setSize(old => old.width === width && old.height === height && old.left==null && old.canvasTop===canvasTop ? old : { width, height,canvasTop });
    };
    const schedule = () => { if (!frame) frame = requestAnimationFrame(measure); };
    const observer = new ResizeObserver(schedule);
    if (canvas.current) observer.observe(canvas.current);
    if (face.current) observer.observe(face.current);
    const mutation = new MutationObserver(schedule);
    // 游走只改画布偏移；观察该属性即可随剩余顶部空间缩小，不制造循环布局监听。
    if (canvas.current) mutation.observe(canvas.current, { attributes: true, attributeFilter: ["style"], childList:true });
    window.addEventListener("resize", schedule); document.fonts?.addEventListener("loadingdone", schedule);
    schedule();
    return () => { cancelAnimationFrame(frame); observer.disconnect(); mutation.disconnect(); window.removeEventListener("resize", schedule); document.fonts?.removeEventListener("loadingdone", schedule); };
  }, [face, canvas, context.contextRevision,item]);
  useEffect(()=>{const node=canvas.current;if(!node)return;const property=`--ym-accessory-top-${item.id}`;const sync=()=>{const values=Array.from(node.style).filter(p=>p.startsWith("--ym-accessory-top-")).map(p=>parseFloat(node.style.getPropertyValue(p))).filter(Number.isFinite);const value=values.length?`${Math.min(...values)}px`:"";if(node.style.getPropertyValue("--ym-accessory-top")!==value){if(value)node.style.setProperty("--ym-accessory-top",value);else node.style.removeProperty("--ym-accessory-top");}};if(size.canvasTop!=null)node.style.setProperty(property,`${size.canvasTop}px`);else node.style.removeProperty(property);sync();return()=>{node.style.removeProperty(property);sync();};},[canvas,item.id,size.canvasTop]);
  if (!item || !size.width || !size.height) return null;
  return <div className="pet-accessory-layer" data-positioned={item.positionX!=null} data-pet-interactive data-render-layer={item.renderLayer??"front"} style={{ zIndex:item.renderLayer==="back"?0:2,width: size.width, height: size.height, ...(size.left!=null?{left:size.left,top:size.top,bottom:"auto",transform:"translate(-50%,-50%)"}:{}) }} title={`${item.name}${listError ? ` · ${listError}` : ""}`}>
    <AccessoryImage key={item.id} context={context} accessory={item} compact />
  </div>;
}
