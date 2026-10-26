import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Reminder, ReminderOccurrence } from "../types";
import useReminders from "./useReminders";

const api = vi.hoisted(() => ({
  list: vi.fn(), listOccurrences: vi.fn(), missed: vi.fn(), listen: vi.fn(), listenError: vi.fn(), acknowledgeMissed: vi.fn(),
}));
vi.mock("../api", () => ({ reminderApi: api, errorMessage: (error: { message: string }) => error.message }));

const rule = (id: string, name: string): Reminder => ({
  id, name, content: "内容", kind: "once", schedule: null, fireAt: "2032-04-05T01:00:00.000Z",
  status: "pending", source: "manual", createdAt: "2032-01-01T00:00:00.000Z",
});
const occurrence = (id: string, reminderId: string, name: string): ReminderOccurrence => ({
  id, reminderId, name, content: "内容", scheduledAt: "2032-04-05T01:00:00.000Z", status: "expired",
  deliveryState: "missed", completedAt: null, createdAt: "2032-04-05T01:00:00.000Z",
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

let changed: (() => void) | undefined;
let listenError: ((error: unknown) => void) | undefined;
const stopChanged = vi.fn();
const stopError = vi.fn();
function emptyReads() {
  api.list.mockResolvedValue([]);
  api.listOccurrences.mockResolvedValue({ items: [], nextAfterId: null });
  api.missed.mockResolvedValue({ occurrences: [], nextAfterId: null, text: "" });
}

beforeEach(() => {
  vi.clearAllMocks(); changed = undefined; listenError = undefined;
  stopChanged.mockClear(); stopError.mockClear(); emptyReads();
  api.listen.mockImplementation(async (callback: typeof changed) => { changed = callback; return stopChanged; });
  api.listenError.mockImplementation(async (callback: typeof listenError) => { listenError = callback; return stopError; });
});
afterEach(cleanup);

it("读完多页离线汇总并按 occurrence ID 去重，读取过程不确认记录", async () => {
  const first = occurrence("occ-1", "rule-1", "喝水");
  const second = occurrence("occ-2", "rule-1", "喝水");
  api.missed.mockResolvedValueOnce({ occurrences: [first], nextAfterId: "occ-1", text: "" })
    .mockResolvedValueOnce({ occurrences: [first, second], nextAfterId: null, text: "" });
  const view = renderHook(() => useReminders());

  await waitFor(() => expect(view.result.current.missed).toHaveLength(2));
  expect(api.missed).toHaveBeenNthCalledWith(1, null);
  expect(api.missed).toHaveBeenNthCalledWith(2, "occ-1");
  expect(api.acknowledgeMissed).not.toHaveBeenCalled();
  expect(view.result.current.missed.map(item => item.id)).toEqual(["occ-1", "occ-2"]);
});

it("刷新保留已展开的页数并传递正确游标", async () => {
  const page = Array.from({ length: 50 }, (_, index) => rule(`r-${index + 1}`, `提醒${index + 1}`));
  api.list.mockImplementation(async (after: string | null) => after === null ? page : [rule("r-51", "提醒51")]);
  const view = renderHook(() => useReminders());
  await waitFor(() => expect(view.result.current.hasMoreRules).toBe(true));

  act(() => view.result.current.moreRules());
  await waitFor(() => expect(view.result.current.rules).toHaveLength(51));
  expect(api.list).toHaveBeenLastCalledWith("r-50", 50);
});

it("focus 和提醒变更事件都会重读，较慢的旧结果不能覆盖较新的数据", async () => {
  const view = renderHook(() => useReminders());
  await waitFor(() => expect(api.list).toHaveBeenCalled());
  await waitFor(() => expect(changed).toBeTypeOf("function"));
  await waitFor(() => expect(listenError).toBeTypeOf("function"));

  const oldRules = deferred<Reminder[]>();
  const oldOccurrences = deferred<{ items: ReminderOccurrence[]; nextAfterId: string | null }>();
  const oldMissed = deferred<{ occurrences: ReminderOccurrence[]; nextAfterId: string | null; text: string }>();
  api.list.mockReturnValueOnce(oldRules.promise);
  api.listOccurrences.mockReturnValueOnce(oldOccurrences.promise);
  api.missed.mockReturnValueOnce(oldMissed.promise);
  act(() => window.dispatchEvent(new Event("focus")));

  api.list.mockResolvedValueOnce([rule("new-rule", "新提醒")]);
  api.listOccurrences.mockResolvedValueOnce({ items: [occurrence("new-occ", "new-rule", "新提醒")], nextAfterId: null });
  api.missed.mockResolvedValueOnce({ occurrences: [], nextAfterId: null, text: "" });
  act(() => changed?.());
  await waitFor(() => expect(view.result.current.rules[0]?.name).toBe("新提醒"));

  await act(async () => {
    oldRules.resolve([rule("old-rule", "舊提醒")]);
    oldOccurrences.resolve({ items: [occurrence("old-occ", "old-rule", "舊提醒")], nextAfterId: null });
    oldMissed.resolve({ occurrences: [occurrence("old-missed", "old-rule", "舊提醒")], nextAfterId: null, text: "" });
    await Promise.resolve();
  });
  expect(view.result.current.rules.map(item => item.id)).toEqual(["new-rule"]);
  expect(view.result.current.occurrences.map(item => item.id)).toEqual(["new-occ"]);
  expect(view.result.current.missed).toEqual([]);
});

it("卸载时释放事件监听并移除 focus 监听", async () => {
  const remove = vi.spyOn(window, "removeEventListener");
  const view = renderHook(() => useReminders());
  await waitFor(() => expect(listenError).toBeTypeOf("function"));

  view.unmount();

  expect(stopChanged).toHaveBeenCalledTimes(1);
  expect(stopError).toHaveBeenCalledTimes(1);
  expect(remove).toHaveBeenCalledWith("focus", expect.any(Function));
});


it("完整管理列表自动读完50条以后的规则和记录，不需点击加载更多",async()=>{
 const first=Array.from({length:50},(_,i)=>rule(String(i+1),`提醒${i+1}`));
 api.list.mockImplementation(async(after:string|null)=>after===null?first:[rule("51","末页保险")]);
 api.listOccurrences.mockImplementation(async(options:{afterId?:string})=>options.afterId?{items:[occurrence("o51","51","末页保险")],nextAfterId:null}:{items:[occurrence("o1","1","喝水")],nextAfterId:"o1"});
 const view=renderHook(()=>useReminders({complete:true}));
 await waitFor(()=>expect(view.result.current.loading).toBe(false));
 expect(view.result.current.rules).toHaveLength(51);
 expect(view.result.current.occurrences.map(item=>item.id)).toEqual(["o1","o51"]);
 expect(view.result.current.hasMoreRules).toBe(false);expect(view.result.current.hasMoreOccurrences).toBe(false);
 expect(api.list).toHaveBeenLastCalledWith("50",50);
 expect(api.listOccurrences).toHaveBeenLastCalledWith({reminderId:undefined,afterId:"o1",limit:50});
});

it("完整读取遇到重复游标时停止并提示，不把部分数据当成全部",async()=>{
 const first=Array.from({length:50},(_,i)=>rule(String(i+1),`提醒${i+1}`));
 api.list.mockResolvedValue(first);
 const view=renderHook(()=>useReminders({complete:true}));
 await waitFor(()=>expect(view.result.current.loading).toBe(false));
 expect(view.result.current.error).toBe("提醒规则分页未前进，请重新读取。");
 expect(api.list).toHaveBeenCalledTimes(2);expect(view.result.current.rules).toEqual([]);
});
