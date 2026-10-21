import { useEffect, useRef, useState, type CSSProperties } from "react";
import { petCreationApi } from "../api";
import type { PetArtwork, PetCreation } from "../types";

/** 图片读取失败只在当前库版本尝试一次；旧人设和旧读取结果不能覆盖新状态。 */
export default function PetArtworkDisplay({ personaId, expression, dock = "none", onImageMode }: { personaId: string; expression: string; dock?: string; onImageMode?: (enabled: boolean) => void }) {
  const [snapshot, setSnapshot] = useState<{ personaId: string; library: PetCreation } | null>(null);
  type Picture = {personaId:string;key:string;url:string;name:string;emotion:string};
  const [source, setSource] = useState<Picture | null>(null);
  const [outgoing,setOutgoing]=useState<Picture|null>(null);
  const shown=useRef<Picture|null>(null);
  const cache=useRef(new Map<string,string>());
  const expressionRef=useRef<HTMLSpanElement>(null);
  const fadeTimer=useRef<ReturnType<typeof setTimeout>>();
  const [failed, setFailed] = useState<Set<string>>(() => new Set());
  const [bounds, setBounds] = useState<{ key: string; top: number; width: number; height: number; visible: number } | null>(null);
  const imageRef = useRef<HTMLImageElement>(null);
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
        const url=cache.current.get(key)??await petCreationApi.image(personaId,item.id);
        const decoded=new Image();decoded.src=url;
        if(typeof decoded.decode==="function")await decoded.decode();
        if(!alive)return;
        cache.current.set(key,url);while(cache.current.size>48){cache.current.delete(cache.current.keys().next().value!);}
        const next={personaId,key,url,name:item.name,emotion:/开心|喜悦|高兴/.test(item.name)?"happy":/生气|愤怒/.test(item.name)?"angry":""};
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
  const visualKey=display?.key??key;
  useEffect(()=>()=>clearTimeout(fadeTimer.current),[]);
  useEffect(()=>{
    if(!display?.emotion||window.matchMedia?.("(prefers-reduced-motion: reduce)").matches)return;
    const frames=display.emotion==="happy"?[{translate:"0 0"},{translate:"0 -8px",offset:.45},{translate:"0 0"}]:[{rotate:"0deg"},{rotate:"-2.5deg"},{rotate:"2.5deg"},{rotate:"-1.5deg"},{rotate:"0deg"}];
    const animation=expressionRef.current?.animate?.(frames,{duration:display.emotion==="happy"?460:420,easing:"ease-in-out"});
    return()=>animation?.cancel();
  },[display?.key]);
  useEffect(()=>{let alive=true;const pose=library?.items.find(a=>a.category==="lifted"&&a.enabled&&a.imageFile);if(library?.enabled&&pose){const poseKey=imageKey(pose);if(!cache.current.has(poseKey))void petCreationApi.image(personaId,pose.id).then(async url=>{const image=new Image();image.src=url;if(typeof image.decode==="function")await image.decode();if(alive)cache.current.set(poseKey,url);}).catch(()=>{});}return()=>{alive=false;};},[personaId,library?.revision]);
  const alignFloor = dock === "rest_sit" || dock === "rest_lie";
  const measure = (image: HTMLImageElement) => {
    if (!alignFloor || !image.naturalWidth || !image.naturalHeight) return;
    try {
      const canvas = document.createElement("canvas"); canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
      const context = canvas.getContext("2d"); if (!context) return;
      context.drawImage(image, 0, 0); const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
      let bottom = canvas.height - 1;
      outer: for (; bottom >= 0; bottom--) for (let x = 0; x < canvas.width; x++) if (pixels[(bottom * canvas.width + x) * 4 + 3] > 8) break outer;
      if (bottom >= 0) {
        let top = 0;
        outer: for (; top < bottom; top++) for (let x = 0; x < canvas.width; x++) if (pixels[(top * canvas.width + x) * 4 + 3] > 8) break outer;
        const longest = Math.max(canvas.width,canvas.height);
        setBounds({ key:visualKey, top: -top / longest, width: canvas.width / longest, height: canvas.height / longest, visible: (bottom-top+1) / longest });
      }
    } catch { /* 受管透明图读取失败时仍保留原图展示。 */ }
  };
  useEffect(() => { if (alignFloor && imageRef.current?.complete) measure(imageRef.current); }, [alignFloor, visualKey, display?.url]);
  return <span ref={expressionRef} className={`pet-expression${imageMode ? " pet-expression-image" : ""}`} style={{ "--ym-art-top-ratio": alignFloor && bounds?.key === visualKey ? bounds.top : 0, "--ym-art-width-ratio": alignFloor && bounds?.key === visualKey ? bounds.width : 1, "--ym-art-height-ratio": alignFloor && bounds?.key === visualKey ? bounds.height : 1, "--ym-art-visible-ratio": alignFloor && bounds?.key === visualKey ? bounds.visible : 1 } as CSSProperties}>
    {display ? <>
      {outgoing?.personaId===personaId&&outgoing.key!==display.key&&<img className="pet-image-outgoing" src={outgoing.url} alt="" aria-hidden draggable={false}/>}
      <img key={display.key} className={outgoing?"pet-image-current pet-image-enter":"pet-image-current"} ref={imageRef} src={display.url} alt={display.name||"桌宠"} draggable={false} onError={()=>setFailed(previous=>new Set(previous).add(display.key))} onLoad={e=>measure(e.currentTarget)}/>
    </> : expression.startsWith("@pet-image:") ? "(・ω・)" : expression}
  </span>;
}
