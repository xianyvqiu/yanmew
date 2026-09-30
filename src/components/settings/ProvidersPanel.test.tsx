import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import ProvidersPanel from "./ProvidersPanel";
import type { Provider } from "../../types";

const api = vi.hoisted(() => ({ listProviders: vi.fn(), listenChanges: vi.fn(), createProvider: vi.fn(), updateProvider: vi.fn(), deleteProvider: vi.fn(), setChatProvider: vi.fn(), setMemoryProvider: vi.fn(), testProvider: vi.fn() }));
vi.mock("../../api", () => ({ configurationApi: api, errorMessage: (error: { message: string }) => error.message }));
const first: Provider = { id: "1", name: "模型一", model: "chat-one", baseUrl: null, hasApiKey: true, isActive: true, memoryIsActive: false };
const second: Provider = { ...first, id: "2", name: "模型二", model: "chat-two", isActive: false };
let rows: Provider[];
beforeEach(() => {
  vi.clearAllMocks(); rows = [first, second];
  api.listProviders.mockImplementation(async () => [...rows]);
  api.listenChanges.mockResolvedValue(() => {});
  api.createProvider.mockResolvedValue(second); api.updateProvider.mockResolvedValue(first);
  api.deleteProvider.mockResolvedValue(undefined); api.setChatProvider.mockResolvedValue(undefined);
  api.setMemoryProvider.mockResolvedValue(undefined); api.testProvider.mockResolvedValue(undefined);
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);

it("空列表可新增，后端拒绝保存时保留输入并显示字段错误", async () => {
  rows = [];
  api.createProvider.mockRejectedValue({ code: "VALIDATION_ERROR", field: "model", message: "模型名称无效" });
  render(<ProvidersPanel />);
  await screen.findByText("暂无提供商，请先新增。");
  fireEvent.click(screen.getByRole("button", { name: "新增提供商" }));
  fireEvent.change(screen.getByLabelText("提供商名称"), { target: { value: "我的模型" } });
  fireEvent.change(screen.getByLabelText("模型名称"), { target: { value: "unknown" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText("模型名称无效");
  expect((screen.getByLabelText("提供商名称") as HTMLInputElement).value).toBe("我的模型");
  expect(api.createProvider).toHaveBeenCalledWith({ name: "我的模型", model: "unknown", baseUrl: null, apiKey: { action: "keep" } });
});

it("当前项只能查看禁止删除提示，未使用项经确认后删除", async () => {
  render(<ProvidersPanel />); await screen.findByText("模型一");
  fireEvent.click(screen.getByRole("button", { name: "删除 模型一" }));
  let modal = within(screen.getByRole("dialog"));
  expect(modal.getByText("此提供商正在使用")).toBeTruthy();
  expect(modal.queryByRole("button", { name: "确认删除" })).toBeNull();
  expect(api.deleteProvider).not.toHaveBeenCalled();
  fireEvent.click(modal.getByRole("button", { name: "知道了" }));
  fireEvent.click(screen.getByRole("button", { name: "删除 模型二" }));
  modal = within(screen.getByRole("dialog"));
  expect(api.deleteProvider).not.toHaveBeenCalled();
  rows = [first]; fireEvent.click(modal.getByRole("button", { name: "确认删除" }));
  await waitFor(() => expect(api.deleteProvider).toHaveBeenCalledWith("2"));
  await waitFor(() => expect(screen.queryByRole("button", { name: "删除 模型二" })).toBeNull());
});

it("聊天与记忆选择读取后端结果，支持记忆跟随聊天", async () => {
  api.setChatProvider.mockImplementation(async id => { rows = rows.map(item => ({ ...item, isActive: item.id === id })); });
  api.setMemoryProvider.mockImplementation(async id => { rows = rows.map(item => ({ ...item, memoryIsActive: item.id === id })); });
  render(<ProvidersPanel />); await screen.findByText("模型一");
  fireEvent.change(screen.getByLabelText("聊天提供商"), { target: { value: "2" } });
  await waitFor(() => expect((screen.getByLabelText("聊天提供商") as HTMLSelectElement).value).toBe("2"));
  fireEvent.change(screen.getByLabelText("记忆提供商"), { target: { value: "1" } });
  await waitFor(() => expect((screen.getByLabelText("记忆提供商") as HTMLSelectElement).value).toBe("1"));
  fireEvent.change(screen.getByLabelText("记忆提供商"), { target: { value: "" } });
  await waitFor(() => expect(api.setMemoryProvider).toHaveBeenLastCalledWith(null));
});

it("连接测试调用已保存 ID，等待结果时防止重复请求", async () => {
  let finish!: () => void;
  api.testProvider.mockImplementation(() => new Promise<void>(resolve => { finish = resolve; }));
  render(<ProvidersPanel />); await screen.findByText("模型一");
  const button = screen.getByRole("button", { name: "测试连接 模型一" });
  fireEvent.click(button); fireEvent.click(button);
  expect(api.testProvider).toHaveBeenCalledTimes(1);
  expect(api.testProvider).toHaveBeenCalledWith("1");
  await act(async () => finish());
  await screen.findByText("模型一：连接成功（已保存配置）");
});

it("新增已提交但列表刷新失败时不重复新增", async () => {
  render(<ProvidersPanel />); await screen.findByText("模型一");
  fireEvent.click(screen.getByRole("button", { name: "新增提供商" }));
  fireEvent.change(screen.getByLabelText("提供商名称"), { target: { value: "新增" } });
  fireEvent.change(screen.getByLabelText("模型名称"), { target: { value: "chat" } });
  api.listProviders.mockRejectedValueOnce({ message: "读取失败" });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText("读取失败");
  expect(screen.queryByRole("button", { name: "保存" })).toBeNull();
  expect(api.createProvider).toHaveBeenCalledTimes(1);
});
