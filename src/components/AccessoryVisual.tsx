import { useEffect, useRef, useState } from "react";
import { accessoriesApi, errorMessage } from "../api";
import type { Accessory, CurrentContext } from "../types";
import "./Accessory.css";

function FitGlyph({ text, name }: { text: string; name: string }) {
  const box = useRef<HTMLDivElement>(null);
  const glyph = useRef<HTMLSpanElement>(null);
  const [scale, setScale] = useState(1);
  useEffect(() => {
    const measure = () => {
      if (!box.current || !glyph.current) return;
      setScale(Math.min(1, box.current.clientWidth / Math.max(1, glyph.current.offsetWidth), box.current.clientHeight / Math.max(1, glyph.current.offsetHeight)));
    };
    const observer = new ResizeObserver(measure);
    if (box.current) observer.observe(box.current);
    if (glyph.current) observer.observe(glyph.current);
    document.fonts?.addEventListener("loadingdone", measure); measure();
    return () => { observer.disconnect(); document.fonts?.removeEventListener("loadingdone", measure); };
  }, [text]);
  return <div ref={box} className="ym-accessory-glyph-fit"><span ref={glyph} className="ym-accessory-glyph" role="img" aria-label={name} style={{ transform: `scale(${scale})` }}>{text}</span></div>;
}

/** 图片外观与始终存在的字符外观分开，破图不能取代实际兜底。 */
export function AccessoryVisual({ name, displayText, dataUrl, notice, compact = false }: {
  name: string; displayText: string; dataUrl: string | null; notice?: string | null; compact?: boolean;
}) {
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  const failed = Boolean(dataUrl && failedUrl === dataUrl);
  return <div className="ym-accessory-visual" data-compact={compact} title={compact ? failed ? "图片无法显示，现使用图标、Emoji、颜文字或符号组合。" : notice ?? undefined : undefined}>
    {dataUrl && !failed ? <img src={dataUrl} alt={name} draggable={false} onError={() => setFailedUrl(dataUrl)} />
      : compact ? <FitGlyph text={displayText} name={name} /> : <span className="ym-accessory-glyph" role="img" aria-label={name}>{displayText}</span>}
    {!compact && (failed || notice) && <p className="ym-page-note">{failed ? "图片无法显示，现使用图标、Emoji、颜文字或符号组合。" : notice}</p>}
  </div>;
}

export default function AccessoryImage({ context, accessory, compact = false }: { context: CurrentContext; accessory: Accessory; compact?: boolean }) {
  const scope = `${context.personaId}:${context.sessionId}:${context.contextRevision}`;
  const [image, setImage] = useState<{ scope: string; id: string; file: string | null; dataUrl: string | null; notice: string | null } | null>(null);
  useEffect(() => {
    let disposed = false;
    if (accessory.imageFile) void accessoriesApi.image(context, accessory.id).then(value => {
      if (!disposed) setImage({ scope, id: accessory.id, file: accessory.imageFile, ...value });
    }).catch(cause => { if (!disposed) setImage({ scope, id: accessory.id, file: accessory.imageFile, dataUrl: null, notice: `图片读取失败：${errorMessage(cause)}。现使用图标、Emoji、颜文字或符号组合。` }); });
    return () => { disposed = true; };
  }, [context.personaId, context.sessionId, context.contextRevision, accessory]);
  const current = image?.scope === scope && image.id === accessory.id && image.file === accessory.imageFile ? image : null;
  return <AccessoryVisual key={`${context.contextRevision}:${accessory.id}:${accessory.imageFile}`} name={accessory.name}
    displayText={accessory.displayText} dataUrl={current?.dataUrl ?? null} notice={current?.notice} compact={compact} />;
}
