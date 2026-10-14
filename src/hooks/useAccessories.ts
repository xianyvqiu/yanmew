import { useCallback, useEffect, useRef, useState } from "react";
import { accessoriesApi, errorMessage } from "../api";
import type { Accessory, CurrentContext } from "../types";

export const accessoryContextKey = (context: CurrentContext | null) => context
  ? `${context.personaId}:${context.sessionId ?? ""}:${context.contextRevision}` : "";

/** 列表分页与头顶单槽共用事件协议；头顶查询不能遗漏较早的佩戴项。 */
export function useAccessories(context: CurrentContext | null, wornOnly = false) {
  const key = accessoryContextKey(context);
  const [result, setResult] = useState<{ key: string; items: Accessory[]; more: boolean }>({ key: "", items: [], more: false });
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const reload = useRef<() => Promise<void>>(async () => {});
  const nextPage = useRef<() => Promise<void>>(async () => {});
  useEffect(() => {
    let disposed = false;
    let generation = 0;
    let items: Accessory[] = [];
    let pageDepth = 1;
    let pageBusy = false;
    const stops: (() => void)[] = [];
    setError(""); setLoading(Boolean(context?.sessionId));
    const read = async (append = false) => {
      if (disposed || !context?.sessionId || (append && pageBusy)) return;
      const token = ++generation;
      pageBusy = true; setLoading(true);
      try {
        let beforeId = append ? items[items.length - 1]?.id ?? null : null;
        let rows: Accessory[];
        let collected: Accessory[] = [];
        let page = 0;
        do {
          rows = await accessoriesApi.list(context, beforeId, wornOnly ? 200 : 30);
          if (disposed || token !== generation) return;
          collected.push(...(wornOnly ? rows.filter(row => row.isWorn) : rows));
          page++;
          const next = rows[rows.length - 1]?.id ?? null;
          // 接口必须按 ID 倒序推进；异常分页不能形成无限查询。
          if (next === beforeId) break;
          beforeId = next;
        } while (wornOnly ? !collected.length && rows.length === 200 : !append && page < pageDepth && rows.length === 30);
        if (append && !wornOnly) pageDepth++;
        const combined = append ? [...items, ...collected] : collected;
        items = combined.filter((item, index) => combined.findIndex(row => row.id === item.id) === index);
        setResult({ key, items, more: !wornOnly && rows.length === 30 }); setError("");
      } catch (cause) { if (!disposed && token === generation) setError(errorMessage(cause)); }
      finally { if (!disposed && token === generation) { pageBusy = false; setLoading(false); } }
    };
    const refresh = () => read();
    reload.current = refresh; nextPage.current = () => read(true);
    const focus = () => { void refresh(); };
    void (async () => {
      try {
        const stop = await accessoriesApi.listen(change => { if (change.sessionId === context?.sessionId) void refresh(); });
        if (disposed) { stop(); return; } stops.push(stop);
      } catch (cause) { if (!disposed) setError(`饰品通知暂不可用：${errorMessage(cause)}`); }
      if (!disposed) { window.addEventListener("focus", focus); await refresh(); }
    })();
    return () => { disposed = true; generation++; stops.forEach(stop => stop()); window.removeEventListener("focus", focus); };
  }, [key, wornOnly]);
  return { items: result.key === key ? result.items : [], more: result.key === key && result.more,
    loading, error, refresh: useCallback(() => reload.current(), []), loadMore: useCallback(() => nextPage.current(), []) };
}
