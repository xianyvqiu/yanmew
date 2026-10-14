import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, gamePluginsApi, walletApi } from "../api";
import type { GamePlugin, Wallet } from "../types";

/** 全局插件列表与钱包，不绑定当前人设或会话。 */
export function useGamePlugins() {
  const [plugins, setPlugins] = useState<GamePlugin[]>([]);
  const [wallet, setWallet] = useState<Wallet | null>(null);
  const [error, setError] = useState("");
  const [walletError, setWalletError] = useState("");
  const [loading, setLoading] = useState(true);
  const generation = useRef(0);
  const walletGeneration = useRef(0);
  const readWallet = useCallback(async () => {
    const token = ++walletGeneration.current;
    try {
      const value = await walletApi.get();
      if (token === walletGeneration.current) { setWallet(value); setWalletError(""); }
    } catch (cause) { if (token === walletGeneration.current) setWalletError(errorMessage(cause)); }
  }, []);
  const refresh = useCallback(async () => {
    const token = ++generation.current;
    try {
      const [items] = await Promise.all([gamePluginsApi.list(), readWallet()]);
      if (token === generation.current) { setPlugins(items); setError(""); }
    } catch (cause) { if (token === generation.current) setError(errorMessage(cause)); }
    finally { if (token === generation.current) setLoading(false); }
  }, [readWallet]);
  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const reload = () => { if (!disposed) void refresh(); };
    for (const [subscribe, receive] of [[gamePluginsApi.listen, reload], [walletApi.listen, () => { if (!disposed) void readWallet(); }]] as const) {
      void subscribe(receive).then(stop => { if (disposed) stop(); else stops.push(stop); })
        .catch(cause => { if (!disposed) setError(errorMessage(cause)); });
    }
    reload(); window.addEventListener("focus", reload);
    return () => { disposed = true; generation.current++; walletGeneration.current++; stops.forEach(stop => stop()); window.removeEventListener("focus", reload); };
  }, [refresh, readWallet]);
  return { plugins, wallet, error: error || walletError, loading, refresh };
}
