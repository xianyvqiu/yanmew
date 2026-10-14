// 原生宿主注入；插件仅使用 window.yanmew。身份与权限均由 Rust 实例映射决定。
(() => {
  const invoke = (method, args = {}) => window.__TAURI_INTERNALS__.invoke("game_plugin_call", { method, args });
  const freeze = value => { if (value && typeof value === "object") { Object.values(value).forEach(freeze); Object.freeze(value); } return value; };
  const api = {
    game: {
      ready: () => invoke("game.ready"),
      getContext: async () => freeze(await invoke("game.getContext")),
      random: () => invoke("game.random"),
      complete: (outcome, details = null) => invoke("game.complete", { outcome, details }),
      close: () => invoke("game.close"),
    },
    storage: {
      get: key => invoke("storage.get", { key }),
      set: (key, value) => invoke("storage.set", { key, value }),
      delete: key => invoke("storage.delete", { key }),
    },
    wallet: {
      getBalance: () => invoke("wallet.getBalance"),
      changeCoins: request => invoke("wallet.changeCoins", request),
    },
    network: {
      fetch: async request => {
        const result = await invoke("network.fetch", request);
        const bytes = Uint8Array.from(atob(result.bodyBase64), c => c.charCodeAt(0));
        return new Response([101, 204, 205, 304].includes(result.status) ? null : bytes, { status: result.status, headers: result.headers });
      },
    },
  };
  Object.defineProperty(window, "yanmew", { value: freeze(api), configurable: false, writable: false });
})();
