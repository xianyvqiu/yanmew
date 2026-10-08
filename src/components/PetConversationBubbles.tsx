import { useEffect, useRef, useState } from "react";
import { petApi } from "../api";
import { navigation } from "../platform/navigation";
import type { PetActivity, ProactiveMessageNotice } from "../types";
import PetStateBubble from "./PetStateBubble";
import "./ReminderNotifications.css";

function matches(notice: ProactiveMessageNotice, view: PetActivity | null) {
  return Boolean(view?.canWander && notice.personaId === view.personaId
    && notice.sessionId === view.sessionId && notice.contextRevision === view.contextRevision
    && notice.message.sessionId === view.sessionId);
}

/** 只消费已保存消息的实时通知；遮挡时由提醒插槽卸载，不补弹历史。 */
export default function PetConversationBubbles({ view, seenMessageIds }: {
  view: PetActivity | null; seenMessageIds?: Set<string>;
}) {
  const [notice, setNotice] = useState<ProactiveMessageNotice | null>(null);
  const [error, setError] = useState("");
  const [opening, setOpening] = useState(false);
  const current = useRef(view);
  current.current = view;
  const seen = useRef(seenMessageIds ?? new Set<string>());
  const activeNotice = useRef(notice);
  activeNotice.current = notice;
  const openingRef = useRef(false);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void petApi.listenProactiveMessage(next => {
      if (disposed || seen.current.has(next.message.id)) return;
      seen.current.add(next.message.id);
      if (!matches(next, current.current)) return;
      setError("");
      setNotice(next);
    }).then(unlisten => { if (disposed) unlisten(); else stop = unlisten; }).catch(() => {});
    return () => { disposed = true; activeNotice.current = null; stop?.(); };
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(null), 20000);
    return () => window.clearTimeout(timeout);
  }, [notice]);
  useEffect(() => {
    if (notice && !matches(notice, view)) setNotice(null);
  }, [notice, view]);
  const open = async () => {
    if (openingRef.current || !notice || !matches(notice, current.current)) return;
    openingRef.current = true; setOpening(true); setError("");
    try { await navigation.open("chat"); }
    catch {
      if (activeNotice.current === notice) setError("暂时无法打开聊天，请重试。");
    } finally { openingRef.current = false; setOpening(false); }
  };
  if (!notice || !matches(notice, view)) return <PetStateBubble view={view} durationMs={8000} />;
  return <section className="ym-reminder-notification" data-pet-interactive role="status" aria-label="桌宠主动消息">
    <div className="ym-reminder-notification__body">
      <p className="ym-reminder-notification__message">{notice.message.content}</p>
      {error && <p className="ym-reminder-notification__error" role="alert">{error}</p>}
    </div>
    <div className="ym-reminder-notification__actions">
      <button className="ym-button ym-reminder-notification__action" disabled={opening} onClick={() => { void open(); }}>查看聊天</button>
    </div>
  </section>;
}
