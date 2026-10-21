import CardBrowserControls from "./CardBrowserControls";
import {useRemoteCardBrowser} from "../hooks/useRemoteCardBrowser";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
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
  const tripsBrowser=useRemoteCardBrowser<Travel>("travels",context);
  const mailBrowser=useRemoteCardBrowser<Letter>("letters",context);
  const [selectedTrip,setSelectedTrip]=useState<Travel|null>(null);
  const [localTab, setLocalTab] = useState<TravelTab>("history");
  const selectedTab = tab ?? localTab;
  const [journalOpened, setJournalOpened] = useState(selectedTab === "journals");
  useEffect(() => { if (selectedTab === "journals") setJournalOpened(true); }, [selectedTab]);
  const [selection, setSelection] = useState<string | null>(null);
  const listRef=useRef<HTMLUListElement>(null);
  const positions=useRef(new Map<string,number>());
  const closeTimer=useRef<ReturnType<typeof setTimeout>>();
  const [closing,setClosing]=useState(false);
  const capture=()=>{positions.current=new Map(Array.from(listRef.current?.children??[]).map(node=>[(node as HTMLElement).dataset.letterId!,node.getBoundingClientRect().top]));};
  const openLetter=(id:string)=>{if(selection===id){closeLetter();return;}clearTimeout(closeTimer.current);capture();setClosing(false);setSelection(id);setDetail(null);};
  const closeLetter=()=>{clearTimeout(closeTimer.current);listRef.current?.querySelector<HTMLButtonElement>(`[data-letter-id="${selection}"] > button`)?.focus({preventScroll:true});if(window.matchMedia?.("(prefers-reduced-motion: reduce)").matches){capture();setSelection(null);return;}setClosing(true);closeTimer.current=setTimeout(()=>{capture();setSelection(null);setClosing(false);},180);};
  useEffect(()=>()=>clearTimeout(closeTimer.current),[]);

  const [detail, setDetail] = useState<Letter | null>(null);
  const [deleting, setDeleting] = useState<Letter | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  useLayoutEffect(()=>{if(window.matchMedia?.("(prefers-reduced-motion: reduce)").matches)return;const animations:Animation[]=[];for(const node of Array.from(listRef.current?.children??[])){const previous=positions.current.get((node as HTMLElement).dataset.letterId!);if(previous!==undefined){const delta=previous-node.getBoundingClientRect().top;if(Math.abs(delta)>1){const a=(node as HTMLElement).animate?.([{transform:`translateY(${delta}px)`},{transform:"translateY(0)"}],{duration:240,easing:"cubic-bezier(.22,1,.36,1)"});if(a)animations.push(a);}}}positions.current.clear();return()=>animations.forEach(a=>a.cancel());},[selection,detailLoading,detail]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const alive = useRef(true);
  const locked = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    setSelectedTrip(previous => previous ? tripsBrowser.items.find(item => item.id === previous.id) ?? previous : null);
  }, [tripsBrowser.items]);
  useEffect(() => {
    let disposed = false;
    if (!selection) { setDetail(null); setDetailLoading(false); return; }
    setDetailLoading(true);
    void (async () => {
      try {
        let letter = await lettersApi.get(context, selection);
        if (disposed) return;
        if (!letter.isRead) letter = await lettersApi.flags(context, letter.id, { read: true });
        if (!disposed) {capture();setDetail(letter);}
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
  const refresh = async () => {
    tripsBrowser.refresh(); mailBrowser.refresh();
    await travel.refresh();
  };
  const act = async (operation: () => Promise<unknown>, after?: () => void, reload = true) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try { await operation(); if (alive.current) { after?.(); if (reload) await refresh(); } }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const canReply = Boolean(detail?.sender === "ai" && !detail.hasBeenReplied && travel.current?.id === detail.travelId && !travel.loading && !travel.error);
  if (!context.sessionId) return <section className="ym-page-card"><h2>出行与邮箱</h2><p>当前没有会话，请先在聊天窗口创建会话。</p></section>;
  return <div className="ym-travel-panel">
    <section className="ym-page-card" aria-label="当前出行">
      <div className="ym-page-heading"><h2>{travel.current ? `${personaName}出门啦` : "出行与邮箱"}</h2>
        <button className="ym-button" disabled={busy || travel.loading} onClick={() => { void act(refresh, undefined, false); }}>刷新</button></div>
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
      <h2>出行历史</h2>{tripsBrowser.loading && <p role="status">正在读取出行记录…</p>}
      {!tripsBrowser.loading && !tripsBrowser.total && !tripsBrowser.search && !tripsBrowser.error && <p className="ym-page-note">还没有出行记录。</p>}
      {!selectedTrip&&<CardBrowserControls browser={tripsBrowser} label="出行" timeLabel="出发时间" disabled={busy}/>}
      {tripsBrowser.error&&<p role="alert" className="ym-page-error">{tripsBrowser.error}</p>}
      {selectedTrip&&<button className="ym-button" onClick={()=>setSelectedTrip(null)}>返回出行列表</button>}
      <ol className="ym-travel-history ym-object-grid">{(selectedTrip?[selectedTrip]:tripsBrowser.items).map(trip => <li className="ym-object-card" key={trip.id} onClick={event=>{if(!selectedTrip&&!(event.target as HTMLElement).closest("button"))setSelectedTrip(trip);}}>
        <div className="ym-page-heading"><h3><button className="ym-card-open" onClick={()=>setSelectedTrip(trip)}>{trip.destination ?? "目的地尚未准备好"}</button></h3><span className="ym-travel-badge" data-status={trip.status}>{stateName[trip.status]}</span></div>
        {!selectedTrip&&<p className="ym-page-note">{travelTime(trip.startedAt)}出发</p>}
        {selectedTrip&&<>
        <dl className="ym-travel-times"><dt>出发</dt><dd>{travelTime(trip.startedAt)}</dd><dt>计划返回</dt><dd>{travelTime(trip.plannedReturnAt)}</dd>
          {trip.status === "returned" && <><dt>实际返回</dt><dd>{travelTime(trip.actualReturnAt)}</dd></>}
          {trip.status === "cancelled" && <><dt>取消</dt><dd>{travelTime(trip.cancelledAt)}</dd></>}
        </dl>
        {trip.status === "cancelled" ? <p className="ym-page-note">旅行已取消，不生成回来信、伴手礼或游记。</p> : trip.status === "returned" && <p className="ym-page-note">{trip.giftKind ? `历史上带回：${trip.giftKind === "text" ? "普通文字伴手礼" : "可佩戴饰品"}` : "伴手礼内容尚未生成。"}</p>}
        <p className="ym-page-note">这次出行的信件、游记、伴手礼可在对应标签中查看。</p>
        </>}
      </li>)}</ol>

    </section> : <section className="ym-page-card" aria-label="邮箱">
      <h2>邮箱</h2>{mailBrowser.loading && <p role="status">正在读取信件…</p>}
      {!mailBrowser.loading && !mailBrowser.total && !mailBrowser.search && !mailBrowser.error && <p className="ym-page-note">还没有信件。出门期间可以寄一封信。</p>}
      <CardBrowserControls browser={mailBrowser} label="信件" timeLabel="收发时间" disabled={busy}/>
      {mailBrowser.error&&<p role="alert" className="ym-page-error">{mailBrowser.error}</p>}
      <ul ref={listRef} className="ym-letter-list ym-mailbox-list">{mailBrowser.items.map(letter=><li key={letter.id} data-letter-id={letter.id} className="ym-letter-card" data-expanded={selection===letter.id}>
        <button className="ym-letter-item" aria-expanded={selection===letter.id} aria-controls={`letter-detail-${letter.id}`} onClick={()=>openLetter(letter.id)}>
          <span className="ym-letter-card-heading"><strong>{letter.sender==="user"?"我寄出的信":`${personaName}的来信`}</strong><time>{travelTime(letter.createdAt)}</time></span>
          <span className="ym-card-summary">{letter.content.replace(/\s+/g," ").slice(0,140)}{letter.content.length>140?"…":""}</span>
          <span className="ym-letter-card-footer"><span className="ym-letter-meta">{letter.isRead?"已读":"未读"}{letter.isFavorite&&" · 已收藏"}{letter.hasBeenReplied&&" · 已回复"}{letter.replyState!=="none"&&` · ${replyStateName[letter.replyState]}`}</span><span className="ym-letter-disclosure">{selection===letter.id?"收起":"查看详情"}<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg></span></span>
        </button>
        {selection===letter.id&&<div id={`letter-detail-${letter.id}`} className="ym-letter-detail" data-closing={closing} role="region" aria-label="信件详情">{detailLoading ? <p role="status">正在读取信件…</p> : detail ? <>
        <div className="ym-page-heading"><h3>{detail.sender === "user" ? "我寄出的信" : `${personaName}的来信`}</h3><button className="ym-button" onClick={closeLetter}>收起信件</button></div>
        <p className="ym-letter-meta">{travelTime(detail.createdAt)} · {detail.isFavorite ? "已收藏" : "未收藏"}</p>
        <p className="ym-letter-body">{detail.content}</p>
        {detail.replyState !== "none" && <p className="ym-page-note">{replyStateName[detail.replyState]}{detail.replyState === "pending" && `，预计 ${travelTime(detail.replyDueAt)} 之后回复；旅行结束则取消。`}</p>}
        {detail.sender === "ai" && <p className="ym-page-note">{detail.hasBeenReplied ? "这封信已经回复过，删除回复也不会恢复资格。" : canReply ? "本次旅行尚未结束，可以回复一次。" : "只能在这封信所属旅行尚未结束时回复。"}</p>}
        <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => { void act(() => lettersApi.flags(context, detail.id, {favorite:!detail.isFavorite})); }}>{detail.isFavorite ? "取消收藏" : "收藏"}</button>
          {detail.sender === "ai" && <button className="ym-button" disabled={busy || !canReply} onClick={() => { void act(() => navigation.openLetter(context, detail.travelId!, detail.id)); }}>回信</button>}
          <button className="ym-button" data-variant="danger" disabled={busy} onClick={() => { setDeleting(detail); }}>删除信件</button></div>
      </> : <p className="ym-page-note">选择一封信，查看完整内容。</p>}</div>}
      </li>)}</ul>

    </section>}
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="letter-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="letter-delete-title">删除这封信？</h2><p>删除后不可恢复。已回复的资格不会恢复，其他信件、出行和伴手礼仍保留；尚未完成的原信回信任务会取消。</p>
      {error && <p role="alert" className="ym-page-error">{error}</p>}
      <div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>取消</button>
        <button className="ym-button" data-variant="danger" disabled={busy || !deleting} onClick={() => { if (deleting) void act(() => lettersApi.delete(context, deleting.id), () => { setDeleting(null); setSelection(null); setDetail(null); }); }}>确认删除</button></div>
    </dialog>
  </div>;
}
