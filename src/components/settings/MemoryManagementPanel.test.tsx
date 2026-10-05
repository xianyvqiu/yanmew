import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CurrentContext, MemoryDetail, MemoryKind, MemoryRecord } from "../../types";
import MemoryManagementPanel from "./MemoryManagementPanel";

const api = vi.hoisted(() => ({ list: vi.fn(), get: vi.fn(), save: vi.fn(), preview: vi.fn(), confirm: vi.fn(), cancel: vi.fn(),
  listen: vi.fn(), getSettings: vi.fn(), listenChanges: vi.fn() }));
vi.mock("../../api", () => ({
  memoryManagementApi: api,
  configurationApi: { getSettings: api.getSettings, listenChanges: api.listenChanges },
  errorMessage: (error: { message?: string }) => error.message ?? "操作失败",
}));
vi.mock("./NotebookPanel", () => ({ default: () => <label>记事本全文<textarea aria-label="记事本全文" /></label> }));
const context: CurrentContext = { personaId: "1", sessionId: "3", contextRevision: 1 };
const record = (kind: MemoryKind, id = kind === "short" ? "10" : "20"): MemoryRecord => ({
  id, sessionId: "3", kind, roundId: kind === "short" ? "7" : null,
  integrationState: kind === "short" ? "integrated" : null, title: kind === "long" ? "长期标题" : null,
  content: kind === "short" ? "短期原正文" : "长期原正文", sourceCount: kind === "long" ? 2 : null,
  activatedByRoundId: kind === "long" ? "9" : null, coveredThroughRoundId: kind === "long" ? "8" : null,
  coveredThroughMessageId: kind === "long" ? "88" : null,
  createdAt: "2026-10-05T00:00:00Z", updatedAt: "2026-10-05T00:00:00Z",
});
const detail = (kind: MemoryKind, id?: string): MemoryDetail => ({ record: record(kind, id), memoryRevision: 1,
  sources: kind === "long" ? [
    { shortMemoryId: "12", roundId: "8", sourceOrder: 2, content: "后来源", updatedAt: "now" },
    { shortMemoryId: "11", roundId: "7", sourceOrder: 1, content: "先来源", updatedAt: "now" },
  ] : [], relatedLongMemories: kind === "short" ? [{ id: "20", title: "长期标题" }] : [],
});
let changed: (() => void)[] = [];
beforeEach(() => {
  vi.clearAllMocks(); changed = [];
  api.list.mockImplementation(async (_context: CurrentContext, kind: MemoryKind) => ({ items: [record(kind)], nextAfterId: null }));
  api.get.mockImplementation(async (_context: CurrentContext, kind: MemoryKind, id: string) => detail(kind, id));
  api.save.mockImplementation(async (_context: CurrentContext, expected: MemoryDetail, title: string | null, content: string) =>
    ({ ...expected, memoryRevision: 2, record: { ...expected.record, title, content } }));
  api.preview.mockImplementation(async (_context: CurrentContext, kind: MemoryKind) => ({ previewId: "preview-1", detail: detail(kind), title: "预览标题", content: "预览正文" }));
  api.confirm.mockImplementation(async (_context: CurrentContext, _previewId: string, title: string | null, content: string) =>
    ({ ...detail("short"), record: { ...record("short"), title, content } }));
  api.cancel.mockResolvedValue(undefined);
  api.listen.mockImplementation(async (receive: () => void) => { changed.push(receive); return vi.fn(); });
  api.getSettings.mockResolvedValue({ short_memory_min_chars: "12", short_memory_max_chars: "100", long_memory_min_chars: "150", long_memory_max_chars: "250" });
  api.listenChanges.mockResolvedValue(vi.fn());
});
afterEach(cleanup);

async function openShort() {
  fireEvent.click(screen.getByRole("tab", { name: "短期记忆" }));
  fireEvent.click(await screen.findByRole("button", { name: /回合 7/ }));
  return screen.findByRole("textbox", { name: "短期记忆正文" });
}

