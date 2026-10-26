/** 协议 1.0.0；所有存档全局按插件隔离，不包含人设、会话或主题。 */
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
/** 可选管理页摘要：先保存实际进度，再通过 storage.set("ui.summary", summary) 更新；最多三项纯文本指标。 */
export type SaveSummary = { version: 1; status?: "saved" | "playing" | "completed"; metrics?: { label: string; value: string }[] };
export type Permission = { permission: "wallet.modify" | "network.fetch"; scope: string; decision: "pending" | "allowed" | "denied"; pluginVersion: string; decidedAt: string | null };
export type Context = { readonly language: "zh-CN"; readonly gameRunId: string; readonly walletPermissionGranted: boolean; readonly pluginVersion: string; readonly permissions: readonly Permission[]; readonly settings: Readonly<Record<string, Json>> };
export type WalletRequest = { request_id: string; game_run_id: string; delta: number; reason: string; details?: Json };
export type WalletResult = { ledgerId: string; appliedDelta: string; duplicate: boolean; balance: string };
export type Run = { id: string; pluginId: string; pluginName: string; pluginVersion: string; walletPermissionGranted: boolean; status: "running" | "completed" | "cancelled" | "error"; outcome: string | null; details: Json; coinDelta: string; startedAt: string; endedAt: string | null };
export interface Yanmew {
  game: {
    ready(): Promise<void>;
    getContext(): Promise<Context>;
    random(): Promise<number>;
    /** 可选：标记本次打开的运行已完成；完成后不能再提交新的钱包变化。长期进度独立保存在 storage。 */
    complete(outcome: string, details?: Json): Promise<Run>;
    close(): Promise<void>;
  };
  storage: { get(key: string): Promise<Json>; set(key: string, value: Json): Promise<void>; delete(key: string): Promise<void> };
  wallet: { getBalance(): Promise<{ coins: string; updatedAt: string }>; changeCoins(request: WalletRequest): Promise<WalletResult> };
  network: { fetch(request: { url: string; method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"; headers?: Record<string, string>; body?: string | null }): Promise<Response> };
}
export type PrivateSnapshot = { storage_version: number; settings: Record<string, Json>; data: Record<string, Json> };
/** 开发者导出此函数，支持所有仍需兼容的旧版本；返回完整目标数据，不修改原对象。 */
export type Migrate = (snapshot: PrivateSnapshot, targetVersion: number) => PrivateSnapshot | Promise<PrivateSnapshot>;
declare global { interface Window { readonly yanmew: Yanmew } }
