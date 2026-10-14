import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, lettersApi, petApi, travelApi } from "../api";
import type { CurrentContext, LetterGenerationStatus, Travel, TravelGenerationStatus } from "../types";

export const travelContextKey = (context: CurrentContext) => `${context.personaId}:${context.sessionId ?? "none"}:${context.contextRevision}`;
const emptyFacts: TravelGenerationStatus = { active: null, failures: [] };
const emptyLetters: LetterGenerationStatus = { active: null, failures: [], notice: null };

/** 固定当前上下文，先订阅再查询；聚焦补查，迟到结果不能替换另一会话。 */
export function useTravel(context: CurrentContext) {
  const key = travelContextKey(context);
  const [current, setCurrent] = useState<Travel | null>(null);
  const [facts, setFacts] = useState(emptyFacts);
  const [letters, setLetters] = useState(emptyLetters);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const loadRef = useRef<() => Promise<void>>(async () => {});
  const refresh = useCallback(() => loadRef.current(), []);
  useEffect(() => {
    let disposed = false;
    let generation = 0;
    const stops: (() => void)[] = [];
    setCurrent(null); setFacts(emptyFacts); setLetters(emptyLetters); setError("");
    const load = async () => {
      const token = ++generation;
      if (!context.sessionId) { setLoading(false); return; }
      setLoading(true);
      try {
        const [trip, factsStatus, letterStatus] = await Promise.all([
          travelApi.current(context), travelApi.status(context), lettersApi.status(context),
        ]);
        if (disposed || token !== generation) return;
        setCurrent(trip); setFacts(factsStatus); setLetters(letterStatus); setError("");
        setVersion(value => value + 1);
      } catch (cause) { if (!disposed && token === generation) setError(errorMessage(cause)); }
      finally { if (!disposed && token === generation) setLoading(false); }
    };
    loadRef.current = load;
    const reload = () => { if (!disposed) void load(); };
    const register = (stop: () => void) => { if (disposed) stop(); else stops.push(stop); };
    void (async () => {
      try {
        register(await travelApi.listen(event => { if (event.sessionId === context.sessionId) reload(); }));
        register(await lettersApi.listen(event => { if (event.sessionId === context.sessionId) reload(); }));
        register(await petApi.listen(view => {
          if (view.personaId === context.personaId && view.sessionId === context.sessionId && view.contextRevision === context.contextRevision) reload();
        }));
      } catch (cause) { if (!disposed) setError(`状态通知连接失败：${errorMessage(cause)}`); }
      if (!disposed) { window.addEventListener("focus", reload); await load(); }
    })();
    return () => { disposed = true; generation++; stops.forEach(stop => stop()); window.removeEventListener("focus", reload); };
  }, [key]);
  return { current, facts, letters, loading, error, version, refresh };
}
