import { readMissedReminders } from "./reminderQueries";
import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, reminderApi } from "../api";
import type { Reminder, ReminderOccurrence } from "../types";

const pageSize = 50;

export default function useReminders({ complete = false }: { complete?: boolean } = {}) {
  const [rules, setRules] = useState<Reminder[]>([]);
  const [occurrences, setOccurrences] = useState<ReminderOccurrence[]>([]);
  const [missed, setMissed] = useState<ReminderOccurrence[]>([]);
  const [selectedId, setSelectedId] = useState<string>("");
  const [hasMoreRules, setHasMoreRules] = useState(false);
  const [hasMoreOccurrences, setHasMoreOccurrences] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const mounted = useRef(false);
  const version = useRef(0);
  const selection = useRef("");
  const rulePages = useRef(1);
  const occurrencePages = useRef(1);

  const refresh = useCallback(async () => {
    const token = ++version.current;
    const filter = selection.current;
    setLoading(true);
    try {
      // 管理页读取完整摘要后本地筛选分页；其他调用方仍保留原按需加载。
      const readRules = async () => {
        const items: Reminder[] = [];
        let after: string | null = null;
        let more = false;
        for (let page = 0; complete || page < rulePages.current; page++) {
          const batch = await reminderApi.list(after, pageSize);
          if (!mounted.current || version.current !== token) return { items: [], more: false };
          items.push(...batch);
          more = batch.length === pageSize;
          if (!more) break;
          const next = batch[batch.length - 1].id;
          if (next === after) throw new Error("提醒规则分页未前进，请重新读取。");
          after = next;
        }
        return { items, more };
      };
      const readOccurrences = async () => {
        const items: ReminderOccurrence[] = [];
        let after: string | undefined;
        let more = false;
        for (let page = 0; complete || page < occurrencePages.current; page++) {
          const batch = await reminderApi.listOccurrences({ reminderId: filter || undefined, afterId: after, limit: pageSize });
          if (!mounted.current || version.current !== token) return { items: [], more: false };
          items.push(...batch.items);
          more = batch.nextAfterId !== null;
          if (!more) break;
          if (batch.nextAfterId === after) throw new Error("提醒记录分页未前进，请重新读取。");
          after = batch.nextAfterId!;
        }
        return { items, more };
      };
      const [rulePage, occurrencePage, summary] = await Promise.all([readRules(), readOccurrences(), readMissedReminders()]);
      if (!mounted.current || version.current !== token) return;
      setRules(rulePage.items); setHasMoreRules(rulePage.more);
      setOccurrences(occurrencePage.items); setHasMoreOccurrences(occurrencePage.more);
      setMissed(summary); setError("");
    } catch (cause) {
      if (mounted.current && version.current === token) setError(errorMessage(cause));
    } finally {
      if (mounted.current && version.current === token) setLoading(false);
    }
  }, [complete]);

  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    const stops: (() => void)[] = [];
    const reload = () => { if (!disposed) void refresh(); };
    void (async () => {
      try {
        const stop = await reminderApi.listen(reload);
        if (disposed) { stop(); return; }
        stops.push(stop);
        const stopError = await reminderApi.listenError(cause => { if (!disposed) setError(errorMessage(cause)); });
        if (disposed) { stopError(); return; }
        stops.push(stopError);
      } catch (cause) { if (!disposed) setError(errorMessage(cause)); }
      reload();
    })();
    window.addEventListener("focus", reload);
    return () => { disposed = true; mounted.current = false; version.current++; stops.forEach(stop => stop()); window.removeEventListener("focus", reload); };
  }, [refresh]);

  const select = (id: string) => {
    selection.current = id; occurrencePages.current = 1;
    setSelectedId(id); setOccurrences([]); setHasMoreOccurrences(false);
    void refresh();
  };
  return { rules, occurrences, missed, selectedId, select, loading, error, refresh,
    hasMoreRules, hasMoreOccurrences,
    moreRules: () => { rulePages.current++; void refresh(); },
    moreOccurrences: () => { occurrencePages.current++; void refresh(); },
  };
}
