import { useAccessoryCraft } from "../hooks/useAccessoryCraft";
import type { CurrentContext } from "../types";
import { AccessoryVisual } from "./AccessoryVisual";

export default function AccessoryCraftCard({ context, pet = false }: { context: CurrentContext; pet?: boolean }) {
  const craft = useAccessoryCraft(context);
  return <AccessoryCraftPresentation context={context} craft={craft} pet={pet} />;
}

export function AccessoryCraftPresentation({ context, craft, pet = false }: { context: CurrentContext; craft: ReturnType<typeof useAccessoryCraft>; pet?: boolean }) {
  const preview = craft.preview;
  const enough = preview && craft.wallet ? BigInt(craft.wallet.coins) >= BigInt(preview.cost) : false;
  if (!context.sessionId) return null;
  if (!preview && !craft.error && (pet || !craft.notice)) return null;
  return <section className={`ym-page-card ym-accessory-craft${pet ? " ym-accessory-craft-pet" : ""}`} aria-label="饰品制作" data-pet-interactive={pet || undefined}>
    {preview ? <>
      <header className="ym-accessory-craft__header"><h2>想做一件{preview.name}</h2>
      <p className="ym-page-note">当前会话 · 第 {preview.tier} 档 · {preview.cost} 喵币</p></header>
      <div className="ym-accessory-craft__body"><AccessoryVisual key={preview.previewId} name={preview.name} displayText={preview.displayText} dataUrl={preview.imageDataUrl} notice={preview.notice} />
      <p className="ym-accessory-description">{preview.description}</p>
      {preview.status === "preparing" && <p role="status">正在准备图片，可以取消；就绪后再确认购买。</p>}
      <p className="ym-page-note">余额：{craft.wallet?.coins ?? "正在读取"} 喵币。{preview.uploaded?"手动上传不产生生图 API 费用；确认后才扣喵币。":"生图服务可能已产生 API 费用，与喵币无关；取消或不购买不保证退还 API 费用。"}</p>
      {!enough && craft.wallet && <p className="ym-page-note">当前喵币不足。</p>}</div>
      <div className="ym-inline-form ym-accessory-craft__actions"><button className="ym-button" disabled={craft.busy} onClick={() => { void craft.cancel(preview.previewId); }}>没有钱了</button>
        <button className="ym-button" data-variant="primary" disabled={craft.busy || preview.status !== "ready" || !enough} onClick={() => { void craft.confirm(preview.previewId); }}>{craft.busy ? "正在处理…" : "给你买"}</button></div>
    </> : <p role="status">{craft.notice || "本次制作未完成，请查看提示或重新读取。"}</p>}
    {craft.error && <p role="alert" className="ym-page-error">{craft.error}<button className="ym-button" disabled={craft.busy} onClick={() => { void craft.refresh(); }}>重新读取</button></p>}
  </section>;
}
