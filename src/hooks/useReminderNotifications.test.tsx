import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReminderOccurrence } from "../types";
import useReminderNotifications from "./useReminderNotifications";

const api = vi.hoisted(() => ({
  getOccurrence: vi.fn(), listOccurrences: vi.fn(), missed: vi.fn(),
  acknowledge: vi.fn(), acknowledgeMissed: vi.fn(), complete: vi.fn(),
  listen: vi.fn(), listenError: vi.fn(),
}));

vi.mock("../api", () => ({
  reminderApi: api,
  errorMessage: (cause: unknown) => cause instanceof Error ? cause.message : "未知错误",
}));

const occurrence = (id: string, deliveryState: "queued" | "missed" = "queued"): ReminderOccurrence => ({
  id, reminderId: `reminder-${id}`, name: `提醒 ${id}`, content: "提醒正文",
  scheduledAt: "2032-04-05T01:00:00.000Z", status: deliveryState === "missed" ? "expired" : "pending",
  deliveryState, completedAt: null, createdAt: "2032-04-04T01:00:00.000Z",
});

const queuedPage = (items: ReminderOccurrence[]) => ({ items, nextAfterId: null });
const missedPage = (occurrences: ReminderOccurrence[], nextAfterId: string | null = null) => ({ occurrences, nextAfterId, text: "" });

let changed: (() => void) | undefined;
let reportError: ((cause: unknown) => void) | undefined;
const stopChanged = vi.fn();
const stopError = vi.fn();

function defer<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

async function waitForInitialRead() {
  await waitFor(() => expect(api.listOccurrences).toHaveBeenCalled());
  await waitFor(() => expect(changed).toBeTypeOf("function"));
  await waitFor(() => expect(reportError).toBeTypeOf("function"));
}

beforeEach(() => {
  vi.clearAllMocks();
  changed = undefined;
  reportError = undefined;
  api.getOccurrence.mockImplementation(async (id: string) => occurrence(id));
  api.listOccurrences.mockResolvedValue(queuedPage([]));
  api.missed.mockResolvedValue(missedPage([]));
  api.acknowledge.mockResolvedValue(undefined);
  api.acknowledgeMissed.mockResolvedValue(undefined);
  api.complete.mockResolvedValue(undefined);
  api.listen.mockImplementation(async (receive: () => void) => { changed = receive; return stopChanged; });
  api.listenError.mockImplementation(async (receive: (cause: unknown) => void) => { reportError = receive; return stopError; });
});

afterEach(cleanup);

