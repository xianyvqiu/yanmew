import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, reminderApi } from "../api";
import type { ReminderOccurrence } from "../types";
import { readMissedReminders, readQueuedReminders } from "./reminderQueries";

export type ReminderNotice = { kind: "due"; item: ReminderOccurrence } | { kind: "missed"; items: ReminderOccurrence[] };
export const noticeKey = (notice: ReminderNotice | null) => !notice ? "" : notice.kind === "due" ? `due:${notice.item.id}` : `missed:${notice.items.map(item => item.id).join(",")}`;
const entries = (notice: ReminderNotice) => notice.kind === "due" ? [notice.item] : notice.items;

/** 只有主宠窗挂载此投递控制器；数据库为恢复依据，当前气泡快照保留到用户收起。 */
export default function useReminderNotifications() {
  const [notice, setNotice] = useState<ReminderNotice | null>(null);
  const [pendingIds, setPendingIds] = useState<string[]>([]);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const alive = useRef(false);
  const sequence = useRef(0);
  const active = useRef<ReminderNotice | null>(null);
  const acknowledged = useRef(new Set<string>());
  const dismissed = useRef(new Set<string>());
  const locked = useRef(false);
  const publish = useCallback((value: ReminderNotice | null) => { active.current = value; setNotice(value); }, []);

  const refresh = useCallback(async () => {
    const token = ++sequence.current;
    const current = active.current;
    try {
      const currentRecord = current?.kind === "due" ? reminderApi.getOccurrence(current.item.id).catch(cause => {
        if (cause?.code === "NOT_FOUND") return null;
        throw cause;
      }) : Promise.resolve(null);
      const [queued, missed, record] = await Promise.all([readQueuedReminders(), readMissedReminders(), currentRecord]);
      if (!alive.current || sequence.current !== token) return;
      const due = queued.filter(item => !dismissed.current.has(item.id));
      const absent = missed.filter(item => !dismissed.current.has(item.id));
      setPendingIds([...due, ...absent].map(item => item.id));
      if (current?.kind === "missed") return; // 已展示的汇总保留原快照，不因自身确认事件消失。
      if (current?.kind === "due" && record?.status === "pending") {
        publish({ kind: "due", item: record });
        return;
      }
      if (current?.kind === "due") dismissed.current.add(current.item.id);
      const next = due.find(item => !dismissed.current.has(item.id));
      if (next) publish({ kind: "due", item: next });
      else publish(absent.length ? { kind: "missed", items: absent } : null);
    } catch (cause) { if (alive.current && sequence.current === token) setError(errorMessage(cause)); }
  }, [publish]);

  useEffect(() => {
    alive.current = true;
    let disposed = false;
    const stops: (() => void)[] = [];
    const reload = () => { if (!disposed) void refresh(); };
    void (async () => {
      try {
        for (const subscribe of [() => reminderApi.listen(reload), () => reminderApi.listenError(cause => { if (!disposed) setError(errorMessage(cause)); })]) {
          const stop = await subscribe();
          if (disposed) { stop(); return; }
          stops.push(stop);
        }
      } catch (cause) { if (!disposed) setError(errorMessage(cause)); }
      reload();
    })();
    window.addEventListener("focus", reload);
    return () => { disposed = true; alive.current = false; sequence.current++; stops.forEach(stop => stop()); window.removeEventListener("focus", reload); };
  }, [refresh]);

  const acknowledge = useCallback(async (snapshot: ReminderNotice) => {
    const ids = entries(snapshot).map(item => item.id).filter(id => !acknowledged.current.has(id));
    for (let offset = 0; offset < ids.length; offset += 200) {
      const batch = ids.slice(offset, offset + 200);
      if (snapshot.kind === "missed") await reminderApi.acknowledgeMissed(batch);
      else await reminderApi.acknowledge(batch, "queued");
      batch.forEach(id => acknowledged.current.add(id));
    }
  }, []);

  const shown = useCallback(async () => {
    const snapshot = active.current;
    if (!snapshot || locked.current || entries(snapshot).every(item => acknowledged.current.has(item.id))) return;
    locked.current = true; setBusy(true);
    try { await acknowledge(snapshot); if (alive.current) setError(""); }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  }, [acknowledge]);

  const close = async (complete: boolean, expectedKey?: string) => {
    const snapshot = active.current;
    if (!snapshot || locked.current || (expectedKey !== undefined && noticeKey(snapshot) !== expectedKey)) return false;
    locked.current = true; setBusy(true); setError("");
    try {
      await acknowledge(snapshot);
      if (complete && snapshot.kind === "due") await reminderApi.complete(snapshot.item.id);
      entries(snapshot).forEach(item => dismissed.current.add(item.id));
      if (alive.current) {
        // 写入时的通知查询可能较晚返回，不能重新显示刚刚收起的气泡。
        sequence.current++; publish(null); await refresh();
      }
      return true;
    } catch (cause) { if (alive.current) setError(errorMessage(cause)); return false; }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  return { notice, pendingIds, error, busy, shown, refresh,
    dismiss: (expectedKey?: string) => close(false, expectedKey), complete: (expectedKey?: string) => close(true, expectedKey),
    reportError: (cause: unknown) => { if (alive.current) setError(errorMessage(cause)); },
  };
}
