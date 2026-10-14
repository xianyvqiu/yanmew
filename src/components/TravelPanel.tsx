import { useEffect, useRef, useState } from "react";
import { errorMessage, lettersApi, travelApi } from "../api";
import { travelContextKey, useTravel } from "../hooks/useTravel";
import { navigation } from "../platform/navigation";
import type { CurrentContext, Letter, Travel } from "../types";
import SouvenirPanel from "./SouvenirPanel";
import TravelJournalPanel from "./TravelJournalPanel";
import "./TravelPanel.css";
import "../pages/workspace.css";

export type TravelTab = "history" | "mailbox" | "souvenirs" | "journals";
import { travelTime } from "./travelTime";
export { travelTime } from "./travelTime";
const stateName = { away: "出门中", returned: "已返回", cancelled: "已取消" };
export const replyStateName = { none: "", pending: "等待回信", completed: "已收到回信", cancelled: "待回信已取消" };
export default function TravelPanel(props: { context: CurrentContext; personaName: string; tab?: TravelTab; onTabChange?: (tab: TravelTab) => void }) {
  return <TravelPanelForContext key={travelContextKey(props.context)} {...props} />;
}
function TravelPanelForContext({ context, personaName, tab, onTabChange }: {
  context: CurrentContext; personaName: string; tab?: TravelTab; onTabChange?: (tab: TravelTab) => void;
}) {
  const travel = useTravel(context);
  const [localTab, setLocalTab] = useState<TravelTab>("history");
  const selectedTab = tab ?? localTab;
  const [journalOpened, setJournalOpened] = useState(selectedTab === "journals");
  useEffect(() => { if (selectedTab === "journals") setJournalOpened(true); }, [selectedTab]);
  const [history, setHistory] = useState<Travel[]>([]);
  const [mail, setMail] = useState<Letter[]>([]);
  const [moreTrips, setMoreTrips] = useState(false);
  const [moreMail, setMoreMail] = useState(false);
  const [selection, setSelection] = useState<string | null>(null);
  const [detail, setDetail] = useState<Letter | null>(null);
  const [deleting, setDeleting] = useState<Letter | null>(null);
  const [loading, setLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const alive = useRef(true);
  const locked = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const listGeneration = useRef(0);
  const tripPages = useRef(1);
  const mailPages = useRef(1);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    if (!context.sessionId) { setLoading(false); return; }
    let disposed = false;
    const token = ++listGeneration.current;
    const valid = () => !disposed && token === listGeneration.current;
    const readPages = async <T extends { id: string },>(read: (before: string | null) => Promise<T[]>, depth: number) => {
      const items: T[] = [];
      let cursor: string | null = null;
      let count = 0;
      // 与游记列表一致，刷新读取已展开页，邮箱和出行历史分别记录深度。
      for (let page = 0; page < depth; page++) {
        const result = await read(cursor);
        if (!valid()) return null;
        items.push(...result); count = result.length;
        cursor = result[result.length - 1]?.id ?? null;
        if (count < 30) break;
      }
      return { items, more: count === 30 };
    };
    setLoading(true);
    void Promise.all([
      readPages<Travel>(before => before ? travelApi.list(context, before) : travelApi.list(context), tripPages.current),
      readPages<Letter>(before => before ? lettersApi.list(context, before) : lettersApi.list(context), mailPages.current),
    ]).then(([trips, letters]) => {
      if (!valid() || !trips || !letters) return;
      setHistory(trips.items); setMail(letters.items); setMoreTrips(trips.more); setMoreMail(letters.more); setError("");
    }).catch(cause => { if (valid()) setError(errorMessage(cause)); }).finally(() => { if (valid()) setLoading(false); });
    return () => { disposed = true; };
  }, [travel.version]);
  useEffect(() => {
    let disposed = false;
    if (!selection) { setDetail(null); return; }
    setDetailLoading(true);
    void (async () => {
      try {
        let letter = await lettersApi.get(context, selection);
        if (disposed) return;
        if (!letter.isRead) letter = await lettersApi.flags(context, letter.id, { read: true });
        if (!disposed) setDetail(letter);
      } catch (cause) {
        if (disposed) return;
        setError(errorMessage(cause));
        if ((cause as { code?: string })?.code === "NOT_FOUND") { setSelection(null); setDetail(null); }
      } finally { if (!disposed) setDetailLoading(false); }
    })();
    return () => { disposed = true; };
  }, [selection, travel.version]);
  useEffect(() => {
    if (deleting && !dialog.current?.open) dialog.current?.showModal();
    if (!deleting) dialog.current?.close();
  }, [deleting]);
  const act = async (operation: () => Promise<unknown>, after?: () => void, reload = true) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try { await operation(); if (alive.current) { after?.(); if (reload) await travel.refresh(); } }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const loadMore = async () => {
    const mailbox = selectedTab === "mailbox";
    await act(async () => {
      const token = ++listGeneration.current;
      setLoading(true);
      try {
        if (mailbox) {
          const rows = await lettersApi.list(context, mail[mail.length - 1]?.id ?? null);
          if (alive.current && token === listGeneration.current) {
            mailPages.current++;
            setMail(old => [...old, ...rows.filter(row => !old.some(item => item.id === row.id))]); setMoreMail(rows.length === 30);
          }
        } else {
          const rows = await travelApi.list(context, history[history.length - 1]?.id ?? null);
          if (alive.current && token === listGeneration.current) {
            tripPages.current++;
            setHistory(old => [...old, ...rows.filter(row => !old.some(item => item.id === row.id))]); setMoreTrips(rows.length === 30);
          }
        }
      } catch (cause) { if (alive.current && token === listGeneration.current) throw cause; }
      finally { if (alive.current && token === listGeneration.current) setLoading(false); }
    }, undefined, false);
  };
  const canReply = Boolean(detail?.sender === "ai" && !detail.hasBeenReplied && travel.current?.id === detail.travelId && !travel.loading && !travel.error);
  if (!context.sessionId) return <section className="ym-page-card"><h2>出行与邮箱</h2><p>当前没有会话，请先在聊天窗口创建会话。</p></section>;
  return <div className="ym-travel-panel">
    <section className="ym-page-card" aria-label="当前出行">
      <div className="ym-page-heading"><h2>{travel.current ? `${personaName}出门啦` : "出行与邮箱"}</h2>
        <button className="ym-button" disabled={busy || travel.loading} onClick={() => { void act(() => travel.refresh(), undefined, false); }}>刷新</button></div>
      {travel.current ? <><p>{travel.current.destination ? `目的地：${travel.current.destination}` : "正在准备本次行程。"}</p>
        <p className="ym-page-note">计划返回：{travelTime(travel.current.plannedReturnAt)}。切换人设或会话会取消本次旅行和待回信，不生成返回奖励。</p>
        <button className="ym-button" disabled={busy || travel.loading || Boolean(travel.error)} onClick={() => { void act(() => navigation.openLetter(context, travel.current!.id)); }}>写信</button></>
        : <p className="ym-page-note">{travel.loading ? "正在读取出行状态…" : "它在家，可以查看以前的出行和信件。"}</p>}
      {(error || travel.error) && <p role="alert" className="ym-page-error">{error || travel.error}</p>}
      {travel.facts.active && <p role="status">正在准备旅行行程…</p>}
      {travel.facts.failures.map(failure => <p key={failure.travelId} role="alert">行程准备失败：{failure.error.message}。约 {failure.retryAfterSeconds} 秒后重试。</p>)}
      {travel.facts.failures.length > 0 && <button className="ym-button" disabled={busy} onClick={() => { void act(() => travelApi.retry(context)); }}>重试行程生成</button>}
      {travel.letters.active && <p role="status">正在生成{travel.letters.active.kind === "midway" ? "途中信" : travel.letters.active.kind === "return" ? "回来信" : "回信"}…</p>}
      {travel.letters.failures.map(failure => <p key={`${failure.task.travelId}:${failure.task.letterId ?? failure.task.kind}`} role="alert">信件生成失败：{failure.error.message}。约 {failure.retryAfterSeconds} 秒后重试。</p>)}
      {travel.letters.failures.length > 0 && <button className="ym-button" disabled={busy} onClick={() => { void act(() => lettersApi.retry(context)); }}>重试待生成信件</button>}
      {travel.letters.notice && <p role="status" className="ym-page-note">{travel.letters.notice}</p>}
    </section>
    <nav className="ym-travel-tabs" aria-label="出行内容">{(["history", "mailbox", "souvenirs", "journals"] as TravelTab[]).map(item => <button key={item} className="ym-button" aria-current={selectedTab === item ? "page" : undefined}
      onClick={() => { setLocalTab(item); onTabChange?.(item); }}>{item === "history" ? "出行历史" : item === "mailbox" ? "邮箱" : item === "souvenirs" ? "伴手礼" : "旅行游记"}</button>)}</nav>
    {(selectedTab === "journals" || journalOpened) && <div hidden={selectedTab !== "journals"}><TravelJournalPanel context={context} /></div>}
    {selectedTab === "journals" ? null : selectedTab === "souvenirs" ? <SouvenirPanel context={context} /> : selectedTab === "history" ? <section className="ym-page-card" aria-label="出行历史">
      <h2>出行历史</h2>{loading && <p role="status">正在读取出行记录…</p>}
      {!loading && !history.length && <p className="ym-page-note">还没有出行记录。</p>}
      <ol className="ym-travel-history">{history.map(trip => <li key={trip.id}>
        <div className="ym-page-heading"><h3>{trip.destination ?? "目的地尚未准备好"}</h3><span className="ym-travel-badge" data-status={trip.status}>{stateName[trip.status]}</span></div>
        <dl className="ym-travel-times"><dt>出发</dt><dd>{travelTime(trip.startedAt)}</dd><dt>计划返回</dt><dd>{travelTime(trip.plannedReturnAt)}</dd>
          {trip.status === "returned" && <><dt>实际返回</dt><dd>{travelTime(trip.actualReturnAt)}</dd></>}
          {trip.status === "cancelled" && <><dt>取消</dt><dd>{travelTime(trip.cancelledAt)}</dd></>}
        </dl>
        {trip.status === "cancelled" ? <p className="ym-page-note">旅行已取消，不生成回来信、伴手礼或游记。</p> : trip.status === "returned" && <p className="ym-page-note">{trip.giftKind ? `历史上带回：${trip.giftKind === "text" ? "普通文字伴手礼" : "可佩戴饰品"}` : "伴手礼内容尚未生成。"}</p>}
      </li>)}</ol>
      {moreTrips && <button className="ym-button" disabled={busy || loading} onClick={() => { void loadMore(); }}>更多出行记录</button>}
    </section> : <section className="ym-page-card" aria-label="邮箱">
      <h2>邮箱</h2>{loading && <p role="status">正在读取信件…</p>}
      {!loading && !mail.length && <p className="ym-page-note">还没有信件。出门期间可以寄一封信。</p>}
      <div className="ym-mailbox-layout"><ul className="ym-letter-list">{mail.map(letter => <li key={letter.id}>
        <button className="ym-letter-item" aria-pressed={selection === letter.id} onClick={() => { if (selection !== letter.id) { setSelection(letter.id); setDetail(null); } }}>
          <strong>{letter.sender === "user" ? "我寄出的信" : `${personaName}的来信`}</strong>
          <span className="ym-letter-meta">{letter.isRead ? "已读" : "未读"}{letter.isFavorite && " · 已收藏"}{letter.hasBeenReplied && " · 已回复"}</span>
          <span className="ym-letter-preview">{letter.content.slice(0, 90)}</span><time>{travelTime(letter.createdAt)}</time>
          {letter.replyState !== "none" && <span className="ym-letter-meta">{replyStateName[letter.replyState]}</span>}
        </button>
      </li>)}</ul>
      <div className="ym-letter-detail" aria-label="信件详情">{detailLoading ? <p role="status">正在读取信件…</p> : detail ? <>
        <div className="ym-page-heading"><h3>{detail.sender === "user" ? "我寄出的信" : `${personaName}的来信`}</h3><button className="ym-button" onClick={() => { setSelection(null); }}>关闭详情</button></div>
        <p className="ym-letter-meta">{travelTime(detail.createdAt)} · {detail.isFavorite ? "已收藏" : "未收藏"}</p>
        <p className="ym-letter-body">{detail.content}</p>
        {detail.replyState !== "none" && <p className="ym-page-note">{replyStateName[detail.replyState]}{detail.replyState === "pending" && `，预计 ${travelTime(detail.replyDueAt)} 之后回复；旅行结束则取消。`}</p>}
        {detail.sender === "ai" && <p className="ym-page-note">{detail.hasBeenReplied ? "这封信已经回复过，删除回复也不会恢复资格。" : canReply ? "本次旅行尚未结束，可以回复一次。" : "只能在这封信所属旅行尚未结束时回复。"}</p>}
        <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => { void act(() => lettersApi.flags(context, detail.id, {favorite:!detail.isFavorite})); }}>{detail.isFavorite ? "取消收藏" : "收藏"}</button>
          {detail.sender === "ai" && <button className="ym-button" disabled={busy || !canReply} onClick={() => { void act(() => navigation.openLetter(context, detail.travelId!, detail.id)); }}>回信</button>}
          <button className="ym-button" data-variant="danger" disabled={busy} onClick={() => { setDeleting(detail); }}>删除信件</button></div>
      </> : <p className="ym-page-note">选择一封信，查看完整内容。</p>}</div></div>
      {moreMail && <button className="ym-button" disabled={busy || loading} onClick={() => { void loadMore(); }}>更多信件</button>}
    </section>}
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="letter-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="letter-delete-title">删除这封信？</h2><p>删除后不可恢复。已回复的资格不会恢复，其他信件、出行和伴手礼仍保留；尚未完成的原信回信任务会取消。</p>
      {error && <p role="alert" className="ym-page-error">{error}</p>}
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>取消</button>
        <button className="ym-button" data-variant="danger" disabled={busy || !deleting} onClick={() => { if (deleting) void act(() => lettersApi.delete(context, deleting.id), () => { setDeleting(null); setSelection(null); setDetail(null); }); }}>确认删除</button></div>
    </dialog>
  </div>;
}
