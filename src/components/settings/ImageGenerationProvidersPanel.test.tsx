import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import ImageGenerationProvidersPanel from "./ImageGenerationProvidersPanel";
const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn(), select: vi.fn(), delete: vi.fn(), enable: vi.fn(), listen: vi.fn() }));
vi.mock("../../api", () => ({ imageGenerationApi: api, errorMessage: (cause: { message?: string }) => cause.message ?? "失败" }));
const provider = { id: "1", name: "自己的生图", protocol: "openai-compatible" as const, baseUrl: "https://images.example/v1", model: "transparent-model", hasApiKey: true };
const config = { version: 1, revision: 2, nextId: "2", enabled: false, activeId: "1", providers: [provider] };
beforeEach(() => {
  vi.clearAllMocks();
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
  api.get.mockResolvedValue({ version: 1, revision: 0, nextId: "1", enabled: false, activeId: null, providers: [] });
  api.listen.mockResolvedValue(() => {}); api.save.mockResolvedValue(config); api.select.mockResolvedValue(config); api.enable.mockResolvedValue({ ...config, enabled: true }); api.delete.mockResolvedValue({ ...config, activeId: null, providers: [] });
});
afterEach(cleanup);
it("默认关闭且无Key不可开启，新增使用独立生图字段和版本", async () => {
  render(<ImageGenerationProvidersPanel />);
  await screen.findByText("暂无生图提供商，请按需新增。");
  expect((screen.getByRole("checkbox", { name: "开启饰品生图" }) as HTMLInputElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "新增生图提供商" }));
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "我的生图" } });
  fireEvent.change(screen.getByLabelText("HTTPS 接口根地址"), { target: { value: "https://mine.example/v1" } });
  fireEvent.change(screen.getByRole("textbox", { name: "生图模型" }), { target: { value: "my-image-model" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "image-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith(null, 0, { name: "我的生图", protocol: "openai-compatible", baseUrl: "https://mine.example/v1", model: "my-image-model", apiKey: { action: "set", value: "image-key" } }));
});
it("开启必须先确认API费用，本地检查没有付费调用", async () => {
  api.get.mockResolvedValue(config); render(<ImageGenerationProvidersPanel />);
  await screen.findByText("自己的生图", { selector: "h3" });
  fireEvent.click(screen.getByRole("button", { name: "检查生图配置 自己的生图" }));
  await screen.findByText(/配置字段齐备。没有请求图片/);
  expect(api.save).not.toHaveBeenCalled(); expect(api.enable).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("checkbox", { name: "开启饰品生图" })); expect(api.enable).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认开启" }));
  await waitFor(() => expect(api.enable).toHaveBeenCalledWith(true, 2));
  await waitFor(() => expect((screen.getByRole("checkbox", { name: "开启饰品生图" }) as HTMLInputElement).checked).toBe(true));
});
it("当前项禁止删除，清除选择使用服务端版本", async () => {
  api.get.mockResolvedValue(config); render(<ImageGenerationProvidersPanel />);
  await screen.findByText("自己的生图", { selector: "h3" });
  fireEvent.click(screen.getByRole("button", { name: "删除生图 自己的生图" }));
  expect(screen.getByText("当前生图项不能删除")).toBeTruthy(); expect(screen.queryByRole("button", { name: "确认删除" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "知道了" }));
  fireEvent.change(screen.getByLabelText("当前生图提供商"), { target: { value: "" } });
  await waitFor(() => expect(api.select).toHaveBeenCalledWith(null, 2)); expect(api.delete).not.toHaveBeenCalled();
});
it("编辑保留Key但绑定打开时版本，冲突保留草稿并提供明确重读", async () => {
  api.get.mockResolvedValue(config); render(<ImageGenerationProvidersPanel />);
  await screen.findByText("自己的生图", { selector: "h3" });
  fireEvent.click(screen.getByRole("button", { name: "编辑生图 自己的生图" }));
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "我的草稿" } });
  api.get.mockResolvedValue({ ...config, revision: 3 });
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  api.save.mockRejectedValue({ code: "CONFIG_CONFLICT", message: "配置已变化" });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith("1", 2, expect.objectContaining({ apiKey: { action: "keep" }, name: "我的草稿" })));
  expect((await screen.findByRole("alert")).textContent).toBe("配置已变化"); expect((screen.getByLabelText("名称") as HTMLInputElement).value).toBe("我的草稿");
  expect(screen.getByRole("button", { name: "取消编辑并重新读取" })).toBeTruthy();
});
it("清除Key与关闭开关都为显式操作，不接触已保存饰品", async () => {
  api.get.mockResolvedValue({ ...config, enabled: true }); render(<ImageGenerationProvidersPanel />);
  await screen.findByText("自己的生图", { selector: "h3" });
  fireEvent.click(screen.getByRole("checkbox", { name: "开启饰品生图" })); await waitFor(() => expect(api.enable).toHaveBeenCalledWith(false, 2));
  fireEvent.click(screen.getByRole("button", { name: "编辑生图 自己的生图" }));
  fireEvent.click(screen.getByLabelText("清除已保存的 Key")); fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith("1", 2, expect.objectContaining({ apiKey: { action: "clear" } })));
});
