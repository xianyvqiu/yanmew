// 浏览器开发模拟器，只模拟私有存档、设置和运行；不修改颜喵钱包，不代发真实网络。
export function createMockHost({ id, version = "1.0.0", settings = {}, storage = window.localStorage } = {}) {
  if (!id) throw new Error("模拟宿主需要插件 ID");
  const prefix = `yanmew.mock.${id}.`;
  const run = { id: crypto.randomUUID(), pluginId: id, pluginName: id, pluginVersion: version, walletPermissionGranted: false, status: "running", outcome: null, details: null, coinDelta: "0", startedAt: new Date().toISOString(), endedAt: null };
  const context = { language: "zh-CN", gameRunId: run.id, walletPermissionGranted: false, pluginVersion: version, permissions: [], settings: structuredClone(settings) };
  const key = value => { if (!/^[a-z0-9_-]+(\.[a-z0-9_-]+)*$/.test(value) || value.length > 128) throw new Error("存档键无效"); return prefix + value; };
  const denied = () => Promise.reject({ code: "PLUGIN_PERMISSION_DENIED", message: "模拟宿主不开放喵币和联网；这些能力须在颜喵中验收" });
  return {
    game: {
      ready: async () => {}, getContext: async () => structuredClone(context),
      random: async () => crypto.getRandomValues(new Uint32Array(1))[0] / 2 ** 32,
      complete: async (outcome, details = null) => {
        if (run.status === "completed") { if (run.outcome !== outcome || JSON.stringify(run.details) !== JSON.stringify(details)) throw new Error("运行已结束，不能覆盖结果"); return structuredClone(run); }
        run.status = "completed"; run.outcome = outcome; run.details = structuredClone(details); run.endedAt = new Date().toISOString(); return structuredClone(run);
      }, close: async () => { if (run.status === "running") { run.status = "cancelled"; run.endedAt = new Date().toISOString(); } },
    },
    storage: { get: async k => JSON.parse(storage.getItem(key(k)) ?? "null"), set: async (k, v) => storage.setItem(key(k), JSON.stringify(v)), delete: async k => storage.removeItem(key(k)) },
    wallet: { getBalance: denied, changeCoins: denied }, network: { fetch: denied },
  };
}
