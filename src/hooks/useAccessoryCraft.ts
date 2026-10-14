import { useCallback, useEffect, useRef, useState } from "react";
import { accessoryCraftApi, errorMessage, walletApi } from "../api";
import type { AccessoryCraftPreview, CurrentContext, Wallet } from "../types";
import { accessoryContextKey } from "./useAccessories";

/** 候选由后端产生；通知只触发补查，不把事件当成购买结果。 */
export function useAccessoryCraft(context: CurrentContext) {
  const key = accessoryContextKey(context);
  const [view, setView] = useState<{ key: string; preview: AccessoryCraftPreview | null }>({ key: "", preview: null });
  const [wallet, setWallet] = useState<Wallet | null>(null);
  const [error, setError] = useState("");
  const [actionError, setActionError] = useState("");
  const [walletError, setWalletError] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const methods = useRef<{ refresh: () => Promise<void>; act: (buy: boolean, id: string) => Promise<void> }>({ refresh: async () => {}, act: async () => {} });
  useEffect(() => {
    let disposed = false;
    let generation = 0;
    let walletGeneration = 0;
    let locked = false;
    const stops: (() => void)[] = [];
    setError(""); setActionError(""); setWalletError(""); setNotice(""); setBusy(false); setWallet(null);
    const readWallet = async () => {
      const token = ++walletGeneration;
      try { const next = await walletApi.get(); if (!disposed && token === walletGeneration) { setWallet(next); setWalletError(""); } }
      catch (cause) { if (!disposed && token === walletGeneration) { setWallet(null); setWalletError(`余额读取失败：${errorMessage(cause)}`); } }
    };
    const read = async () => {
      if (disposed || locked || !context.sessionId) return;
      const token = ++generation;
      try {
        const preview = await accessoryCraftApi.current(context);
        if (!disposed && token === generation) { setView({ key, preview }); setError(""); }
      } catch (cause) { if (!disposed && token === generation) setError(errorMessage(cause)); }
    };
    const refresh = async () => { await Promise.all([read(), readWallet()]); };
    const act = async (buy: boolean, id: string) => {
      if (disposed || locked || !context.sessionId) return;
      locked = true; generation++; walletGeneration++; setBusy(true); setActionError(""); setNotice("");
      try {
        if (buy) await accessoryCraftApi.confirm(context, id); else await accessoryCraftApi.cancel(context, id);
        if (!disposed) { setView({ key, preview: null }); setNotice(buy ? "已制作，前往饰品页佩戴。" : "已取消制作。"); }
      } catch (cause) { if (!disposed) setActionError(errorMessage(cause)); }
      finally {
        locked = false;
        // 缺图或报价失效后读取权威候选，必须再次点击才能确认，绝不自动重试提交。
        if (!disposed) { await refresh(); if (!disposed) setBusy(false); }
      }
    };
    methods.current = { refresh, act };
    const focus = () => { void refresh(); };
    void (async () => {
      for (const subscribe of [() => accessoryCraftApi.listen(change => { if (change.sessionId === context.sessionId) void read(); }), () => walletApi.listen(() => { void readWallet(); })]) {
        try { const stop = await subscribe(); if (disposed) { stop(); return; } stops.push(stop); }
        catch (cause) { if (!disposed) setError(`变更通知暂不可用：${errorMessage(cause)}`); }
      }
      if (!disposed) { window.addEventListener("focus", focus); await refresh(); }
    })();
    return () => { disposed = true; generation++; walletGeneration++; stops.forEach(stop => stop()); window.removeEventListener("focus", focus); };
  }, [key]);
  return { preview: view.key === key ? view.preview : null, wallet, busy, error: actionError || error || walletError, notice,
    refresh: useCallback(() => { setActionError(""); return methods.current.refresh(); }, []),
    confirm: useCallback((id: string) => methods.current.act(true, id), []),
    cancel: useCallback((id: string) => methods.current.act(false, id), []) };
}
