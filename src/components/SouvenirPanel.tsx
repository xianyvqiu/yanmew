import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, souvenirsApi, travelApi } from "../api";
import type { CurrentContext, GiftGenerationStatus, Souvenir } from "../types";
import { travelTime } from "./travelTime";

export default function SouvenirPanel({ context }: { context: CurrentContext }) {
  return <SouvenirPanelForContext key={`${context.personaId}:${context.sessionId}:${context.contextRevision}`} context={context} />;
}
function SouvenirPanelForContext({ context }: { context: CurrentContext }) {
  const [items, setItems] = useState<Souvenir[]>([]);
  const [status, setStatus] = useState<GiftGenerationStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [more, setMore] = useState(false);
  const [error, setError] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Souvenir | null>(null);
  const [deleting, setDeleting] = useState<Souvenir | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const read = useRef<(append?: boolean) => Promise<void>>(async () => {});
  const locked = useRef(false);
  const alive = useRef(false);
  const refresh = useCallback(() => read.current(), []);
  useEffect(() => {
    alive.current = true; let disposed = false; let generation = 0; let rows: Souvenir[] = [];
    let pageDepth = 1;
    const stops: (() => void)[] = [];
    const load = async (append = false) => {
      if (disposed || !context.sessionId) { if (!disposed) setLoading(false); return; }
      const token = ++generation; setLoading(true);
      try {
        const readPages = async () => {
          let cursor = append ? rows[rows.length - 1]?.id ?? null : null;
          const records: Souvenir[] = [];
          let lastCount = 0;
          for (let page = 0; page < (append ? 1 : pageDepth); page++) {
            const result = await souvenirsApi.list(context, cursor);
            if (disposed || token !== generation) return { records: [], lastCount: 0 };
            records.push(...result); lastCount = result.length;
            cursor = result[result.length - 1]?.id ?? null;
            if (lastCount < 30) break;
          }
          return { records, lastCount };
        };
        const [page, progress] = await Promise.all([readPages(), souvenirsApi.status(context)]);
        if (disposed || token !== generation) return;
        if (append) pageDepth++;
        const combined = append ? [...rows, ...page.records] : page.records;
        rows = combined.filter((item, index) => combined.findIndex(row => row.id === item.id) === index);
        setItems(rows); setStatus(progress); setMore(page.lastCount === 30); setError("");
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
    let disposed = false; setDetail(null);
    if (selected) void souvenirsApi.get(context, selected).then(item => { if (!disposed) setDetail(item); })
      .catch(cause => { if (!disposed) setError(errorMessage(cause)); });
    return () => { disposed = true; };
  }, [selected, items]);
  useEffect(() => { if (deleting) dialog.current?.showModal(); else dialog.current?.close(); }, [deleting]);
  const act = async (operation: () => Promise<unknown>, after?: () => void) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try { await operation(); if (alive.current) { after?.(); await refresh(); } }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  return <section className="ym-page-card" aria-label="伴手礼">
    <div className="ym-page-heading"><h2>伴手礼</h2><button className="ym-button" disabled={busy || loading} onClick={() => { void refresh(); }}>刷新伴手礼</button></div>
    <p className="ym-page-note">这里保存普通文字礼物。可佩戴的旅行饰品在“饰品”页查看；删除礼物不会清除出行事实、信件或游记，也不会再次发放。</p>
    {status?.active && <p role="status">正在准备返回礼物…</p>}
    {status && status.pendingCount > 0 && !status.active && <p className="ym-page-note">有 {status.pendingCount} 次出行的礼物待生成；需要完整行程和有效聊天提供商。</p>}
    {status?.notice && <p className="ym-page-note" role="status">{status.notice}</p>}
    {status?.failures.map(failure => <p key={failure.travelId} role="alert" className="ym-page-error">礼物生成失败：{failure.error.message}。约 {failure.retryAfterSeconds} 秒后重试。</p>)}
    {Boolean(status?.failures.length) && <button className="ym-button" disabled={busy || Boolean(status?.active)} onClick={() => { void act(() => souvenirsApi.retry(context)); }}>重试待生成礼物</button>}
    {loading && <p role="status">正在读取伴手礼…</p>}
    {error && <p role="alert" className="ym-page-error">{error}</p>}
    {!loading && !items.length && !error && <p className="ym-page-empty">还没有收到伴手礼。</p>}
    <div className="ym-mailbox-layout"><ul className="ym-letter-list">{items.map(item => <li key={item.id}><button className="ym-letter-item" aria-pressed={selected === item.id} onClick={() => setSelected(item.id)}>
      <strong>{item.content}</strong><span className="ym-letter-meta">{item.isFavorite ? "已收藏" : "未收藏"}</span><time>{travelTime(item.createdAt)}</time></button></li>)}</ul>
      <div className="ym-letter-detail" aria-label="伴手礼详情">{detail ? <>
        <div className="ym-page-heading"><h3>带回的礼物</h3><button className="ym-button" onClick={() => setSelected(null)}>关闭详情</button></div>
        <p className="ym-letter-body">{detail.content}</p><p className="ym-letter-body">{detail.note}</p>
        <p className="ym-page-note">{travelTime(detail.createdAt)} · {detail.isFavorite ? "已收藏" : "未收藏"}</p>
        <div className="ym-inline-form"><button className="ym-button" disabled={busy || loading} onClick={() => { void act(() => souvenirsApi.favorite(context, detail.id, !detail.isFavorite)); }}>{detail.isFavorite ? "取消收藏" : "收藏"}</button>
          <button className="ym-button" data-variant="danger" disabled={busy || loading} onClick={() => { setError(""); setDeleting(detail); }}>删除伴手礼</button></div>
      </> : <p className="ym-page-note">{selected ? error ? "礼物详情暂不可用，请刷新后重试。" : "正在读取礼物详情…" : "选择一件伴手礼，看看它写的备注。"}</p>}</div></div>
    {more && <button className="ym-button" disabled={busy || loading} onClick={() => { void read.current(true); }}>更多伴手礼</button>}
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="souvenir-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="souvenir-delete-title">删除这件伴手礼？</h2><div className="ym-session-dialog-body"><p>删除后不可恢复，不会再次发放。本次出行事实、信件、饰品和游记仍保留，不改变喵币。</p>{error && <p role="alert" className="ym-page-error">{error}</p>}</div>
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>取消</button><button className="ym-button" data-variant="danger" disabled={busy || !deleting} onClick={() => {
        if (deleting) void act(() => souvenirsApi.delete(context, deleting.id), () => { setDeleting(null); setSelected(null); setDetail(null); });
      }}>确认删除伴手礼</button></div>
    </dialog>
  </section>;
}
