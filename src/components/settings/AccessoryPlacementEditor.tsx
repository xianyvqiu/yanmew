import { useEffect, useRef, useState } from "react";
import { petCreationApi, saveAccessoryPlacement, errorMessage } from "../../api";
import type { Accessory, CurrentContext } from "../../types";
import AccessoryImage from "../AccessoryVisual";
import PetArtworkDisplay from "../PetArtworkDisplay";
import "./AccessoryPlacementEditor.css";
export default function AccessoryPlacementEditor({ context, item, onClose, onSaved }: {
    context: CurrentContext;
    item: Accessory;
    onClose: () => void;
    onSaved: () => void;
}) {
    const [image, setImage] = useState(false);
    const [expression, setExpression] = useState("(・ω・)");
    const [x, setX] = useState(item.positionX ?? 0);
    const [y, setY] = useState(item.positionY ?? -750);
    const [scale, setScale] = useState(item.scale ?? 1000);
    const [defaults, setDefaults] = useState(item.positionX == null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState("");
    const alive = useRef(true);
    const drag = useRef<{
        id: number;
        x: number;
        y: number;
        startX: number;
        startY: number;
    }>();
    const area = useRef<HTMLDivElement>(null);
    useEffect(() => { alive.current = true; void petCreationApi.get(context.personaId).then(l => { if (!alive.current)
        return; const base = l.items.find(a => a.kind === "base" && a.imageFile); if (l.enabled && base) {
        setImage(true);
        setExpression(`@pet-image:${base.id}`);
    } }).catch(() => { }); return () => { alive.current = false; }; }, [context.personaId]);
    const width = image ? 96 : 120, height = image ? 96 : 56;
    const size = 48 * scale / 1000;
    const position = defaults ? { left: "50%", top: `calc(50% - ${height / 2 + size / 2}px)` } : { left: `calc(50% + ${x * width / 1000}px)`, top: `calc(50% + ${y * height / 1000}px)` };
    return <section className="ym-page-card ym-placement-editor" aria-label="调整佩戴位置"><h3>调整 {item.name} 的佩戴位置</h3><p className="ym-page-note">拖动饰品调整位置，改变大小后点击保存。位置跟随桌宠，不绑定屏幕；所有表情共用此位置。</p>
 <div ref={area} className="ym-placement-canvas"><div className="ym-placement-body" style={{ width, height }}><PetArtworkDisplay personaId={context.personaId} expression={expression}/></div>
 <div className="ym-placement-accessory" style={{ ...position, width: size, height: size }} role="slider" aria-label="饰品位置" aria-valuetext={`横向${x}，纵向${y}`} tabIndex={0} onKeyDown={e => { if (busy)
        return; const changes: Record<string, [
        number,
        number
    ]> = { ArrowLeft: [-25, 0], ArrowRight: [25, 0], ArrowUp: [0, -25], ArrowDown: [0, 25] }; const step = changes[e.key]; if (step) {
        e.preventDefault();
        setDefaults(false);
        setX(Math.max(-1500, Math.min(1500, (defaults ? 0 : x) + step[0])));
        const startY = defaults ? Math.round(-(height / 2 + size / 2) / height * 1000) : y;
        setY(Math.max(-1500, Math.min(1500, startY + step[1])));
    } }} onPointerDown={e => { if (busy)
        return; e.preventDefault(); e.currentTarget.setPointerCapture(e.pointerId); drag.current = { id: e.pointerId, x: e.clientX, y: e.clientY, startX: defaults ? 0 : x, startY: defaults ? Math.round(-(height / 2 + size / 2) / height * 1000) : y }; }} onPointerMove={e => { const d = drag.current; if (!d || d.id !== e.pointerId)
        return; const rect = area.current!.getBoundingClientRect(); const limitX = Math.min(1500, Math.max(0, (rect.width - size) / 2 / width * 1000)), limitY = Math.min(1500, Math.max(0, (rect.height - size) / 2 / height * 1000)); setDefaults(false); setX(Math.round(Math.max(-limitX, Math.min(limitX, d.startX + (e.clientX - d.x) / width * 1000)))); setY(Math.round(Math.max(-limitY, Math.min(limitY, d.startY + (e.clientY - d.y) / height * 1000)))); }} onPointerUp={() => { drag.current = undefined; }} onPointerCancel={() => { drag.current = undefined; }} onLostPointerCapture={() => { drag.current = undefined; }}><AccessoryImage context={context} accessory={item} compact/></div></div>
 <label className="ym-field">大小：{Math.round(scale / 10)}%<input type="range" min={200} max={3000} step={50} value={scale} disabled={busy} onChange={e => setScale(Number(e.target.value))}/></label>
 {error && <p role="alert" className="ym-page-error">{error}</p>}<div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => { setDefaults(true); setX(0); setY(-750); setScale(1000); }}>恢复默认位置与大小</button><button className="ym-button" disabled={busy} onClick={onClose}>取消</button><button className="ym-button" disabled={busy} onClick={() => { setBusy(true); setError(""); void saveAccessoryPlacement(context, item.id, defaults ? null : x, defaults ? null : y, scale).then(() => { if (alive.current)
        onSaved(); }).catch(e => { if (alive.current)
        setError(errorMessage(e)); }).finally(() => { if (alive.current)
        setBusy(false); }); }}>保存位置</button></div></section>;
}
