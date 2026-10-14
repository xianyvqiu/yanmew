import { useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import useReminderNotifications, { noticeKey } from "../hooks/useReminderNotifications";
import { navigation } from "../platform/navigation";
import ReminderBubble from "./ReminderBubble";
import MissedReminderSummaryDialog from "./MissedReminderSummaryDialog";
import type { CurrentContext } from "../types";

export default function PetReminderNotifications({ hidden, ready, unobscured, stateBubble, context }: {
  hidden: boolean; ready: boolean; unobscured: boolean; stateBubble?: ReactNode;
  context?: Pick<CurrentContext, "personaId" | "contextRevision">;
}) {
  const notifications = useReminderNotifications();
  const { notice, pendingIds, shown, busy, error } = notifications;
  const awakened = useRef(new Set<string>());
  const restoring = useRef(false);
  const attemptedDisplays = useRef(new Set<string>());
  const [restoreAttempt, setRestoreAttempt] = useState(0);
  const opening = useRef(false);
  const [openingPage, setOpeningPage] = useState(false);
  const [documentVisible, setDocumentVisible] = useState(document.visibilityState !== "hidden");
  const visible = ready && !hidden && unobscured && documentVisible;
  const key = noticeKey(notice);
  useEffect(() => {
    const update = () => setDocumentVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);
  useEffect(() => {
    if (!ready || restoring.current) return;
    const fresh = pendingIds.filter(id => !awakened.current.has(id));
    if (!fresh.length) return;
    fresh.forEach(id => awakened.current.add(id));
    if (!hidden) return;
    restoring.current = true;
    void invoke("set_pet_hidden", { hidden: false }).catch(notifications.reportError).finally(() => { restoring.current = false; });
    // 每批新通知只恢复一次，用户随后手动隐藏不会陷入反复恢复。
  }, [hidden, ready, pendingIds, restoreAttempt]);
  useEffect(() => {
    if (!visible || !key || busy || attemptedDisplays.current.has(key)) return;
    let second = 0;
    const first = requestAnimationFrame(() => { second = requestAnimationFrame(() => { attemptedDisplays.current.add(key); void shown(); }); });
    return () => { cancelAnimationFrame(first); cancelAnimationFrame(second); };
  }, [visible, key, shown, busy]);
  const open = async () => {
    if (opening.current) return;
    opening.current = true; setOpeningPage(true);
    try {
      await navigation.open("reminders");
      await notifications.dismiss(key);
    } catch (cause) { notifications.reportError(cause); }
    finally { opening.current = false; setOpeningPage(false); }
  };
  const retry = () => {
    notifications.reportError("");
    awakened.current.clear(); setRestoreAttempt(value => value + 1);
    void notifications.refresh();
    if (visible) void shown();
  };
  if (!visible) return null;
  return <div className="pet-reminder-slot">
    {/* 提醒优先；提示组件被卸载后丢弃内容，不在提醒收起后重放。 */}
    {!notice && !error && stateBubble}
    {notice?.kind === "due" && <ReminderBubble occurrence={notice.item} context={context} busy={busy || openingPage} error={error}
      onComplete={() => { void notifications.complete(key); }} onDismiss={() => { void notifications.dismiss(key); }} onOpenReminders={() => { void open(); }} />}
    {notice?.kind === "missed" && <MissedReminderSummaryDialog occurrences={notice.items} busy={busy || openingPage} error={error}
      onDismiss={() => { void notifications.dismiss(key); }} onOpenReminders={() => { void open(); }} />}
    {error && <button className="ym-button pet-reminder-retry" data-pet-interactive disabled={busy} onClick={retry} title={error}>重试提醒{!notice && `：${error}`}</button>}
  </div>;
}
