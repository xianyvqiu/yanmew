import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import TravelJournalPanel from "./TravelJournalPanel";
import type { TravelJournal } from "../types";
const api = vi.hoisted(() => ({ list: vi.fn(), get: vi.fn(), update: vi.fn(), delete: vi.fn(), status: vi.fn(), retry: vi.fn(), listen: vi.fn() }));
vi.mock("../api", () => ({ travelJournalsApi: api, travelApi: { listen: api.listen }, errorMessage: (cause: { message: string }) => cause.message }));
const context = { personaId: "1", sessionId: "2", contextRevision: 1 };
const journal: TravelJournal = { id: "3", travelId: "4", sessionId: "2", title: "今天在湖边散步两小时，带回明信片", content: "我在湖边看了水鸟。", createdAt: "2026-10-13T01:00:00Z", updatedAt: "2026-10-13T01:00:00Z" };
beforeEach(() => {
  vi.resetAllMocks(); api.list.mockResolvedValue([journal]); api.get.mockResolvedValue(journal);
  api.status.mockResolvedValue({ active: null, pendingCount: 0, failures: [], notice: null });
  api.listen.mockResolvedValue(() => {}); api.update.mockResolvedValue(journal); api.delete.mockResolvedValue(undefined); api.retry.mockResolvedValue(undefined);
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);
async function open() {
  render(<TravelJournalPanel context={context} />);
  fireEvent.click(await screen.findByRole("button", { name: /今天在湖边/ }));
  await screen.findByText(journal.content);
}
async function edit() { await open(); fireEvent.click(screen.getByRole("button", { name: "编辑游记" })); }
it("手动编辑只要求非空，以原始内容和原始时间戳保存", async () => {
  await edit(); fireEvent.change(screen.getByLabelText("游记标题"), { target: { value: "短标题" } });
  fireEvent.change(screen.getByLabelText("游记正文"), { target: { value: "短正文" } });
  api.update.mockResolvedValue({ ...journal, title: "短标题", content: "短正文" });
  fireEvent.click(screen.getByRole("button", { name: "保存游记" }));
  await waitFor(() => expect(api.update).toHaveBeenCalledWith(context, "3", "4",
    { title: journal.title, content: journal.content, updatedAt: journal.updatedAt }, "短标题", "短正文"));
  await screen.findByRole("button", { name: "编辑游记" });
});
it("聚焦补查保留草稿，远端变化提示冲突，明确确认后才丢弃", async () => {
  await edit(); fireEvent.change(screen.getByLabelText("游记正文"), { target: { value: "我的草稿" } });
  api.get.mockResolvedValue({ ...journal, content: "另一窗口的新正文", updatedAt: "2026-10-13T02:00:00Z" });
  await act(async () => { fireEvent.focus(window); });
  await screen.findByText(/游记已有新的修改/);
  expect((screen.getByLabelText("游记正文") as HTMLTextAreaElement).value).toBe("我的草稿");
  expect((screen.getByRole("button", { name: "保存游记" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "放弃草稿并加载最新" }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  expect((screen.getByLabelText("游记正文") as HTMLTextAreaElement).value).toBe("我的草稿");
  fireEvent.click(screen.getByRole("button", { name: "放弃草稿并加载最新" }));
  fireEvent.click(screen.getByRole("button", { name: "确认放弃修改" }));
  await screen.findByText("另一窗口的新正文");
});
it("保存返回冲突后，后台刷新不清除错误或草稿", async () => {
  await edit(); fireEvent.change(screen.getByLabelText("游记正文"), { target: { value: "待保存的草稿" } });
  api.update.mockRejectedValue({ code: "JOURNAL_CONFLICT", message: "游记已被修改" });
  api.get.mockResolvedValue({ ...journal, content: "新正文", updatedAt: "新时间" });
  fireEvent.click(screen.getByRole("button", { name: "保存游记" }));
  await screen.findByText("游记已被修改");
  await act(async () => { fireEvent.focus(window); });
  expect((screen.getByLabelText("游记正文") as HTMLTextAreaElement).value).toBe("待保存的草稿");
  expect(screen.getByText("游记已被修改")).toBeTruthy();
});
it("切换篇目先确认，取消保持原草稿", async () => {
  api.list.mockResolvedValue([journal, { ...journal, id: "5", title: "另一篇游记" }]);
  await edit(); fireEvent.change(screen.getByLabelText("游记正文"), { target: { value: "草稿" } });
  fireEvent.click(screen.getByRole("button", { name: /另一篇游记/ }));
  expect(screen.getByRole("dialog")).toBeTruthy();
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  expect((screen.getByLabelText("游记正文") as HTMLTextAreaElement).value).toBe("草稿");
  expect(api.get).not.toHaveBeenCalledWith(context, "5");
});
it("删除先确认，成功后关闭详情，说明保留其他旅行数据", async () => {
  await open(); fireEvent.click(screen.getByRole("button", { name: "删除游记" }));
  expect(within(screen.getByRole("dialog")).getByText(/出行记录、信件、伴手礼和饰品仍保留/)).toBeTruthy();
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  expect(api.delete).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "删除游记" })); api.list.mockResolvedValue([]);
  fireEvent.click(screen.getByRole("button", { name: "确认删除游记" }));
  await screen.findByText(/还没有游记/);
  expect(api.delete).toHaveBeenCalledWith(context, "3", "4");
  expect(screen.queryByText(journal.content)).toBeNull();
});
it("游记在其他窗口被删除时保留可复制草稿，禁止保存复活", async () => {
  await edit(); fireEvent.change(screen.getByLabelText("游记正文"), { target: { value: "可以复制" } });
  api.list.mockResolvedValue([]); api.get.mockRejectedValue({ code: "NOT_FOUND", message: "游记不存在" });
  await act(async () => { fireEvent.focus(window); });
  await screen.findByText(/草稿仍可复制/);
  expect((screen.getByLabelText("游记正文") as HTMLTextAreaElement).value).toBe("可以复制");
  expect((screen.getByRole("button", { name: "保存游记" }) as HTMLButtonElement).disabled).toBe(true);
});
it("先订阅再读取，分页刷新保留已展开篇目，状态重试复用后端", async () => {
  const page = Array.from({ length: 30 }, (_, i) => ({ ...journal, id: String(50 - i), title: `游记${i}` }));
  api.list.mockImplementation(async (_context, cursor) => cursor ? [{ ...journal, id: "1", title: "最早游记" }] : page);
  api.status.mockResolvedValue({ active: null, pendingCount: 1, failures: [{ travelId: "4", error: { message: "生成失败" }, retryAfterSeconds: 30 }], notice: "已减少旧信件" });
  render(<TravelJournalPanel context={context} />);
  fireEvent.click(await screen.findByRole("button", { name: "更多游记" }));
  await screen.findByRole("button", { name: /最早游记/ });
  expect(api.listen.mock.invocationCallOrder[0]).toBeLessThan(api.list.mock.invocationCallOrder[0]);
  expect(api.list).toHaveBeenCalledWith(context, "21");
  await act(async () => { fireEvent.focus(window); });
  expect(await screen.findByRole("button", { name: /最早游记/ })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "重试待生成游记" }));
  await waitFor(() => expect(api.retry).toHaveBeenCalledWith(context));
  expect(screen.queryByRole("button", { name: /重新生成/ })).toBeNull();
});
it("切换会话后忽略旧详情和旧编辑，迟到结果不进入新会话", async () => {
  let resolve!: (journal: TravelJournal) => void;
  api.get.mockReturnValue(new Promise(done => { resolve = done; }));
  const view = render(<TravelJournalPanel context={context} />);
  fireEvent.click(await screen.findByRole("button", { name: /今天在湖边/ }));
  api.list.mockResolvedValue([]);
  view.rerender(<TravelJournalPanel context={{ ...context, sessionId: "8", contextRevision: 2 }} />);
  await act(async () => { resolve(journal); });
  await screen.findByText(/还没有游记/);
  expect(screen.queryByText(journal.content)).toBeNull(); expect(screen.queryByRole("button", { name: "编辑游记" })).toBeNull();
});
