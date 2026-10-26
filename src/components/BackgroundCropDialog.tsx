import { useEffect, useRef, useState } from "react";
import {type BackgroundViewport,readBackgroundViewport} from "./chatBackgroundViewport";

export default function BackgroundCropDialog({ source, viewport=readBackgroundViewport(), onCancel, onSave }: {
  source: string; viewport?:BackgroundViewport; onCancel: () => void; onSave: (base64: string) => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null), frame = useRef<HTMLDivElement>(null), picture = useRef<HTMLImageElement>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [natural, setNatural] = useState({ width: 1, height: 1 });
  const [zoom, setZoom] = useState(1), [pan, setPan] = useState({ x: 0, y: 0 });
  const [ready, setReady] = useState(false), [error, setError] = useState("");
  const drag = useRef<{ id: number; x: number; y: number; pan: typeof pan }>();
  useEffect(() => {
    if(dialog.current?.showModal)dialog.current.showModal();else dialog.current?.setAttribute("open","");
    const element = frame.current;
    if (!element) return;
    const measure = () => { const rect=element.getBoundingClientRect();setSize({ width:rect.width||element.clientWidth,height:rect.height||element.clientHeight }); };
    measure(); const observer = new ResizeObserver(measure); observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const base = Math.max(size.width / natural.width, size.height / natural.height);
  const bound = (x: number, y: number, scale = zoom) => ({
    x: Math.max(-(natural.width * base * scale - size.width) / 2, Math.min((natural.width * base * scale - size.width) / 2, x)),
    y: Math.max(-(natural.height * base * scale - size.height) / 2, Math.min((natural.height * base * scale - size.height) / 2, y)),
  });
  const save = () => {
    if (!picture.current || !ready || !base) return;
    try {
      const canvas = document.createElement("canvas");
      const outputScale=Math.min(2,1600/Math.max(viewport.width,viewport.height));
      canvas.width=Math.max(1,Math.round(viewport.width*outputScale));canvas.height=Math.max(1,Math.round(viewport.height*outputScale));
      const context = canvas.getContext("2d"); if (!context) throw new Error("当前窗口无法裁剪图片");
      const scale = base * zoom, position = bound(pan.x, pan.y);
      const width = size.width / scale, height = size.height / scale;
      context.drawImage(picture.current, (natural.width - width) / 2 - position.x / scale,
        (natural.height - height) / 2 - position.y / scale, width, height, 0, 0, canvas.width, canvas.height);
      onSave(canvas.toDataURL("image/png").split(",")[1]);
    } catch { setError("图片裁剪失败，请重新选择图片。"); }
  };
  return <dialog ref={dialog} className="ym-session-dialog ym-crop-dialog" aria-label="裁剪聊天背景" onCancel={event => { event.preventDefault(); onCancel(); }}>
    <div className="ym-page-heading"><h2>裁剪聊天背景</h2><button type="button" className="ym-button" onClick={onCancel}>取消</button></div>
    <p className="ym-page-note">拖动调整取景，缩放查看细节。取景框按聊天背景区域 {Math.round(viewport.width)} × {Math.round(viewport.height)} 的比例显示，与外观预览一致。</p>
    <div className="ym-crop-stage"><div ref={frame} className="ym-crop-frame" style={{aspectRatio:`${viewport.width} / ${viewport.height}`,width:`min(100%, ${46*viewport.width/viewport.height}vh)`}} onPointerDown={event => {
      event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId);
      drag.current = { id: event.pointerId, x: event.clientX, y: event.clientY, pan };
    }} onPointerMove={event => {
      const start = drag.current; if (!start || start.id !== event.pointerId) return;
      setPan(bound(start.pan.x + event.clientX - start.x, start.pan.y + event.clientY - start.y));
    }} onPointerUp={() => { drag.current = undefined; }} onPointerCancel={() => { drag.current = undefined; }} onLostPointerCapture={() => { drag.current = undefined; }}>
      <img ref={picture} src={source} alt="待裁剪背景" draggable={false} onLoad={event => { setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight }); setReady(true); }} onError={() => setError("图片无法解码，请换一张图片。")}
        style={{ position:"absolute",left:"50%",top:"50%",width: natural.width * base, height: natural.height * base, transform: `translate(calc(-50% + ${bound(pan.x, pan.y).x}px),calc(-50% + ${bound(pan.x, pan.y).y}px)) scale(${zoom})` }} />
      <span aria-hidden="true" className="ym-crop-grid" />
    </div></div>
    <label className="ym-field">缩放：{Math.round(zoom * 100)}%<input aria-label="背景裁剪缩放" type="range" min="1" max="4" step="0.01" value={zoom} onChange={event => { const next = Number(event.target.value); setZoom(next); setPan(bound(pan.x, pan.y, next)); }} /></label>
    <div className="ym-inline-form"><button type="button" className="ym-button" onClick={() => { setZoom(1); setPan({ x: 0, y: 0 }); }}>重置构图</button><button type="button" className="ym-button" data-variant="primary" disabled={!ready || !!error} onClick={save}>保存裁剪</button></div>
    <p className="ym-page-note">保存裁剪更新预览；点击“保存外观”后应用到该人设的聊天背景。</p>{error && <p role="alert" className="ym-page-error">{error}</p>}
  </dialog>;
}
