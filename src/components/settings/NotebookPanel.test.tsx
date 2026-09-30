import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrentContext, Notebook } from "../../types";
import NotebookPanel from "./NotebookPanel";

const api = vi.hoisted(() => ({
  get: vi.fn(), save: vi.fn(), listen: vi.fn(), getSettings: vi.fn(), listenChanges: vi.fn(),
}));
vi.mock("../../api", () => ({
  configurationApi: { getSettings: api.getSettings, listenChanges: api.listenChanges },
  notebookApi: { get: api.get, save: api.save, listen: api.listen },
  errorMessage: (error: { message?: string }) => error.message ?? "操作失败",
}));

const context: CurrentContext = { personaId: "1", sessionId: "3", contextRevision: 1 };
const notebook = (content: string, updatedAt = "2026-09-30T00:00:00.000Z"): Notebook => ({ sessionId: "3", content, updatedAt });
const view = (content: string | null, limitChars = 2) => ({ notebook: content === null ? null : notebook(content), limitChars });
let notebookChanged: (() => void) | undefined;
let configurationChanged: (() => void) | undefined;

beforeEach(() => {
  vi.clearAllMocks();
  notebookChanged = undefined;
  configurationChanged = undefined;
  api.get.mockResolvedValue(view("🙂"));
  api.save.mockImplementation(async (_context: CurrentContext, _expected: Notebook | null, content: string) => view(content));
  api.listen.mockImplementation(async (receive: () => void) => { notebookChanged = receive; return vi.fn(); });
  api.getSettings.mockResolvedValue({ notebook_max_chars: "2" });
  api.listenChanges.mockImplementation(async (receive: () => void) => { configurationChanged = receive; return vi.fn(); });
});
afterEach(cleanup);

describe("NotebookPanel", () => {
  it("按 Unicode 码点计数，超出软上限仍可保存完整或空正文", async () => {
    render(<NotebookPanel context={context} />);
    const editor = await screen.findByRole("textbox", { name: "记事本全文" });
    expect(screen.getByText("1 / 2 字")).toBeTruthy();
    fireEvent.change(editor, { target: { value: "🙂猫字" } });
    expect(screen.getByText("3 / 2 字")).toBeTruthy();
    expect(screen.getByText(/已超过建议上限/)).toBeTruthy();
    expect(editor.getAttribute("maxLength")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(api.save).toHaveBeenCalledWith(context, notebook("🙂"), "🙂猫字"));
    await screen.findByText("记事本已保存。");

    fireEvent.change(editor, { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(api.save).toHaveBeenLastCalledWith(context, notebook("🙂猫字"), ""));
  });

  it("CAS 冲突保留草稿，只有明确重新加载才用远端内容替换", async () => {
    api.save.mockRejectedValueOnce({ code: "NOTEBOOK_CONFLICT", message: "记事本已在其他位置更新" });
    api.get.mockResolvedValueOnce(view("旧内容")).mockResolvedValueOnce(view("远端内容"));
    render(<NotebookPanel context={context} />);
    const editor = await screen.findByRole("textbox", { name: "记事本全文" });
    fireEvent.change(editor, { target: { value: "我的草稿" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText(/草稿仍保留/)).toBeTruthy();
    expect(editor).toHaveProperty("value", "我的草稿");
    fireEvent.click(screen.getByRole("button", { name: "重新加载最新内容" }));
    await waitFor(() => expect(editor).toHaveProperty("value", "远端内容"));
  });

  it("事件刷新不会覆盖已修改的草稿，并提供显式重新加载", async () => {
    api.get.mockResolvedValueOnce(view("已保存")).mockResolvedValue(view("远端修改"));
    render(<NotebookPanel context={context} />);
    const editor = await screen.findByRole("textbox", { name: "记事本全文" });
    fireEvent.change(editor, { target: { value: "未保存的草稿" } });
    await act(async () => { notebookChanged?.(); });
    expect(editor).toHaveProperty("value", "未保存的草稿");
    expect(await screen.findByText(/当前草稿已保留/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "重新加载最新内容" }));
    await waitFor(() => expect(editor).toHaveProperty("value", "远端修改"));
  });

  it("草稿有改动但远端快照未变时，窗口聚焦不会提示冲突", async () => {
    api.get.mockResolvedValue(view("仍是同一份基线"));
    render(<NotebookPanel context={context} />);
    const editor = await screen.findByRole("textbox", { name: "记事本全文" });
    fireEvent.change(editor, { target: { value: "本地草稿" } });
    await act(async () => { window.dispatchEvent(new Event("focus")); });
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(2));
    expect(editor).toHaveProperty("value", "本地草稿");
    expect(screen.queryByText(/检测到记事本在其他位置更新/)).toBeNull();
  });

  it("保存会使旧后台读取失效，且保存期间的变更事件会在保存后补读", async () => {
    let resolveOldRead!: (value: ReturnType<typeof view>) => void;
    let resolveSave!: (value: ReturnType<typeof view>) => void;
    api.get.mockResolvedValueOnce(view("基线"))
      .mockImplementationOnce(() => new Promise(resolve => { resolveOldRead = resolve; }))
      .mockResolvedValueOnce(view("保存后再次更新"));
    api.save.mockImplementationOnce(() => new Promise(resolve => { resolveSave = resolve; }));
    render(<NotebookPanel context={context} />);
    const editor = await screen.findByRole("textbox", { name: "记事本全文" });

    await act(async () => { window.dispatchEvent(new Event("focus")); });
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(2));
    fireEvent.change(editor, { target: { value: "我的保存内容" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await waitFor(() => expect(api.save).toHaveBeenCalledTimes(1));
    await act(async () => { notebookChanged?.(); });
    await act(async () => { resolveSave(view("我的保存内容")); });
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(3));
    await act(async () => { resolveOldRead(view("过期读取结果")); });

    await waitFor(() => expect(editor).toHaveProperty("value", "保存后再次更新"));
    expect(screen.queryByText(/过期读取结果/)).toBeNull();
  });

  it("上下文切换后丢弃较早请求的迟到结果", async () => {
    let resolveOld!: (value: ReturnType<typeof view>) => void;
    api.get.mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }))
      .mockResolvedValueOnce(view("新会话内容"));
    const first = render(<NotebookPanel context={context} />);
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(1));
    const nextContext = { ...context, sessionId: "4", contextRevision: 2 };
    first.rerender(<NotebookPanel context={nextContext} />);
    await waitFor(() => expect(api.get).toHaveBeenCalledTimes(2));
    expect(await screen.findByRole("textbox", { name: "记事本全文" })).toHaveProperty("value", "新会话内容");
    await act(async () => { resolveOld(view("旧会话迟到内容")); });
    expect(screen.getByRole("textbox", { name: "记事本全文" })).toHaveProperty("value", "新会话内容");
  });
});
