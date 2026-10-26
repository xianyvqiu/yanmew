import { useEffect, useRef, useState, type CSSProperties } from "react";
import { petCreationApi } from "../api";
import {readArtworkGeometry,type ArtworkGeometry} from "./petArtworkGeometry";
import type { PetArtwork, PetCreation } from "../types";

/** 图片读取失败只在当前库版本尝试一次；旧人设和旧读取结果不能覆盖新状态。 */
export default function PetArtworkDisplay({ personaId, expression, dock = "none", onImageMode }: { personaId: string; expression: string; dock?: string; onImageMode?: (enabled: boolean) => void }) {
  const [snapshot, setSnapshot] = useState<{ personaId: string; library: PetCreation } | null>(null);
  type Picture = {personaId:string;key:string;url:string;name:string;emotion:string;geometry:ArtworkGeometry|null};
  const [source, setSource] = useState<Picture | null>(null);
  const [outgoing,setOutgoing]=useState<Picture|null>(null);
  const shown=useRef<Picture|null>(null);
  const cache=useRef(new Map<string,{url:string;geometry:ArtworkGeometry|null}>());
  const expressionRef=useRef<HTMLSpanElement>(null);
  const fadeTimer=useRef<ReturnType<typeof setTimeout>>();
  const [failed, setFailed] = useState<Set<string>>(() => new Set());
  useEffect(() => {
    let alive = true;
    let generation = 0;
    let revision = -1;
    let stop: (() => void) | undefined;
    setSnapshot(null);
    setFailed(new Set());cache.current.clear();shown.current=null;setSource(null);setOutgoing(null);
    const refresh = () => {
      if (!personaId) return;
      const request = ++generation;
      void petCreationApi.get(personaId).then(library => {
        if (!alive || request !== generation || library.revision < revision) return;
        revision = library.revision;
        setSnapshot({ personaId, library });
      }).catch(() => { if (alive && request === generation) setSnapshot(null); });
    };
    // 先订阅再补查，避免第一次读取与事件监听之间丢失开关变化。
    void petCreationApi.listen(refresh).then(unlisten => {
      if (!alive) { unlisten(); return; }
      stop = unlisten;
      refresh();
    }).catch(() => { if (alive) refresh(); });
    return () => { alive = false; stop?.(); };
  }, [personaId]);
  const library = snapshot?.personaId === personaId ? snapshot.library : null;
  useEffect(() => { if (library) onImageMode?.(library.enabled); }, [library?.enabled, onImageMode]);
  const imageMode = expression.startsWith("@pet-image:") && (library?.enabled ?? true);
  const id = imageMode ? expression.slice(11) : null;
  const imageKey = (item: PetArtwork) => `${personaId}:${library?.revision}:${item.id}:${item.imageFile}`;
  const base = library?.items.find(item => item.kind === "base" && item.imageFile);
  const requested = library?.items.find(item => item.id === id && item.imageFile && (item.enabled || item.kind === "base"));
  const item = requested && !failed.has(imageKey(requested)) ? requested : base && !failed.has(imageKey(base)) ? base : undefined;
  const key = item ? imageKey(item) : "";
  useEffect(() => {
    let alive = true;
    if (imageMode && item) {
      const load=async()=>{
        const cached=cache.current.get(key);
        const url=cached?.url??await petCreationApi.image(personaId,item.id);
        const decoded=new Image();decoded.src=url;
        if(typeof decoded.decode==="function")await decoded.decode();
        const geometry=cached?.geometry??readArtworkGeometry(decoded);
        if(!alive)return;
        cache.current.set(key,{url,geometry});while(cache.current.size>48){cache.current.delete(cache.current.keys().next().value!);}
        const next={personaId,key,url,geometry,name:item.name,emotion:/开心|喜悦|高兴/.test(item.name)?"happy":/生气|愤怒/.test(item.name)?"angry":""};
        clearTimeout(fadeTimer.current);
        setOutgoing(shown.current?.personaId===personaId&&shown.current.key!==key?shown.current:null);
        shown.current=next;setSource(next);
        fadeTimer.current=setTimeout(()=>setOutgoing(null),180);
      };
      void load().catch(()=>{if(alive)setFailed(previous=>new Set(previous).add(key));});
    }
    return () => { alive = false; };
  }, [key, imageMode]);
  const display=imageMode&&item&&source?.personaId===personaId&&!failed.has(source.key)?source:null;
  useEffect(()=>()=>clearTimeout(fadeTimer.current),[]);
  useEffect(()=>{
    if(!display?.emotion||window.matchMedia?.("(prefers-reduced-motion: reduce)").matches)return;
    const frames=display.emotion==="happy"?[{translate:"0 0"},{translate:"0 -8px",offset:.45},{translate:"0 0"}]:[{rotate:"0deg"},{rotate:"-2.5deg"},{rotate:"2.5deg"},{rotate:"-1.5deg"},{rotate:"0deg"}];
    const animation=expressionRef.current?.animate?.(frames,{duration:display.emotion==="happy"?460:420,easing:"ease-in-out"});
    return()=>animation?.cancel();
  },[display?.key]);
  useEffect(()=>{let alive=true;if(library?.enabled)for(const pose of library.items.filter(a=>["lifted","toss_up","toss_down","landing","rest_sit","rest_lie","wait_pet"].includes(a.category)&&a.enabled&&a.imageFile)){const poseKey=imageKey(pose);if(!cache.current.has(poseKey))void petCreationApi.image(personaId,pose.id).then(async url=>{const image=new Image();image.src=url;if(typeof image.decode==="function")await image.decode();if(alive)cache.current.set(poseKey,{url,geometry:readArtworkGeometry(image)});}).catch(()=>{});}return()=>{alive=false;};},[personaId,library?.revision]);
  const alignFloor = dock === "rest_sit" || dock === "rest_lie";
  const measure = (image:HTMLImageElement) => {
    if(!display||display.geometry)return;
    const geometry=readArtworkGeometry(image);if(!geometry)return;
    cache.current.set(display.key,{url:display.url,geometry});
    const next={...display,geometry};if(shown.current?.key===next.key)shown.current=next;
    setSource(previous=>previous?.key===next.key?next:previous);
  };
  const geometryStyle=(geometry:ArtworkGeometry|null):CSSProperties=>({
    "--ym-art-top-ratio":alignFloor&&geometry?geometry.top:0,
    "--ym-art-bottom-ratio":alignFloor&&geometry?geometry.bottom:0,
    "--ym-art-width-ratio":alignFloor&&geometry?geometry.width:1,
    "--ym-art-height-ratio":alignFloor&&geometry?geometry.height:1,
    "--ym-art-visible-ratio":alignFloor&&geometry?geometry.visible:1,
  } as CSSProperties);
  return <span ref={expressionRef} className={`pet-expression${imageMode ? " pet-expression-image" : ""}`} style={geometryStyle(display?.geometry??null)}>
    {display ? <>
      {outgoing?.personaId===personaId&&outgoing.key!==display.key&&<img className="pet-image-outgoing" style={geometryStyle(outgoing.geometry)} src={outgoing.url} alt="" aria-hidden draggable={false}/>}
      <img key={display.key} className="pet-image-current" style={geometryStyle(display.geometry)} src={display.url} alt={display.name||"桌宠"} draggable={false} onError={()=>setFailed(previous=>new Set(previous).add(display.key))} onLoad={e=>measure(e.currentTarget)}/>
    </> : expression.startsWith("@pet-image:") ? "(・ω・)" : expression}
  </span>;
}
