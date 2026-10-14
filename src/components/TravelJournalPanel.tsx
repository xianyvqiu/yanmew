import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, travelApi, travelJournalsApi } from "../api";
import type { CurrentContext, JournalGenerationStatus, TravelJournal, TravelJournalSummary } from "../types";
import { travelTime } from "./travelTime";

type Confirmation = { kind: "delete"; journal: TravelJournal } | { kind: "discard"; next: string | null; reload: boolean };
export default function TravelJournalPanel({ context }: { context: CurrentContext }) {
  return <JournalPage key={`${context.personaId}:${context.sessionId}:${context.contextRevision}`} context={context} />;
}
function JournalPage(props: { context: CurrentContext }) {
  const [context] = useState(props.context);
  const [items, setItems] = useState<TravelJournalSummary[]>([]);
  const [status, setStatus] = useState<JournalGenerationStatus | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<TravelJournal | null>(null);
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [remoteChanged, setRemoteChanged] = useState(false);
  const [missing, setMissing] = useState(false);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [more, setMore] = useState(false);
  const [readError, setReadError] = useState("");
  const [actionError, setActionError] = useState("");
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const alive = useRef(false);
  const locked = useRef(false);
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);
  const pageDepth = useRef(1);
  const rows = useRef<TravelJournalSummary[]>([]);
  const current = useRef({ selected, detail, editing });
  current.current = { selected, detail, editing };
  const dirty = editing && detail !== null && (title !== detail.title || content !== detail.content);

  const accept = useCallback((journal: TravelJournal) => {
    setDetail(journal); setTitle(journal.title); setContent(journal.content);
    setRemoteChanged(false); setMissing(false);
  }, []);
  const readDetail = useCallback(async (id: string, force = false) => {
    const token = ++detailGeneration.current;
    if (!current.current.detail || force) setDetailLoading(true);
    try {
      const journal = await travelJournalsApi.get(context, id);
      if (!alive.current || token !== detailGeneration.current || current.current.selected !== id || locked.current) return;
      const old = current.current;
      // 编辑期间保留原始 expected 与草稿，刷新只提示远端变化。
      if (old.editing && !force && old.detail) {
        setRemoteChanged(journal.title !== old.detail.title || journal.content !== old.detail.content || journal.updatedAt !== old.detail.updatedAt);
      } else accept(journal);
      setMissing(false);
    } catch (cause) {
      if (!alive.current || token !== detailGeneration.current || current.current.selected !== id || locked.current) return;
      setReadError(errorMessage(cause));
      if ((cause as { code?: string })?.code === "NOT_FOUND") { setMissing(true); if (!current.current.editing) setDetail(null); }
    } finally { if (alive.current && token === detailGeneration.current) setDetailLoading(false); }
  }, [context, accept]);
  const load = useCallback(async (append = false) => {
    if (!alive.current || locked.current) return;
    if (!context.sessionId) { setLoading(false); return; }
    const token = ++listGeneration.current; setLoading(true);
    try {
      let cursor = append ? rows.current[rows.current.length - 1]?.id ?? null : null;
      const records: TravelJournalSummary[] = [];
      let lastCount = 0;
      const progress = travelJournalsApi.status(context).then(value => ({ value, error: null as unknown })).catch(error => ({ value: null, error }));
      // 刷新保留已展开页数，旧篇目不会因首页补查而丢失。
      for (let page = 0; page < (append ? 1 : pageDepth.current); page++) {
        const result = await travelJournalsApi.list(context, cursor);
        if (!alive.current || token !== listGeneration.current || locked.current) return;
        records.push(...result); lastCount = result.length;
        cursor = result[result.length - 1]?.id ?? null;
        if (lastCount < 30) break;
      }
      const nextStatus = await progress;
      if (nextStatus.error) throw nextStatus.error;
      if (!alive.current || token !== listGeneration.current || locked.current) return;
      if (append) pageDepth.current++;
      rows.current = append ? [...rows.current, ...records.filter(row => !rows.current.some(old => old.id === row.id))] : records;
      setItems(rows.current); setStatus(nextStatus.value); setMore(lastCount === 30); setReadError("");
      if (current.current.selected) void readDetail(current.current.selected);
    } catch (cause) { if (alive.current && token === listGeneration.current && !locked.current) setReadError(errorMessage(cause)); }
    finally { if (alive.current && token === listGeneration.current) setLoading(false); }
  }, [context, readDetail]);
  useEffect(() => {
    alive.current = true; let disposed = false; let stop: (() => void) | undefined;
    const focus = () => { void load(); };
    void travelApi.listen(change => { if (change.sessionId === context.sessionId) void load(); }).then(unlisten => {
      if (disposed) { unlisten(); return; }
      stop = unlisten; window.addEventListener("focus", focus); void load();
    }).catch(cause => {
      if (!disposed) { setActionError(`通知暂不可用：${errorMessage(cause)}`); window.addEventListener("focus", focus); void load(); }
    });
    return () => { disposed = true; alive.current = false; listGeneration.current++; detailGeneration.current++; stop?.(); window.removeEventListener("focus", focus); };
  }, [context, load]);
  useEffect(() => { if (selected) void readDetail(selected); }, [selected, readDetail]);
  useEffect(() => { if (confirmation && !dialog.current?.open) dialog.current?.showModal(); if (!confirmation) dialog.current?.close(); }, [confirmation]);
  const choose = (id: string | null) => {
    if (locked.current || id === selected) return;
    if (dirty) { setConfirmation({ kind: "discard", next: id, reload: false }); return; }
    switchSelection(id);
  };
  const switchSelection = (id: string | null) => {
    detailGeneration.current++; setSelected(id); setDetail(null); setEditing(false);
    setTitle(""); setContent(""); setMissing(false); setRemoteChanged(false); setReadError(""); setActionError("");
  };
  const act = async (operation: () => Promise<void>) => {
    if (locked.current) return;
    locked.current = true; listGeneration.current++; detailGeneration.current++;
    setLoading(false); setDetailLoading(false); setBusy(true); setActionError("");
    try { await operation(); }
    catch (cause) {
      if (alive.current) {
        setActionError(errorMessage(cause));
        if ((cause as { code?: string })?.code === "JOURNAL_CONFLICT") setRemoteChanged(true);
        if ((cause as { code?: string })?.code === "NOT_FOUND") setMissing(true);
      }
    } finally { locked.current = false; if (alive.current) { setBusy(false); await load(); } }
  };
  const discard = () => {
    if (!detail) return;
    if (dirty) setConfirmation({ kind: "discard", next: selected, reload: true });
    else { setEditing(false); void readDetail(detail.id, true); }
  };
  if (!context.sessionId) return <section className="ym-page-card"><h2>旅行游记</h2><p>当前没有会话，请先在聊天窗口创建会话。</p></section>;
  return <section className="ym-page-card" aria-label="旅行游记">
    <div className="ym-page-heading"><h2>旅行游记</h2><button className="ym-button" disabled={busy || loading} onClick={() => { void load(); }}>刷新游记</button></div>
    <p className="ym-page-note">游记以人设口吻总结旅行，不参与聊天记忆整合。可以手动修改标题和正文，不提供 AI 重新生成。</p>
    {status?.active && <p role="status">正在生成旅行游记…</p>}
    {status && status.pendingCount > 0 && !status.active && <p className="ym-page-note">有 {status.pendingCount} 篇游记待生成，行程与返回礼物准备好后会继续。</p>}
    {status?.failures.map(failure => <p role="alert" key={failure.travelId}>游记生成失败：{failure.error.message}。约 {failure.retryAfterSeconds} 秒后重试。</p>)}
    {Boolean(status?.failures.length) && <button className="ym-button" disabled={busy} onClick={() => { void act(() => travelJournalsApi.retry(context)); }}>重试待生成游记</button>}
    {status?.notice && <p role="status" className="ym-page-note">{status.notice}</p>}
    {readError && <p role="alert" className="ym-page-error">{readError}</p>}
    {actionError && <p role="alert" className="ym-page-error">{actionError}</p>}
    {loading && <p role="status">正在读取游记列表…</p>}
    {!loading && !items.length && <p className="ym-page-note">还没有游记。正常旅行返回后会自动生成；已删除的游记不会补回。</p>}
    <div className="ym-mailbox-layout"><ul className="ym-letter-list">{items.map(item => <li key={item.id}>
      <button className="ym-letter-item" disabled={busy} aria-pressed={selected === item.id} onClick={() => choose(item.id)}><strong>{item.title}</strong><time>生成：{travelTime(item.createdAt)}</time></button>
    </li>)}</ul>
      <div className="ym-letter-detail" aria-label="游记详情">{detailLoading ? <p role="status">正在读取游记…</p> : detail ? <>
        <div className="ym-page-heading"><h3>{editing ? "编辑游记" : detail.title}</h3><button className="ym-button" disabled={busy} onClick={() => choose(null)}>关闭详情</button></div>
        <p className="ym-letter-meta">生成：{travelTime(detail.createdAt)} · 修改：{travelTime(detail.updatedAt)}</p>
        {missing && <p role="alert" className="ym-page-error">这篇游记已不存在。草稿仍可复制，但不能保存或重新创建。</p>}
        {remoteChanged && <p role="alert" className="ym-page-error">游记已有新的修改。当前草稿已保留，请先加载最新内容再编辑。</p>}
        {editing ? <div className="ym-letter-editor">
          <label>游记标题<textarea className="ym-journal-title" rows={2} value={title} disabled={busy} onChange={event => setTitle(event.target.value)} /></label>
          <label>游记正文<textarea value={content} disabled={busy} onChange={event => setContent(event.target.value)} /></label>
          <p className="ym-page-note">手动修改只要求标题和正文非空，不受 AI 生成字数范围限制。</p>
          <div className="ym-inline-form"><button className="ym-button" disabled={busy || missing || remoteChanged || !title.trim() || !content.trim()} onClick={() => {
            const original = detail;
            void act(async () => {
              const saved = await travelJournalsApi.update(context, original.id, original.travelId,
                { title: original.title, content: original.content, updatedAt: original.updatedAt }, title, content);
              if (alive.current) { accept(saved); setEditing(false); }
            });
          }}>保存游记</button><button className="ym-button" disabled={busy} onClick={discard}>{remoteChanged ? "放弃草稿并加载最新" : "取消编辑"}</button></div>
        </div> : <><p className="ym-letter-body">{detail.content}</p><div className="ym-inline-form">
          <button className="ym-button" disabled={busy || missing} onClick={() => { setEditing(true); setActionError(""); }}>编辑游记</button>
          <button className="ym-button" data-variant="danger" disabled={busy || missing} onClick={() => { setActionError(""); setConfirmation({ kind: "delete", journal: detail }); }}>删除游记</button>
        </div></>}
      </> : <p className="ym-page-note">{selected ? "游记详情暂不可用，请刷新后重试。" : "选择一篇游记，查看完整内容。"}</p>}</div>
    </div>
    {more && <button className="ym-button" disabled={busy || loading} onClick={() => { void load(true); }}>更多游记</button>}
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="journal-confirm-title" onCancel={event => { if (busy) event.preventDefault(); else setConfirmation(null); }}>
      <h2 id="journal-confirm-title">{confirmation?.kind === "delete" ? "删除这篇游记？" : "放弃未保存的修改？"}</h2>
      <div className="ym-session-dialog-body"><p>{confirmation?.kind === "delete" ? "删除后不可恢复，也不会重新生成。出行记录、信件、伴手礼和饰品仍保留，不改变喵币。" : "当前草稿尚未保存。确认后会放弃这些修改。"}</p>{confirmation && actionError && <p role="alert" className="ym-page-error">{actionError}</p>}</div>
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setConfirmation(null)}>取消</button>
        <button className="ym-button" data-variant="danger" disabled={busy || !confirmation} onClick={() => {
          if (confirmation?.kind === "delete") {
            const journal = confirmation.journal;
            void act(async () => { await travelJournalsApi.delete(context, journal.id, journal.travelId); if (alive.current) { setConfirmation(null); switchSelection(null); } });
          } else if (confirmation) {
            const choice = confirmation; setConfirmation(null);
            if (choice.reload && detail) { setEditing(false); setActionError(""); void readDetail(detail.id, true); }
            else switchSelection(choice.next);
          }
        }}>{confirmation?.kind === "delete" ? "确认删除游记" : "确认放弃修改"}</button>
      </div>
    </dialog>
  </section>;
}
