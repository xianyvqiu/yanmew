import AccessoryUpload from "./AccessoryUpload";
import AccessoryPlacementEditor from "./AccessoryPlacementEditor";
import CardBrowserControls, {CardBrowserPagination} from "../CardBrowserControls";
import {useRemoteCardBrowser} from "../../hooks/useRemoteCardBrowser";
import { useEffect, useRef, useState } from "react";
import { accessoriesApi, errorMessage } from "../../api";
import type { Accessory, CurrentContext } from "../../types";
import AccessoryImage from "../AccessoryVisual";
import AccessoryCraftCard from "../AccessoryCraftCard";

export default function AccessoryPanel({ context }: { context: CurrentContext }) {
  const browser=useRemoteCardBrowser<Accessory>("accessories",context);
  const [selected,setSelected]=useState<Accessory|null>(null);
  const [placement,setPlacement]=useState<Accessory|null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [confirmation, setConfirmation] = useState<{ item: Accessory; sell: boolean } | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const placementPanel = useRef<HTMLDivElement>(null);
  useEffect(()=>{if(placement){const frame=requestAnimationFrame(()=>placementPanel.current?.scrollIntoView?.({block:"start",behavior:window.matchMedia("(prefers-reduced-motion: reduce)").matches?"auto":"smooth"}));return()=>cancelAnimationFrame(frame);}},[placement]);
  const locked = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => { if (confirmation) dialog.current?.showModal(); else dialog.current?.close(); }, [confirmation]);
  useEffect(() => {
    setSelected(previous => previous ? browser.items.find(item => item.id === previous.id) ?? previous : null);
  }, [browser.items]);
  const act = async (operation: () => Promise<unknown>, success: string, removeSelection = false) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(""); setNotice("");
    try {
      await operation();
      if (alive.current) { setConfirmation(null); if (removeSelection) setSelected(null); setNotice(success); browser.refresh(); }
    } catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  if (!context.sessionId) return <section className="ym-page-card"><h2>饰品</h2><p>当前没有会话，请先在聊天窗口创建会话。</p></section>;
  return <div className="ym-accessory-panel">
    <AccessoryCraftCard context={context} />
    <AccessoryUpload context={context}/>
    <section className="ym-page-card" aria-label="当前会话饰品">
      <div className="ym-page-heading"><h2>饰品</h2><button className="ym-button" disabled={busy || browser.loading} onClick={() => { browser.refresh(); }}>刷新饰品</button></div>
      <p className="ym-page-note">饰品随当前会话保存，可同时佩戴多件，前后位置与层级在佩戴编辑器调整。卖出未佩戴饰品固定获得 10 喵币；制作资格不会恢复。</p>
      {browser.loading && <p role="status">正在读取饰品…</p>}
      {(error || browser.error) && <p role="alert" className="ym-page-error">{error || browser.error}</p>}
      {notice && <p role="status">{notice}</p>}
      {!browser.loading && !browser.total && !browser.search && !browser.error && <p className="ym-page-empty">当前会话还没有饰品。聊天中符合制作资格时，它可以提出制作请求。</p>}
      {!selected&&<CardBrowserControls browser={browser} label="饰品" timeLabel="获得时间" disabled={busy}/>}
      {selected&&<button className="ym-button" disabled={busy} onClick={()=>setSelected(null)}>返回饰品列表</button>}
      <ul className="ym-accessory-list ym-object-grid">{(selected?[selected]:browser.items).map(item => <li className="ym-object-card" key={item.id} onClick={event=>{if(!selected&&!busy&&!(event.target as HTMLElement).closest("button"))setSelected(item);}}>
        <AccessoryImage context={context} accessory={item} />
        <div className="ym-accessory-detail"><div className="ym-page-heading"><h3><button className="ym-card-open" disabled={busy} onClick={()=>setSelected(item)}>{item.name}</button></h3><span className="ym-page-note">{item.isWorn ? "已佩戴" : "未佩戴"}</span></div>
          {!selected&&<p className="ym-card-summary">{item.description}</p>}
          <p className="ym-page-note">获得：{new Date(item.createdAt).toLocaleString()}</p>
          {selected&&<>
          <p className="ym-accessory-description">{item.description}</p>
          <p className="ym-page-note">来源：{item.source === "crafted" ? "制作" : "旅行带回"} · 制作花费：{item.cost} 喵币</p>
          <p className="ym-page-note">图标 / Emoji / 颜文字兜底：<span className="ym-accessory-fallback">{item.displayText}</span></p>
          <div className="ym-inline-form"><button className="ym-button" disabled={busy || browser.loading} onClick={() => { void act(() => item.isWorn ? accessoriesApi.unwear(context, item.id) : accessoriesApi.wear(context, item.id), item.isWorn ? "已卸载饰品。" : "已佩戴饰品。"); }}>{item.isWorn ? "卸载" : "佩戴"}</button>
            <button className="ym-button" disabled={busy} onClick={()=>setPlacement(item)}>调整佩戴位置</button>
            <button className="ym-button" disabled={busy || browser.loading || item.isWorn} onClick={() => { setError(""); setConfirmation({ item, sell: true }); }}>卖出 · 10 喵币</button>
            <button className="ym-button" data-variant="danger" disabled={busy || browser.loading || item.isWorn} onClick={() => { setError(""); setConfirmation({ item, sell: false }); }}>删除</button></div>
          </>}
        </div></li>)}</ul>
      {!selected&&<CardBrowserPagination browser={browser} label="饰品" disabled={busy}/>}

    </section>
    {placement&&<div ref={placementPanel}><AccessoryPlacementEditor context={context} item={placement} onClose={()=>setPlacement(null)} onSaved={()=>{setPlacement(null);browser.refresh();}}/></div>}
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="accessory-remove-title" onCancel={event => { if (busy) event.preventDefault(); else setConfirmation(null); }}>
      <h2 id="accessory-remove-title">{confirmation?.sell ? "卖出这件饰品？" : "删除这件饰品？"}</h2>
      <div className="ym-session-dialog-body"><p>{confirmation?.item.name}：{confirmation?.sell ? "卖出后获得固定 10 喵币" : "删除不返还喵币"}，操作不可恢复。制作档位资格不会恢复，其他饰品不受影响。</p>{error && <p role="alert" className="ym-page-error">{error}</p>}</div>
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setConfirmation(null)}>取消</button>
        <button className="ym-button" data-variant="danger" disabled={busy || !confirmation} onClick={() => {
          if (!confirmation) return;
          const target = confirmation;
          void act(async () => {
            if (target.sell) { await accessoriesApi.sell(context, target.item.id); }
            else await accessoriesApi.delete(context, target.item.id);
          }, target.sell ? "已卖出，获得 10 喵币。" : "已删除饰品。", true);
        }}>{busy ? "正在处理…" : confirmation?.sell ? "确认卖出" : "确认删除"}</button></div>
    </dialog>
  </div>;
}
