import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import EmbeddingProvidersPanel from "./EmbeddingProvidersPanel";
import type { EmbeddingProvider } from "../../types";
const api = vi.hoisted(() => ({ list: vi.fn(), status: vi.fn(), listen: vi.fn(), test: vi.fn(), select: vi.fn(), save: vi.fn(), delete: vi.fn() }));
vi.mock("../../api", () => ({ embeddingApi: api, errorMessage: (cause: {message?: string}) => cause.message ?? "失败" }));
const builtin: EmbeddingProvider = {id:"1",name:"本地模型",kind:"local",model:"BAAI/bge-small-zh-v1.5",baseUrl:null,hasApiKey:false,isBuiltin:true,isActive:true};
beforeEach(() => {
  HTMLDialogElement.prototype.close = vi.fn();
  HTMLDialogElement.prototype.showModal = vi.fn();
  vi.clearAllMocks();
  api.list.mockResolvedValue([builtin]);
  api.status.mockResolvedValue({total:5,ready:2,pending:3,unavailable:0,running:false,error:null});
  api.listen.mockResolvedValue(() => {});
  api.test.mockResolvedValue(512);
});
afterEach(cleanup);
it("内置模型不可编辑删除，测试后显示真实维度和补建状态", async () => {
  render(<EmbeddingProvidersPanel />);
  await screen.findByText("长期记忆向量：2/5 已就绪，3 条待补建。");
  expect(screen.queryByRole("button",{name:"编辑 Embedding 本地模型"})).toBeNull();
  expect(screen.queryByRole("button",{name:"删除 Embedding 本地模型"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"测试 Embedding 本地模型"}));
  await screen.findByText("本地模型：测试成功，返回 512 维向量。");
  expect(api.test).toHaveBeenCalledWith("1");
});
it("联网项保留密钥语义，明确使用 embeddings 接口", async () => {
  render(<EmbeddingProvidersPanel />);
  await waitFor(() => expect(screen.getByRole("button",{name:"新增联网模型"}).getAttribute("disabled")).toBeNull());
  fireEvent.click(screen.getByRole("button",{name:"新增联网模型"}));
  fireEvent.change(screen.getByLabelText("提供商名称"),{target:{value:"自己的服务"}});
  fireEvent.change(screen.getByLabelText("Base URL"),{target:{value:"https://example.com/v1"}});
  fireEvent.change(screen.getByLabelText("模型名称"),{target:{value:"embedding-model"}});
  expect(screen.getByText(/提交时补上 \/embeddings/)).toBeTruthy();
  expect(screen.queryByText("高级设置")).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"保存"}));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith(null,{name:"自己的服务",baseUrl:"https://example.com/v1",model:"embedding-model",apiKey:{action:"keep"}}));
});
