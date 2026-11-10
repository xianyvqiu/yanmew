import SouvenirImage from "./SouvenirImage";
import SouvenirImageSettings from "./SouvenirImageSettings";
import TravelFilter, {TravelOriginLabel} from "./TravelFilter";
import InlineDetailCard from "./InlineDetailCard";
import UiIcon from "./UiIcon";
import CardBrowserControls, {CardBrowserPagination} from "./CardBrowserControls";
import {useRemoteCardBrowser} from "../hooks/useRemoteCardBrowser";
import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, souvenirsApi, travelApi } from "../api";
import type { CurrentContext, GiftGenerationStatus, Souvenir } from "../types";
import { travelTime } from "./travelTime";

export default function SouvenirPanel({ context }: { context: CurrentContext }) {
  return <SouvenirPanelForContext key={`${context.personaId}:${context.sessionId}:${context.contextRevision}`} context={context} />;
}
function SouvenirPanelForContext({ context }: { context: CurrentContext }) {
  const browser=useRemoteCardBrowser<Souvenir>("souvenirs",context);
  const [status, setStatus] = useState<GiftGenerationStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(()=>{setSelected(null);},[browser.selectionKey]);
  const [detail, setDetail] = useState<Souvenir | null>(null);
  const [deleting, setDeleting] = useState<Souvenir | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const read = useRef<() => Promise<void>>(async () => {});
  const [detailVersion, setDetailVersion] = useState(0);
  const locked = useRef(false);
  const alive = useRef(false);
  const refresh = useCallback(() => { browser.refresh(); return read.current(); }, [browser.refresh]);
  useEffect(() => {
    alive.current = true; let disposed = false; let generation = 0;
    const stops: (() => void)[] = [];
    const load = async () => {
      if (disposed || !context.sessionId) { if (!disposed) setLoading(false); return; }
      const token = ++generation; setLoading(true);
      try {
        const progress = await souvenirsApi.status(context);
        if (disposed || token !== generation) return;
        setStatus(progress); setDetailVersion(value => value + 1); setError("");
      } catch (cause) { if (!disposed && token === generation) setError(errorMessage(cause)); }
      finally { if (!disposed && token === generation) setLoading(false); }
    };
    read.current = load;
    const focus = () => { void load(); };
    void travelApi.listen(change => { if (change.sessionId === context.sessionId) void load(); }).then(stop => {
      if (disposed) { stop(); return; } stops.push(stop); window.addEventListener("focus", focus); void load();
    }).catch(cause => { if (!disposed) { setError(`通知暂不可用：${errorMessage(cause)}`); window.addEventListener("focus", focus); void load(); } });
    return () => { disposed = true; alive.current = false; generation++; stops.forEach(stop => stop()); window.removeEventListener("focus", focus); };
  }, [context.personaId, context.sessionId, context.contextRevision]);
  useEffect(() => {
    let disposed = false;
    if (!selected) { setDetail(null); return; }
    if (selected) void souvenirsApi.get(context, selected).then(item => { if (!disposed) setDetail(item); })
      .catch(cause => { if (!disposed) setError(errorMessage(cause)); });
    return () => { disposed = true; };
  }, [selected, detailVersion]);
  useEffect(() => { if (deleting) dialog.current?.showModal(); else dialog.current?.close(); }, [deleting]);
  const act = async (operation: () => Promise<unknown>, after?: () => void, reload = true) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try { await operation(); if (alive.current) { after?.(); if (reload) await refresh(); } }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const toggleFavorite = (item: Souvenir) => act(async () => {
    const saved = await souvenirsApi.favorite(context, item.id, !item.isFavorite);
    if (alive.current) {
      setDetail(previous => previous?.id === saved.id ? { ...previous, isFavorite: saved.isFavorite } : previous);
      browser.refresh();
    }
  }, undefined, false);
  return <section className="ym-page-card" aria-label="伴手礼">
    <div className="ym-page-heading"><h2>伴手礼</h2><button className="ym-button" disabled={busy || loading || browser.loading} onClick={() => { void refresh(); }}>刷新伴手礼</button></div>
    <p className="ym-page-note">这里保存旅行带回的普通礼物和可选图片。可佩戴的旅行饰品在“饰品”页查看；删除礼物不会清除出行事实、信件或游记，也不会再次发放。</p>
    <SouvenirImageSettings/>
    {status?.active && <p role="status">正在准备返回礼物…</p>}
    {status && status.pendingCount > 0 && !status.active && <p className="ym-page-note">有 {status.pendingCount} 次出行的礼物待生成；需要完整行程和有效聊天提供商。</p>}
    {status?.notice && <p className="ym-page-note" role="status">{status.notice}</p>}
    {status?.failures.map(failure => <p key={failure.travelId} role="alert" className="ym-page-error">礼物生成失败：{failure.error.message}。约 {failure.retryAfterSeconds} 秒后重试。</p>)}
    {Boolean(status?.failures.length) && <button className="ym-button" disabled={busy || Boolean(status?.active)} onClick={() => { void act(() => souvenirsApi.retry(context)); }}>重试待生成礼物</button>}
    {((loading && !status) || (browser.loading && !browser.items.length)) && <p role="status">正在读取伴手礼…</p>}
    {error && <p role="alert" className="ym-page-error">{error}</p>}
    {!loading && !browser.loading && !browser.total && !browser.search && !error && !browser.error && <p className="ym-page-empty">{browser.favoritesOnly?"当前筛选下没有收藏的伴手礼。":"还没有收到伴手礼。"}</p>}
    <TravelFilter browser={browser} disabled={busy} favoriteFilter={{label:"只看收藏的伴手礼",checked:browser.favoritesOnly,onChange:browser.setFavoritesOnly}}/><CardBrowserControls browser={browser} label="伴手礼" timeLabel="获得时间" disabled={busy}/>
    {browser.error&&<p role="alert" className="ym-page-error">{browser.error}</p>}
    <ul className="ym-letter-list ym-mailbox-list">{browser.items.map(item=><li key={item.id} className="ym-card-with-favorite">
      <InlineDetailCard thumbnail={item.imageFile?<SouvenirImage item={item} context={context}/>:undefined} title={item.title?.trim()||`${item.travel?.destination||"旅行"}·伴手礼`} summary={item.content.length>140?item.content.slice(0,140)+"…":item.content} meta={<><TravelOriginLabel travel={item.travel} travelId={item.travelId}/><span>{travelTime(item.createdAt)}</span></>} expanded={selected===item.id} disabled={busy} detailLabel="伴手礼详情" onToggle={()=>{setDetail(null);setSelected(selected===item.id?null:item.id);}}>
{detail ? <>
        <div className="ym-page-heading"><h3>{detail.title?.trim()||`${browser.items.find(i=>i.id===detail.id)?.travel?.destination||"旅行"}·伴手礼`}</h3><button className="ym-button" onClick={() => setSelected(null)}>关闭详情</button></div>
        <p className="ym-letter-body">{detail.content}</p><p className="ym-letter-body">{detail.note}</p>
        <p className="ym-page-note">{travelTime(detail.createdAt)} · {detail.isFavorite ? "已收藏" : "未收藏"}</p>
        <div className="ym-inline-form">
          <button className="ym-button" data-variant="danger" disabled={busy || loading || browser.loading} onClick={() => { setError(""); setDeleting(detail); }}>删除伴手礼</button></div>
      </> : <p className="ym-page-note">{selected ? error ? "礼物详情暂不可用，请刷新后重试。" : "正在读取礼物详情…" : "选择一件伴手礼，看看它写的备注。"}</p>}</InlineDetailCard>
      <button type="button" className="ym-button ym-card-favorite" aria-label={item.isFavorite?"取消收藏伴手礼":"收藏伴手礼"} title={item.isFavorite?"取消收藏伴手礼":"收藏伴手礼"} aria-pressed={item.isFavorite} disabled={busy||loading||browser.loading} onClick={()=>{void toggleFavorite(item);}}><UiIcon name="star" filled={item.isFavorite}/></button>
      </li>)}</ul>
      <CardBrowserPagination browser={browser} label="伴手礼" disabled={busy}/>

    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="souvenir-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="souvenir-delete-title">删除这件伴手礼？</h2><div className="ym-session-dialog-body"><p>删除后不可恢复，不会再次发放。本次出行事实、信件、饰品和游记仍保留，不改变喵币。</p>{error && <p role="alert" className="ym-page-error">{error}</p>}</div>
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>取消</button><button className="ym-button" data-variant="danger" disabled={busy || !deleting} onClick={() => {
        if (deleting) void act(() => souvenirsApi.delete(context, deleting.id), () => { setDeleting(null); setSelected(null); setDetail(null); });
      }}>确认删除伴手礼</button></div>
    </dialog>
  </section>;
}
