import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AccessoryCraftCard from "./AccessoryCraftCard";
import type { AccessoryCraftPreview, CurrentContext } from "../types";

const api = vi.hoisted(() => ({ current: vi.fn(), confirm: vi.fn(), cancel: vi.fn(), wallet: vi.fn(), listen: vi.fn() }));
vi.mock("../api", () => ({ accessoryCraftApi: { current: api.current, confirm: api.confirm, cancel: api.cancel, listen: api.listen },
  walletApi: { get: api.wallet, listen: api.listen }, errorMessage: (cause: { message?: string }) => cause.message ?? String(cause) }));
const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 1 };
const ready: AccessoryCraftPreview = { context: { ...context, sessionId: "2" }, previewId: "preview1", tier: 1, cost: "20", name: "小花", displayText: "✿", description: "一朵小花。", status: "ready", appearance: "text", imageDataUrl: null, notice: null };
beforeEach(() => {
  api.current.mockReset().mockResolvedValue(ready); api.confirm.mockReset().mockResolvedValue({}); api.cancel.mockReset().mockResolvedValue(undefined);
  api.wallet.mockReset().mockResolvedValue({ coins: "100", updatedAt: "now" }); api.listen.mockReset().mockResolvedValue(() => {});
});
afterEach(cleanup);
it("准备中禁买可取消，展示真实字符外观及 API 费用说明", async () => {
  api.current.mockResolvedValue({ ...ready, status: "preparing" });
  render(<AccessoryCraftCard context={context} />);
  await screen.findByText("✿");
  expect((screen.getByRole("button", { name: "给你买" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "没有钱了" }) as HTMLButtonElement).disabled).toBe(false);
  expect(screen.getByText(/取消或不购买不保证退还 API 费用/)).toBeTruthy();
  api.current.mockResolvedValue(null);
  fireEvent.click(screen.getByRole("button", { name: "没有钱了" }));
  await waitFor(() => expect(api.cancel).toHaveBeenCalledWith(context, "preview1"));
  expect(api.confirm).not.toHaveBeenCalled();
});
it("余额不足不能购买，金币字符串不用浮点比较", async () => {
  api.wallet.mockResolvedValue({ coins: "19", updatedAt: "now" });
  render(<AccessoryCraftCard context={context} />);
  await screen.findByText("当前喵币不足。");
  expect((screen.getByRole("button", { name: "给你买" }) as HTMLButtonElement).disabled).toBe(true);
});
it("双击购买只提交一次，成功重新查询候选与余额", async () => {
  let finish!: () => void;
  api.confirm.mockImplementation(() => new Promise<void>(resolve => { finish = resolve; }));
  render(<AccessoryCraftCard context={context} />);
  await screen.findByText("✿");
  const buy = screen.getByRole("button", { name: "给你买" });
  fireEvent.click(buy); fireEvent.click(buy);
  expect(api.confirm).toHaveBeenCalledTimes(1);
  api.current.mockResolvedValue(null);
  await act(async () => { finish(); });
  await screen.findByText("已制作，前往饰品页佩戴。");
  expect(api.wallet.mock.calls.length).toBeGreaterThan(1);
});
it("确认发现缺图时刷新字符预览，不能自动重试购买", async () => {
  api.current.mockResolvedValueOnce({ ...ready, appearance: "image", imageDataUrl: "data:image/png;base64,AA==" }).mockResolvedValue({ ...ready, notice: "图片已丢失，请重新确认。" });
  api.confirm.mockRejectedValueOnce({ code: "ACCESSORY_IMAGE_UNAVAILABLE", message: "图片已丢失，请重新确认。" });
  render(<AccessoryCraftCard context={context} />);
  await screen.findByRole("img", { name: "小花" });
  await waitFor(() => expect((screen.getByRole("button", { name: "给你买" }) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "给你买" }));
  await screen.findByText("✿");
  expect(api.confirm).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("img", { name: "小花" })?.tagName).toBe("SPAN");
});
it("旧会话查询迟到不能显示旧报价，聚焦补查新会话", async () => {
  let old!: (value: AccessoryCraftPreview) => void;
  api.current.mockImplementation((scope: CurrentContext) => scope.sessionId === "2" ? new Promise(resolve => { old = resolve; }) : Promise.resolve(null));
  const view = render(<AccessoryCraftCard context={context} />);
  await waitFor(() => expect(api.current).toHaveBeenCalled());
  const next = { ...context, sessionId: "3", contextRevision: 2 };
  view.rerender(<AccessoryCraftCard context={next} />);
  await act(async () => { old(ready); });
  expect(screen.queryByText("想做一件小花")).toBeNull();
  await act(async () => { fireEvent.focus(window); });
  expect(api.current).toHaveBeenLastCalledWith(next);
});
