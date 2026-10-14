import { useEffect, useRef, useState, type RefObject } from "react";
import { useAccessories } from "../hooks/useAccessories";
import type { CurrentContext } from "../types";
import AccessoryImage from "./AccessoryVisual";

/** 绝对定位在颜文字上方；不把饰品宽度加入颜文字排版。 */
export default function AccessoryLayer({ context, face, canvas }: { context: CurrentContext; face: RefObject<HTMLDivElement>; canvas: RefObject<HTMLDivElement> }) {
  const list = useAccessories(context, true);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const layer = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let frame = 0;
    const measure = () => {
      frame = 0;
      const expression = face.current?.querySelector<HTMLElement>(".pet-expression");
      if (!expression || !canvas.current) return;
      const r = expression.getBoundingClientRect(); const c = canvas.current.getBoundingClientRect();
      const font = parseFloat(getComputedStyle(face.current!).fontSize) || 32;
      const width = Math.max(0, Math.min(r.width, c.width - 8));
      const height = Math.max(0, Math.min(font * 1.2, r.top - c.top - 44 - 4));
      setSize(old => old.width === width && old.height === height ? old : { width, height });
    };
    const schedule = () => { if (!frame) frame = requestAnimationFrame(measure); };
    const observer = new ResizeObserver(schedule);
    if (canvas.current) observer.observe(canvas.current);
    if (face.current) observer.observe(face.current);
    const mutation = new MutationObserver(schedule);
    // 游走只改画布偏移；观察该属性即可随剩余顶部空间缩小，不制造循环布局监听。
    if (canvas.current) mutation.observe(canvas.current, { attributes: true, attributeFilter: ["style"] });
    window.addEventListener("resize", schedule); document.fonts?.addEventListener("loadingdone", schedule);
    schedule();
    return () => { cancelAnimationFrame(frame); observer.disconnect(); mutation.disconnect(); window.removeEventListener("resize", schedule); document.fonts?.removeEventListener("loadingdone", schedule); };
  }, [face, canvas, context.contextRevision]);
  const item = list.items[0];
  if (!item || !size.width || !size.height) return null;
  return <div ref={layer} className="pet-accessory-layer" data-pet-interactive style={{ width: size.width, height: size.height }} title={`${item.name}${list.error ? ` · ${list.error}` : ""}`}>
    <AccessoryImage key={item.id} context={context} accessory={item} compact />
  </div>;
}
