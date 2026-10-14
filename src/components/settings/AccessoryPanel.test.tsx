import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AccessoryPanel from "./AccessoryPanel";
import type { Accessory } from "../../types";
const api = vi.hoisted(() => ({ list: vi.fn(), listen: vi.fn(), wear: vi.fn(), unwear: vi.fn(), sell: vi.fn(), delete: vi.fn(), image: vi.fn() }));
vi.mock("../../api", () => ({ accessoriesApi: api, errorMessage: (cause: Error) => cause.message }));
vi.mock("../AccessoryCraftCard", () => ({ default: () => null }));
const context = { personaId: "1", sessionId: "2", contextRevision: 1 };
const item: Accessory = { id: "3", sessionId: "2", name: "小帽子", displayText: "🎩", imageFile: null, description: "一顶帽子", cost: "20", source: "crafted", travelId: null, isWorn: false, createdAt: "now" };
beforeEach(() => {
  api.list.mockReset().mockResolvedValue([item]); api.listen.mockReset().mockResolvedValue(() => {});
  for (const method of [api.wear, api.unwear, api.sell, api.delete]) method.mockReset().mockResolvedValue({});
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);
it("未佩戴饰品需要确认才卖出，成功后列表重读而非再次结算", async () => {
  render(<AccessoryPanel context={context} />);
  await screen.findByText("小帽子");
  fireEvent.click(screen.getByRole("button", { name: "卖出 · 10 喵币" }));
  expect(api.sell).not.toHaveBeenCalled();
  const dialog = screen.getByRole("dialog");
  expect(within(dialog).getByText(/不可恢复/)).toBeTruthy();
  api.list.mockResolvedValue([]);
  fireEvent.click(within(dialog).getByRole("button", { name: "确认卖出" }));
  await screen.findByText("已卖出，获得 10 喵币。");
  expect(api.sell).toHaveBeenCalledTimes(1);
  expect(api.sell).toHaveBeenCalledWith(context, "3");
  expect(api.list.mock.calls.length).toBeGreaterThan(1);
});
it("佩戴项只能卸载，卖出和删除均禁用", async () => {
  api.list.mockResolvedValue([{ ...item, isWorn: true }]);
  render(<AccessoryPanel context={context} />);
  await screen.findByText("已佩戴");
  expect((screen.getByRole("button", { name: "卖出 · 10 喵币" }) as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "删除" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "卸载" }));
  await waitFor(() => expect(api.unwear).toHaveBeenCalledWith(context, "3"));
});
it("取消删除不调用后端，提交失败保留确认及原因", async () => {
  render(<AccessoryPanel context={context} />);
  await screen.findByText("小帽子");
  fireEvent.click(screen.getByRole("button", { name: "删除" }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  expect(api.delete).not.toHaveBeenCalled();
  api.delete.mockRejectedValue(new Error("饰品已经佩戴，请先卸载"));
  fireEvent.click(screen.getByRole("button", { name: "删除" }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "确认删除" }));
  await waitFor(() => expect(within(screen.getByRole("dialog")).getByRole("alert").textContent).toContain("请先卸载"));
  expect(screen.getByText("一顶帽子")).toBeTruthy();
});
it("没有会话不查询饰品或发起任何操作", async () => {
  await act(async () => { render(<AccessoryPanel context={{ ...context, sessionId: null }} />); });
  expect(screen.getByText(/请先在聊天窗口创建会话/)).toBeTruthy();
  expect(api.list).not.toHaveBeenCalled();
});
