import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, petApi } from "../api";
import type { PetAction, PetActivity } from "../types";

export function usePetActivity() {
  const [view, setView] = useState<PetActivity | null>(null);
  const current = useRef<PetActivity | null>(null);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const locked = useRef(false);
  const alive = useRef(true);
  const accept = useCallback((next: PetActivity) => {
    if (!alive.current || !next || (current.current && next.revision < current.current.revision)) return;
    current.current = next; setView(next); setError("");
  }, []);
  useEffect(() => {
    alive.current = true;
    let disposed = false;
    const stops: Array<() => void> = [];
    const add = (stop: () => void) => { if (disposed) stop(); else stops.push(stop); };
    void (async () => {
      try {
        add(await petApi.listen(next => { if (!disposed) accept(next); }));
        add(await petApi.listenError(cause => { if (!disposed) setError(errorMessage(cause)); }));
        const next = await petApi.get();
        if (!disposed) accept(next);
      } catch (cause) { if (!disposed) setError(errorMessage(cause)); }
    })();
    return () => { disposed = true; alive.current = false; stops.forEach(stop => stop()); };
  }, [accept]);
  const interact = async (action: PetAction) => {
    if (locked.current) return;
    const captured = current.current;
    if (!captured?.sessionId) { setError("当前还没有会话，创建会话后即可互动"); return; }
    if (!captured.canInteract) return;
    locked.current = true; setPending(true);
    try { accept(await petApi.interact(captured, action)); }
    catch (cause) {
      if(alive.current&&current.current?.contextRevision===captured.contextRevision){
        if((cause as {code?:string})?.code==="PET_AWAY"){try{accept(await petApi.get());}catch{/* 等待后续状态通知，不将正常出行显示为互动故障。 */}}
        else setError(errorMessage(cause));
      }
    }
    finally { locked.current = false; if (alive.current) setPending(false); }
  };
  const refresh = async () => {
    if (locked.current) return;
    locked.current = true; setPending(true);
    try { await petApi.refreshProvider(); accept(await petApi.get()); }
    catch (cause) {
      if (alive.current && (cause as { code?: string })?.code === "CONTEXT_CHANGED") {
        // 配置已更新时只读取新状态，不能把旧测试失败显示成新提供商故障。
        try { accept(await petApi.get()); }
        catch (readError) { if (alive.current) setError(errorMessage(readError)); }
      } else if (alive.current) setError(errorMessage(cause));
    }
    finally { locked.current = false; if (alive.current) setPending(false); }
  };
  return { view, error, pending, interact, refresh };
}
