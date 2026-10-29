import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import WebSearchProvidersPanel from "./WebSearchProvidersPanel";
const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn(), select: vi.fn(), delete: vi.fn(), enable: vi.fn(), test: vi.fn(), listen: vi.fn() }));
vi.mock("../../api", () => ({ webSearchApi: api, errorMessage: (cause: { message?: string }) => cause.message ?? "失败" }));
const current = { id: "1", name: "自己的搜索", kind: "tavily", baseUrl: "https://api.tavily.com", hasApiKey: true, isActive: true };
beforeEach(() => {
  vi.clearAllMocks(); HTMLDialogElement.prototype.close = vi.fn(); HTMLDialogElement.prototype.showModal = vi.fn();
  api.get.mockResolvedValue({ enabled: false, providers: [] }); api.listen.mockResolvedValue(() => {});
  api.save.mockResolvedValue(undefined); api.select.mockResolvedValue(undefined); api.enable.mockResolvedValue(undefined); api.test.mockResolvedValue(3);
});
afterEach(cleanup);
it("搜索测试中、失败和重试成功都显示在所点的服务卡片",async()=>{
  const second={...current,id:"2",name:"备用搜索",isActive:false};api.get.mockResolvedValue({enabled:true,providers:[current,second]});
  let reject!:(error:unknown)=>void;api.test.mockImplementationOnce(()=>new Promise<number>((_done,fail)=>{reject=fail;}));
  render(<WebSearchProvidersPanel/>);const element=(await screen.findByRole("heading",{name:current.name})).closest("li")!;const local=within(element);
  fireEvent.click(local.getByRole("button",{name:`测试搜索 ${current.name}`}));
  expect(local.getByRole("status").textContent).toContain("正在测试连接");
  await act(async()=>reject({message:"搜索连接超时"}));await local.findByText("搜索连接超时");
  expect(screen.getByRole("alert").closest("li")).toBe(element);
  const other=within(screen.getByRole("heading",{name:second.name}).closest("li")!);expect(other.queryByRole("alert")).toBeNull();
  fireEvent.click(local.getByRole("button",{name:`测试搜索 ${current.name}`}));await local.findByText("自己的搜索：测试成功，返回 3 条结果（测试可能消耗服务额度）。");
  expect(local.queryByRole("alert")).toBeNull();expect(other.queryByRole("status")).toBeNull();
});
it("显示关闭空态，并可新增实际博查配置", async () => {
  render(<WebSearchProvidersPanel />);
  await screen.findByText("暂无搜索服务，请先新增");
  expect((screen.getByLabelText("允许正常聊天联网搜索") as HTMLInputElement).checked).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "新增搜索服务" }));
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "博查搜索" } });
  fireEvent.change(screen.getByLabelText("服务类型"), { target: { value: "bocha" } });
  expect((screen.getByLabelText("接口地址") as HTMLInputElement).value).toBe("https://api.bocha.cn");
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fake-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith(null, { name: "博查搜索", kind: "bocha", baseUrl: "https://api.bocha.cn", apiKey: { action: "set", value: "fake-key" } }));
});
it("编辑留空保留 Key，当前服务禁止删除", async () => {
  api.get.mockResolvedValue({ enabled: true, providers: [current] }); render(<WebSearchProvidersPanel />);
  await screen.findByText("自己的搜索", { selector: "h3" });
  fireEvent.click(screen.getByRole("button", { name: "编辑搜索 自己的搜索" }));
  expect((screen.getByLabelText("API Key") as HTMLInputElement).value).toBe("");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith("1", expect.objectContaining({ apiKey: { action: "keep" } })));
  await screen.findByRole("button", { name: "新增搜索服务" });
  fireEvent.click(screen.getByRole("button", { name: "删除搜索 自己的搜索" }));
  await screen.findByText("当前搜索服务不能删除，请先切换当前项。");
  expect(screen.queryByRole("button", { name: "确认删除" })).toBeNull(); expect(api.delete).not.toHaveBeenCalled();
});
it("清除 Key 和开关操作语义独立，测试显示额度说明", async () => {
  api.get.mockResolvedValue({ enabled: false, providers: [current] }); render(<WebSearchProvidersPanel />);
  await screen.findByText("自己的搜索", { selector: "h3" });
  fireEvent.click(screen.getByLabelText("允许正常聊天联网搜索"));
  await waitFor(() => expect(api.enable).toHaveBeenCalledWith(true));
  await waitFor(() => expect(screen.getByRole("button", { name: "测试搜索 自己的搜索" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "测试搜索 自己的搜索" }));
  await screen.findByText("自己的搜索：测试成功，返回 3 条结果（测试可能消耗服务额度）。");
  fireEvent.click(screen.getByRole("button", { name: "编辑搜索 自己的搜索" }));
  fireEvent.click(screen.getByLabelText("清除已保存的 Key")); fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith("1", expect.objectContaining({ apiKey: { action: "clear" } })));
});
it("失败后保留表单和草稿，显示安全中文错误", async () => {
  api.save.mockRejectedValue({ code: "VALIDATION_ERROR", field: "apiKey", message: "请重新填写 Key" });
  render(<WebSearchProvidersPanel />); await screen.findByText("暂无搜索服务，请先新增");
  fireEvent.click(screen.getByRole("button", { name: "新增搜索服务" })); fireEvent.change(screen.getByLabelText("名称"), { target: { value: "我的搜索" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" })); await screen.findByRole("alert");
  expect((screen.getByLabelText("名称") as HTMLInputElement).value).toBe("我的搜索");
});

it("自定义明确使用 Tavily 兼容协议并要求自己的根地址", async () => {
  render(<WebSearchProvidersPanel />); await screen.findByText("暂无搜索服务，请先新增");
  fireEvent.click(screen.getByRole("button", { name: "新增搜索服务" }));
  fireEvent.change(screen.getByLabelText("服务类型"), { target: { value: "custom" } });
  expect((screen.getByLabelText("接口地址") as HTMLInputElement).value).toBe("");
  expect(screen.getByText(/普通聊天模型接口不能直接作为搜索接口/)).toBeTruthy();
  fireEvent.change(screen.getByLabelText("名称"), { target: { value: "私有搜索" } });
  fireEvent.change(screen.getByLabelText("接口地址"), { target: { value: "https://search.example.com/api" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fake-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith(null, { name: "私有搜索", kind: "custom", baseUrl: "https://search.example.com/api", apiKey: { action: "set", value: "fake-key" } }));
});
