import { invoke } from "@tauri-apps/api/core";
export const ledgerReasons: Record<string, string> = { daily_sign_in: "签到", sign_in_festival: "节日奖励", food_purchase: "食物购买", accessory_craft: "饰品制作", accessory_sell: "饰品卖出", plugin_game: "小游戏" };
export type LedgerQuery = { start: string | null; end: string | null; direction: "all" | "income" | "expense"; reason: string; pluginId: string; search: string; ascending: boolean; page: number };
export type LedgerSummary = { income: string; expense: string; net: string; count: number };
export type LedgerGroup = { key: string; name: string; summary: LedgerSummary };
export type LedgerEntry = { id: string; delta: string; reason: string; pluginId: string | null; pluginName: string | null; pluginVersion: string | null; description: string | null; refId: string | null; createdAt: string };
export type LedgerReport = { entries: LedgerEntry[]; summary: LedgerSummary; balance: string; totalPages: number; page: number; categories: LedgerGroup[]; games: LedgerGroup[]; plugins: { id: string; name: string }[]; timeline: LedgerGroup[]; bucket: string; rangeStart: string; rangeEnd: string };
export const ledgerApi = { query: (input: LedgerQuery) => invoke<LedgerReport>("query_currency_ledger", { input }) };
export const coinText = (value: string) => BigInt(value).toLocaleString("zh-CN");
const localDay = (date: Date) => `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,"0")}-${String(date.getDate()).padStart(2,"0")}`;
export function ledgerRange(preset: string, now = new Date()): {start:string|null;end:string|null} {
    if (preset === "all") return { start:null,end:null };
    const end = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const start = new Date(end);
    if (preset === "yesterday") { start.setDate(start.getDate()-1); return {start:localDay(start),end:localDay(start)}; }
    if (preset === "week") start.setDate(start.getDate()-6);
    else {
        const months = preset === "month" ? 1 : preset === "halfYear" ? 6 : 12;
        // 月末向前推按目标月最后一天钳制，不能让2月溢出到3月。
        const day = start.getDate(); start.setDate(1); start.setMonth(start.getMonth()-months);
        start.setDate(Math.min(day,new Date(start.getFullYear(),start.getMonth()+1,0).getDate()));
        start.setDate(start.getDate()+1);
    }
    return {start:localDay(start),end:localDay(end)};
}
