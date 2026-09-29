import { useCallback, useEffect, useState } from "react";
import { configurationApi, errorMessage, sessionApi } from "../api";
import type { CurrentContext, Persona, Session } from "../types";

export type WorkspaceContext = { context: CurrentContext; persona: Persona; sessions: Session[] };

/** 先订阅后查询，所有关联数据按同一上下文版本发布，窗口聚焦补偿丢失事件。 */
export function useCurrentContext() {
  const [snapshot, setSnapshot] = useState<WorkspaceContext | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const refresh = useCallback(() => setAttempt(value => value + 1), []);
  useEffect(() => {
    let disposed = false;
    let generation = 0;
    let newestRevision = -1;
    const stops: (() => void)[] = [];
    const load = async (clear = true) => {
      const token = ++generation;
      if (clear) setSnapshot(null);
      setLoading(true); setError("");
      try {
        const context = await configurationApi.getCurrentContext();
        if (disposed || token !== generation) return;
        newestRevision = Math.max(newestRevision, context.contextRevision);
        const [personas, sessions] = await Promise.all([configurationApi.listPersonas(), sessionApi.list(context)]);
        const check = await configurationApi.getCurrentContext();
        if (disposed || token !== generation) return;
        if (check.contextRevision !== context.contextRevision || context.contextRevision < newestRevision) {
          setError("当前人设或会话已改变，请刷新后重试"); return;
        }
        const persona = personas.find(item => item.id === context.personaId);
        if (!persona) throw new Error("当前人设不存在，请刷新后重试");
        setSnapshot({ context, persona, sessions });
      } catch (cause) { if (!disposed && token === generation) setError(errorMessage(cause)); }
      finally { if (!disposed && token === generation) setLoading(false); }
    };
    const focus = () => { void load(false); };
    void (async () => {
      try {
        const stopContext = await configurationApi.listenContext(context => {
          if (disposed || context.contextRevision < newestRevision) return;
          newestRevision = context.contextRevision; void load();
        });
        if (disposed) { stopContext(); return; } stops.push(stopContext);
        const stopChanges = await configurationApi.listenChanges(() => { if (!disposed) void load(false); });
        if (disposed) { stopChanges(); return; } stops.push(stopChanges);
        window.addEventListener("focus", focus);
        await load(false);
      } catch (cause) { if (!disposed) { setError(errorMessage(cause)); setLoading(false); } }
    })();
    return () => { disposed = true; generation++; stops.forEach(stop => stop()); window.removeEventListener("focus", focus); };
  }, [attempt]);
  return { snapshot, loading, error, refresh };
}