describe("useReminderNotifications", () => {
  it("恢复 queued 和 missed 提醒但不自动确认，shown 后刷新仍保留当前气泡", async () => {
    const due = occurrence("queued-1");
    const missed = occurrence("missed-1", "missed");
    api.listOccurrences.mockResolvedValue(queuedPage([due]));
    api.missed.mockResolvedValue(missedPage([missed]));
    const view = renderHook(() => useReminderNotifications());

    await waitFor(() => expect(view.result.current.notice?.kind).toBe("due"));
    expect(view.result.current.pendingIds).toEqual([due.id, missed.id]);
    expect(api.acknowledge).not.toHaveBeenCalled();
    expect(api.acknowledgeMissed).not.toHaveBeenCalled();

    await act(async () => { await view.result.current.shown(); });
    expect(api.acknowledge).toHaveBeenCalledWith([due.id], "queued");
    api.listOccurrences.mockResolvedValue(queuedPage([]));
    act(() => changed?.());
    await waitFor(() => expect(view.result.current.pendingIds).toEqual([missed.id]));
    expect(view.result.current.notice).toEqual({ kind: "due", item: due });
    expect(api.acknowledgeMissed).not.toHaveBeenCalled();
    await act(async () => { await view.result.current.dismiss(); });
    expect(view.result.current.notice).toEqual({ kind: "missed", items: [missed] });
  });

  it("dismiss 不 complete，并在收起后推进到下一条 queued", async () => {
    const first = occurrence("queued-1");
    const second = occurrence("queued-2");
    api.listOccurrences.mockResolvedValue(queuedPage([first, second]));
    const view = renderHook(() => useReminderNotifications());

    await waitFor(() => expect(view.result.current.notice?.kind === "due" && view.result.current.notice.item.id).toBe(first.id));
    await act(async () => { await view.result.current.dismiss(); });

    expect(api.acknowledge).toHaveBeenCalledWith([first.id], "queued");
    expect(api.complete).not.toHaveBeenCalled();
    expect(view.result.current.notice).toEqual({ kind: "due", item: second });
  });

  it("在线自动完成的提醒正常展示，知道了只确认投递", async () => {
    const current = {...occurrence("queued-to-complete"),status:"done" as const,autoCompleted:true};
    api.listOccurrences.mockResolvedValue(queuedPage([current]));
    const view = renderHook(() => useReminderNotifications());

    await waitFor(() => expect(view.result.current.notice?.kind === "due" && view.result.current.notice.item.id).toBe(current.id));
    await act(async () => { await view.result.current.dismiss(); });

    expect(api.complete).not.toHaveBeenCalled();
    expect(api.acknowledge).toHaveBeenCalledWith([current.id],"queued");
  });

  it("missed 205 条按 200/5 确认，并在刷新后保留原汇总快照", async () => {
    const items = Array.from({ length: 205 }, (_, index) => occurrence(`missed-${index + 1}`, "missed"));
    api.missed.mockImplementation(async (afterId: string | null) => afterId === null
      ? missedPage(items.slice(0, 200), items[199].id)
      : missedPage(items.slice(200)));
    const view = renderHook(() => useReminderNotifications());

    await waitFor(() => expect(view.result.current.notice?.kind === "missed" && view.result.current.notice.items).toHaveLength(205));
    expect(api.missed).toHaveBeenNthCalledWith(1, null);
    expect(api.missed).toHaveBeenNthCalledWith(2, items[199].id);
    await act(async () => { await view.result.current.shown(); });

    expect(api.acknowledgeMissed).toHaveBeenNthCalledWith(1, items.slice(0, 200).map(item => item.id));
    expect(api.acknowledgeMissed).toHaveBeenNthCalledWith(2, items.slice(200).map(item => item.id));
    api.missed.mockResolvedValue(missedPage([]));
    act(() => changed?.());
    await waitFor(() => expect(view.result.current.pendingIds).toEqual([]));
    expect(view.result.current.notice).toEqual({ kind: "missed", items });
  });

  it("acknowledge 失败保留气泡并允许 shown 重试", async () => {
    const current = occurrence("queued-retry");
    api.listOccurrences.mockResolvedValue(queuedPage([current]));
    api.acknowledge.mockRejectedValueOnce(new Error("确认失败")).mockResolvedValueOnce(undefined);
    const view = renderHook(() => useReminderNotifications());

    await waitFor(() => expect(view.result.current.notice?.kind).toBe("due"));
    await act(async () => { await view.result.current.shown(); });
    expect(view.result.current.notice).toEqual({ kind: "due", item: current });
    expect(view.result.current.error).toBe("确认失败");

    await act(async () => { await view.result.current.shown(); });
    expect(api.acknowledge).toHaveBeenCalledTimes(2);
    expect(view.result.current.notice).toEqual({ kind: "due", item: current });
    expect(view.result.current.error).toBe("");
  });

  it.each(["done", "NOT_FOUND"] as const)("当前提醒外部变为 %s 时撤下气泡", async state => {
    const current = occurrence("queued-external");
    api.listOccurrences.mockResolvedValue(queuedPage([current]));
    const view = renderHook(() => useReminderNotifications());
    await waitFor(() => expect(view.result.current.notice?.kind).toBe("due"));

    if (state === "done") api.getOccurrence.mockResolvedValue({ ...current, status: "done" });
    else api.getOccurrence.mockRejectedValue(Object.assign(new Error("missing"), { code: "NOT_FOUND" }));
    act(() => changed?.());

    await waitFor(() => expect(view.result.current.notice).toBeNull());
  });

  it("较慢的旧 refresh 不能覆盖较新的提醒结果", async () => {
    const view = renderHook(() => useReminderNotifications());
    await waitForInitialRead();

    const oldQueued = defer<ReturnType<typeof queuedPage>>();
    const oldMissed = defer<ReturnType<typeof missedPage>>();
    api.listOccurrences.mockReturnValueOnce(oldQueued.promise);
    api.missed.mockReturnValueOnce(oldMissed.promise);
    act(() => window.dispatchEvent(new Event("focus")));

    const latest = occurrence("latest");
    api.listOccurrences.mockResolvedValueOnce(queuedPage([latest]));
    api.missed.mockResolvedValueOnce(missedPage([]));
    act(() => changed?.());
    await waitFor(() => expect(view.result.current.notice?.kind === "due" && view.result.current.notice.item.id).toBe(latest.id));

    await act(async () => {
      oldQueued.resolve(queuedPage([occurrence("stale")]));
      oldMissed.resolve(missedPage([]));
      await Promise.resolve();
    });
    expect(view.result.current.notice).toEqual({ kind: "due", item: latest });
  });

  it("卸载时停止事件监听并移除 focus 监听", async () => {
    const remove = vi.spyOn(window, "removeEventListener");
    const view = renderHook(() => useReminderNotifications());
    await waitForInitialRead();

    view.unmount();

    expect(stopChanged).toHaveBeenCalledTimes(1);
    expect(stopError).toHaveBeenCalledTimes(1);
    expect(remove).toHaveBeenCalledWith("focus", expect.any(Function));
  });
});
