import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import MemoryGenerationPanel from "./MemoryGenerationPanel";
import type { CurrentContext, MemoryGenerationView } from "../../types";

const api = vi.hoisted(() => ({ get: vi.fn(), retry: vi.fn(), listen: vi.fn() }));
vi.mock("../../api", () => ({ memoryGenerationApi: api, errorMessage: (error: { message?: string }) => error.message ?? "失败" }));
const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 4 };
const view = (changes: Partial<MemoryGenerationView> = {}): MemoryGenerationView => ({
  sessionId: "2", completedCount: 3, pendingCount: 1, activeRoundId: null, failedRoundId: "8",
  error: { code: "PROVIDER_UNAVAILABLE", message: "模型暂不可用" }, autoRetryPending: false,
  longCompletedCount: 0, longPendingCount: 0, longActiveRoundId: null, longFailedRoundId: null,
  longError: null, longAutoRetryPending: false, ...changes,
});
let notify: () => void;
beforeEach(() => {
  vi.clearAllMocks();
  api.get.mockResolvedValue(view());
  api.retry.mockResolvedValue(undefined);
  api.listen.mockImplementation(async (receive: () => void) => { notify = receive; return vi.fn(); });
});
afterEach(cleanup);
describe("MemoryGenerationPanel", () => {
  it("展示失败和持久化待办，明确重试后重查，事件成功后移除旧错误", async () => {
    render(<MemoryGenerationPanel context={context} />);
    expect(await screen.findByText(/短期记忆生成失败：模型暂不可用/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "重试待处理记忆" }));
    await waitFor(() => expect(api.retry).toHaveBeenCalledWith(context));
    api.get.mockResolvedValue(view({ pendingCount: 0, completedCount: 4, error: null, failedRoundId: null }));
    notify();
    await screen.findByText(/已生成 4 条，待生成 0 轮/);
    expect(screen.queryByText(/短期记忆生成失败/)).toBeNull();
    expect(screen.queryByRole("button", { name: "重试待处理记忆" })).toBeNull();
  });
  it("生成期间禁止重复重试，关闭后丢弃迟到读取并清理订阅", async () => {
    let resolve: (view: MemoryGenerationView) => void = () => {};
    const stop = vi.fn();
    api.listen.mockImplementation(async () => stop);
    api.get.mockResolvedValueOnce(view({ activeRoundId: "8", autoRetryPending: true }));
    const rendered = render(<MemoryGenerationPanel context={context} />);
    const button = await screen.findByRole("button", { name: "重试待处理记忆" });
    expect(button.hasAttribute("disabled")).toBe(true);
    api.get.mockImplementationOnce(() => new Promise<MemoryGenerationView>(done => { resolve = done; }));
    fireEvent(window, new Event("focus"));
    rendered.unmount();
    resolve(view());
    expect(stop).toHaveBeenCalledTimes(1);
  });
  it("长期整合独立显示失败、待办和恢复状态，并复用当前会话重试", async () => {
    api.get.mockResolvedValue(view({ pendingCount: 0, error: null, failedRoundId: null,
      longCompletedCount: 2, longPendingCount: 1, longFailedRoundId: "21",
      longError: { code: "MEMORY_INVALID_OUTPUT", message: "整合结果字数不合格" } }));
    render(<MemoryGenerationPanel context={context} />);
    expect(await screen.findByText(/已整合 2 条，待整合 1 组/)).toBeTruthy();
    expect(screen.getByText(/长期记忆整合失败：整合结果字数不合格/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "重试待处理记忆" }));
    await waitFor(() => expect(api.retry).toHaveBeenCalledWith(context));
    api.get.mockResolvedValue(view({ pendingCount: 0, error: null, longCompletedCount: 3,
      longPendingCount: 0, longError: null }));
    notify();
    await screen.findByText(/已整合 3 条，待整合 0 组/);
    expect(screen.queryByText(/长期记忆整合失败/)).toBeNull();
  });
});