describe("MemoryManagementPanel", () => {
  it("AI 预览可编辑并通过 previewId 确认，取消不调用任何保存命令", async () => {
    render(<MemoryManagementPanel context={context} />);
    await openShort();
    fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    const previewEditor = await screen.findByRole("textbox", { name: "预览正文" });
    expect(screen.getByText("短期原正文", { selector: "p" })).toBeTruthy();
    expect(api.save).not.toHaveBeenCalled(); expect(api.confirm).not.toHaveBeenCalled();
    fireEvent.change(previewEditor, { target: { value: "🙂" } });
    fireEvent.click(screen.getByRole("button", { name: "确认替换" }));
    await waitFor(() => expect(api.confirm).toHaveBeenCalledWith(context, "preview-1", null, "🙂"));
    expect(api.save).not.toHaveBeenCalled();
    await screen.findByText("短期记忆已替换。");
    fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    await screen.findByRole("textbox", { name: "预览正文" });
    fireEvent.click(screen.getByRole("button", { name: "取消预览" }));
    await screen.findByText("已取消，原记忆未被替换。");
    expect(api.cancel).toHaveBeenCalledWith(context); expect(api.confirm).toHaveBeenCalledTimes(1);
  });

  it("事件和聚焦不覆盖手动草稿，冲突要求显式重新加载", async () => {
    render(<MemoryManagementPanel context={context} />);
    const editor = await openShort();
    fireEvent.change(editor, { target: { value: "我的草稿" } });
    const updated = { ...detail("short"), memoryRevision: 2, record: { ...record("short"), content: "远端正文" } };
    api.get.mockResolvedValue(updated);
    await act(async () => { changed.forEach(receive => receive()); window.dispatchEvent(new Event("focus")); });
    await screen.findByText(/记忆或来源已有更新/);
    expect(editor).toHaveProperty("value", "我的草稿");
    expect(screen.getByRole("button", { name: "保存编辑" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("button", { name: "重新加载最新内容并放弃草稿" }));
    await waitFor(() => expect(editor).toHaveProperty("value", "远端正文"));
    fireEvent.change(editor, { target: { value: "新的草稿" } });
    api.save.mockRejectedValueOnce({ code: "MEMORY_CONFLICT", message: "过期" });
    fireEvent.click(screen.getByRole("button", { name: "保存编辑" }));
    await screen.findByText(/草稿仍保留/);
    expect(editor).toHaveProperty("value", "新的草稿");
  });

  it("跨页签保留记事本及记忆草稿，相关长期定位展示来源和独立触发轮", async () => {
    render(<MemoryManagementPanel context={context} />);
    const notebook = screen.getByRole("textbox", { name: "记事本全文" });
    fireEvent.change(notebook, { target: { value: "记事本草稿" } });
    const editor = await openShort();
    fireEvent.change(editor, { target: { value: "短期草稿" } });
    fireEvent.click(screen.getByRole("button", { name: "查看长期记忆：长期标题" }));
    await screen.findByRole("textbox", { name: "长期记忆标题" });
    expect(screen.getByText(/来源 2 条 · 覆盖至回合 8 · 覆盖至 AI 消息 88 · 触发回合 9/)).toBeTruthy();
    fireEvent.click(screen.getByText("来源短期记忆（2 条）"));
    const sources = screen.getAllByRole("listitem");
    expect(within(sources[0]).getByText("先来源")).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "短期记忆" }));
    expect(screen.getByRole("textbox", { name: "短期记忆正文" })).toHaveProperty("value", "短期草稿");
    fireEvent.click(screen.getByRole("tab", { name: "记事本" }));
    expect(screen.getByRole("textbox", { name: "记事本全文" })).toHaveProperty("value", "记事本草稿");
  });

  it("预览过期保留修改草稿并阻止重复确认", async () => {
    render(<MemoryManagementPanel context={context} />);
    await openShort(); fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    const editor = await screen.findByRole("textbox", { name: "预览正文" });
    fireEvent.change(editor, { target: { value: "我的预览修改" } });
    api.confirm.mockRejectedValueOnce({ code: "MEMORY_PREVIEW_EXPIRED", message: "预览过期" });
    fireEvent.click(screen.getByRole("button", { name: "确认替换" }));
    await screen.findByText(/预览已失效/);
    expect(editor).toHaveProperty("value", "我的预览修改");
    expect(screen.getByRole("button", { name: "确认替换" })).toHaveProperty("disabled", true);
  });

  it("分页使用游标，手动编辑不限建议范围且空内容不能保存", async () => {
    api.list.mockImplementation(async (_context: CurrentContext, kind: MemoryKind, afterId: string | null) => ({
      items: [record(kind, afterId ? "11" : "10")], nextAfterId: kind === "short" && !afterId ? "10" : null,
    }));
    render(<MemoryManagementPanel context={context} />);
    const editor = await openShort();
    fireEvent.click(screen.getByRole("button", { name: "加载更多" }));
    await waitFor(() => expect(api.list).toHaveBeenCalledWith(context, "short", "10"));
    fireEvent.change(editor, { target: { value: "" } });
    expect(screen.getByRole("button", { name: "保存编辑" })).toHaveProperty("disabled", true);
    const content = "长".repeat(150);
    fireEvent.change(editor, { target: { value: content } });
    expect(editor.getAttribute("maxLength")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "保存编辑" }));
    await waitFor(() => expect(api.save).toHaveBeenCalledWith(context, detail("short"), null, content));
  });

  it("切换上下文取消生成并丢弃迟到预览，无会话时不查询记忆", async () => {
    let complete: ((value: unknown) => void) | undefined;
    api.preview.mockImplementation(() => new Promise(resolve => { complete = resolve; }));
    const view = render(<MemoryManagementPanel context={context} />);
    await openShort(); fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    const nextContext = { personaId: "1", sessionId: null, contextRevision: 2 };
    view.rerender(<MemoryManagementPanel context={nextContext} />);
    await waitFor(() => expect(api.cancel).toHaveBeenCalledWith(context));
    await act(async () => { complete?.({ previewId: "old", detail: detail("short"), title: null, content: "旧会话预览" }); });
    fireEvent.click(screen.getByRole("tab", { name: "短期记忆" }));
    expect(within(screen.getByRole("tabpanel", { name: "短期记忆" })).getByText("尚无当前会话")).toBeTruthy();
    expect(screen.queryByText("旧会话预览")).toBeNull();
    expect(api.list.mock.calls.some(args => args[0].sessionId === null)).toBe(false);
  });

  it("生成中跨页签禁止并行 AI 请求，取消后丢弃同一会话迟到结果", async () => {
    let complete: ((value: unknown) => void) | undefined;
    api.preview.mockImplementation(() => new Promise(resolve => { complete = resolve; }));
    render(<MemoryManagementPanel context={context} />);
    await openShort(); fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    fireEvent.click(screen.getByRole("tab", { name: "长期记忆" }));
    fireEvent.click(await screen.findByRole("button", { name: /长期标题长期原正文/ }));
    await screen.findByRole("textbox", { name: "长期记忆标题" });
    expect(screen.getByRole("button", { name: "重新整合" })).toHaveProperty("disabled", true);
    expect(api.preview).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("tab", { name: "短期记忆" }));
    fireEvent.click(screen.getByRole("button", { name: "取消生成" }));
    await screen.findByText("已取消，原记忆未被替换。");
    await act(async () => { complete?.({ previewId: "late", detail: detail("short"), title: null, content: "迟到预览" }); });
    expect(screen.queryByRole("textbox", { name: "预览正文" })).toBeNull();
    expect(screen.getByRole("textbox", { name: "短期记忆正文" })).toHaveProperty("value", "短期原正文");
  });

  it.each(["resolve", "reject"])("取消生成失败后可重试，原请求迟到 %s 不恢复旧预览", async outcome => {
    let complete!: (value: unknown) => void;
    let fail!: (cause: unknown) => void;
    api.preview.mockImplementationOnce(() => new Promise((resolve, reject) => { complete = resolve; fail = reject; }));
    api.cancel.mockRejectedValueOnce({ message: "读取失败" });
    render(<MemoryManagementPanel context={context} />);
    await openShort();
    fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    fireEvent.click(screen.getByRole("button", { name: "取消生成" }));
    await screen.findByText("取消失败：读取失败");
    expect(screen.getByRole("button", { name: "重新生成" })).toHaveProperty("disabled", false);
    fireEvent.click(screen.getByRole("tab", { name: "长期记忆" }));
    fireEvent.click(await screen.findByRole("button", { name: /长期标题长期原正文/ }));
    await screen.findByRole("textbox", { name: "长期记忆标题" });
    expect(screen.getByRole("button", { name: "重新整合" })).toHaveProperty("disabled", false);
    fireEvent.click(screen.getByRole("tab", { name: "短期记忆" }));
    await act(async () => {
      if (outcome === "resolve") complete({ previewId: "late", detail: detail("short"), title: null, content: "迟到预览" });
      else fail({ message: "迟到失败" });
    });
    expect(screen.queryByRole("textbox", { name: "预览正文" })).toBeNull();
    expect(screen.getByText("取消失败：读取失败")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    await screen.findByRole("textbox", { name: "预览正文" });
    expect(api.preview).toHaveBeenCalledTimes(2);
  });

  it("取消已有预览失败仍保留草稿和独占标记，可再次取消", async () => {
    render(<MemoryManagementPanel context={context} />);
    await openShort();
    fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    const editor = await screen.findByRole("textbox", { name: "预览正文" });
    fireEvent.change(editor, { target: { value: "预览修改" } });
    api.cancel.mockRejectedValueOnce({ message: "读取失败" });
    fireEvent.click(screen.getByRole("button", { name: "取消预览" }));
    await screen.findByText("取消失败：读取失败");
    expect(editor).toHaveProperty("value", "预览修改");
    fireEvent.click(screen.getByRole("tab", { name: "长期记忆" }));
    fireEvent.click(await screen.findByRole("button", { name: /长期标题长期原正文/ }));
    await screen.findByRole("textbox", { name: "长期记忆标题" });
    expect(screen.getByRole("button", { name: "重新整合" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("tab", { name: "短期记忆" }));
    fireEvent.click(screen.getByRole("button", { name: "取消预览" }));
    await screen.findByText("已取消，原记忆未被替换。");
  });

  it("预览使用返回的最新详情版本，事件与聚焦保留可编辑预览", async () => {
    render(<MemoryManagementPanel context={context} />);
    await openShort();
    const latest = { ...detail("short"), memoryRevision: 2 };
    api.preview.mockResolvedValueOnce({ previewId: "latest", detail: latest, title: null, content: "最新预览" });
    api.get.mockResolvedValue(latest);
    fireEvent.click(screen.getByRole("button", { name: "重新生成" }));
    const editor = await screen.findByRole("textbox", { name: "预览正文" });
    fireEvent.change(editor, { target: { value: "保留的预览草稿" } });
    await act(async () => { changed.forEach(receive => receive()); window.dispatchEvent(new Event("focus")); });
    expect(editor).toHaveProperty("value", "保留的预览草稿");
    expect(screen.getByRole("button", { name: "确认替换" })).toHaveProperty("disabled", false);
    expect(screen.queryByText(/记忆或来源已有更新/)).toBeNull();
  });
});
