import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { usePetMemoryErrors } from "./usePetMemoryErrors";
import type { MemoryGenerationView, PetActivity } from "../types";

const api = vi.hoisted(() => ({ get: vi.fn(), listen: vi.fn() }));
vi.mock("../api", () => ({ memoryGenerationApi: api, errorMessage: (error: { message: string }) => error.message }));
const activity: PetActivity = { personaId: "1", sessionId: "2", contextRevision: 0, revision: 1,
  activity: "normal", expression: "(・ω・)", intimacy: 0, sleepUntil: null, error: null,
  canInteract: true, canWander: true };
const status = (sessionId = "2", message = "记忆模型不可用"): MemoryGenerationView => ({
  sessionId, completedCount: 0, pendingCount: 1, activeRoundId: null, failedRoundId: "1",
  error: { code: "PROVIDER_UNAVAILABLE", message }, autoRetryPending: false,
  longCompletedCount: 0, longPendingCount: 1, longActiveRoundId: null, longFailedRoundId: "2",
  longError: { code: "MEMORY_INVALID_OUTPUT", message: "整合结果不合格" }, longAutoRetryPending: false,
});
let notify: () => void;
const stop = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  api.get.mockResolvedValue(status());
  api.listen.mockImplementation(async (callback: () => void) => { notify = callback; return stop; });
});
afterEach(cleanup);

it("只读当前会话短期和长期错误，成功事件清除，卸载释放监听", async () => {
  const hook = renderHook(() => usePetMemoryErrors(activity));
  await waitFor(() => expect(hook.result.current).toEqual(["短期记忆生成失败：记忆模型不可用", "长期记忆整合失败：整合结果不合格"]));
  expect(api.get).toHaveBeenCalledWith({ personaId: "1", sessionId: "2", contextRevision: 0 });
  api.get.mockResolvedValue({ ...status(), error: null, longError: null });
  await act(async () => { notify(); });
  await waitFor(() => expect(hook.result.current).toEqual([]));
  hook.unmount();
  expect(stop).toHaveBeenCalledTimes(1);
});

it("切换会话立即隐藏旧错误，旧查询迟到不能污染新会话", async () => {
  const hook = renderHook(({ view }: { view: PetActivity }) => usePetMemoryErrors(view), { initialProps: { view: activity } });
  await waitFor(() => expect(hook.result.current[0]).toContain("记忆模型不可用"));
  let resolve!: (value: MemoryGenerationView) => void;
  api.get.mockImplementationOnce(() => new Promise(done => { resolve = done; }));
  await act(async () => { notify(); });
  api.get.mockResolvedValue(status("3", "新会话错误"));
  hook.rerender({ view: { ...activity, sessionId: "3", contextRevision: 1 } });
  expect(hook.result.current).toEqual([]);
  await waitFor(() => expect(hook.result.current[0]).toContain("新会话错误"));
  await act(async () => { resolve(status("2", "旧结果")); });
  expect(hook.result.current[0]).toContain("新会话错误");
  expect(hook.result.current.join()).not.toContain("旧结果");
});

it("无会话不查询记忆，读取失败只返回局部提示", async () => {
  const empty: PetActivity = { ...activity, sessionId: null };
  const hook = renderHook(({ view }: { view: PetActivity }) => usePetMemoryErrors(view), { initialProps: { view: empty } });
  expect(api.get).not.toHaveBeenCalled();
  api.get.mockRejectedValue({ code: "STORAGE_ERROR", message: "请检查磁盘空间" });
  hook.rerender({ view: activity });
  await waitFor(() => expect(hook.result.current).toEqual(["记忆状态读取失败：请检查磁盘空间"]));
});
