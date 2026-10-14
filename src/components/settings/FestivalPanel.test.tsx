import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import FestivalPanel from "./FestivalPanel";
import type { CurrentContext, FestivalCatalogRecord, SessionFestivalRecord } from "../../types";

const api = vi.hoisted(() => ({ catalog: vi.fn(), list: vi.fn(), get: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn(), listen: vi.fn() }));
vi.mock("../../api", () => ({ festivalsApi: api, errorMessage: (cause: unknown) => cause instanceof Error ? cause.message : String(cause) }));
const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 3 };
const system: FestivalCatalogRecord = { id: "1", code: "solar_term_qingming", name: "清明", year: 2024, calendarType: "solar_term", solarMonth: 4, solarDay: 4,
  lunarMonth: null, lunarDay: null, isLeapMonth: false, solarTerm: "清明", blessing: "缅怀故人，珍惜当下。", isMemorial: true, note: null };
function record(id = "9"): SessionFestivalRecord { return { id, sessionId: "2", festivalId: null, kind: "birthday", name: "生日", year: 2024,
  calendarType: "lunar", solarMonth: 1, solarDay: 1, lunarMonth: 11, lunarDay: 20, isLeapMonth: false, blessing: "生日快乐", note: null, source: "ai",
  createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z", isMemorial: false, revisionToken: "original-token", occurrences: ["2024-01-01", "2024-12-20"] }; }
beforeEach(() => {
  vi.resetAllMocks();
  api.catalog.mockResolvedValue([system]); api.list.mockResolvedValue([record()]); api.get.mockResolvedValue(record());
  api.create.mockResolvedValue(record()); api.update.mockResolvedValue(record()); api.delete.mockResolvedValue(undefined); api.listen.mockResolvedValue(vi.fn());
  HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) { this.setAttribute("open", ""); });
  HTMLDialogElement.prototype.close = vi.fn(function (this: HTMLDialogElement) { this.removeAttribute("open"); });
});
afterEach(cleanup);
async function show(value = context) { let view!: ReturnType<typeof render>; await act(async () => { view = render(<FestivalPanel key={value.sessionId} context={value} />); }); return view; }

it("无会话仍展示只读系统节日，不调用会话接口", async () => {
  await show({ ...context, sessionId: null });
  expect(screen.getByText("清明")).toBeTruthy();
  expect(screen.getByText(/缅怀故人/)).toBeTruthy();
  expect((screen.getByRole("button", { name: "新增日期" }) as HTMLButtonElement).disabled).toBe(true);
  expect(api.list).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "编辑 清明" })).toBeNull();
});

it("手动新增只提交固定月日和非空祝福，没有母表关联或出生年份", async () => {
  await show(); fireEvent.click(screen.getByRole("button", { name: "新增日期" }));
  const form = screen.getByRole("form", { name: "新增日期" });
  fireEvent.change(within(form).getByLabelText("名称"), { target: { value: "相识纪念日" } });
  fireEvent.change(within(form).getByLabelText("祝福词"), { target: { value: "愿往后的每一天都值得期待。" } });
  fireEvent.change(within(form).getByLabelText("类型"), { target: { value: "anniversary" } });
  fireEvent.change(within(form).getByLabelText("月份"), { target: { value: "2" } });
  fireEvent.change(within(form).getByLabelText("日期"), { target: { value: "29" } });
  await act(async () => { fireEvent.submit(form); });
  expect(api.create).toHaveBeenCalledWith(context, { kind: "anniversary", name: "相识纪念日", calendarType: "solar", month: 2, day: 29, isLeapMonth: false, blessing: "愿往后的每一天都值得期待。", note: null });
  expect(screen.queryByRole("form")).toBeNull(); expect(screen.getByText("日期已保存。")).toBeTruthy();
});

it("农历两次日期完整展示，编辑冲突及刷新均保留草稿和原快照", async () => {
  await show(); expect(screen.getByText(/2024-01-01、2024-12-20/)).toBeTruthy();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "编辑 生日" })); });
  fireEvent.change(screen.getByLabelText("祝福词"), { target: { value: "用户未保存的祝福" } });
  api.update.mockRejectedValueOnce(new Error("记录已修改，请重新读取"));
  await act(async () => { fireEvent.submit(screen.getByRole("form", { name: "编辑日期" })); });
  expect(api.update.mock.calls[0][2]).toBe("original-token");
  expect((screen.getByLabelText("祝福词") as HTMLTextAreaElement).value).toBe("用户未保存的祝福");
  const newest = { ...record(), revisionToken: "new-token", blessing: "其他窗口改过的祝福" }; api.list.mockResolvedValue([newest]);
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  expect((screen.getByLabelText("祝福词") as HTMLTextAreaElement).value).toBe("用户未保存的祝福");
  expect(screen.getByRole("alert").textContent).toBe("记录已修改，请重新读取");
  api.get.mockResolvedValue(newest);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "放弃草稿并重新读取" })); });
  expect((screen.getByLabelText("祝福词") as HTMLTextAreaElement).value).toBe("其他窗口改过的祝福");
});

it("删除需要确认，失败保留记录与确认框，成功才刷新", async () => {
  await show(); fireEvent.click(screen.getByRole("button", { name: "删除 生日" }));
  expect(api.delete).not.toHaveBeenCalled();
  api.delete.mockRejectedValueOnce(new Error("记录已修改"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "确认删除" })); });
  expect(screen.getByRole("dialog").hasAttribute("open")).toBe(true);
  expect(screen.getByRole("button", { name: "删除 生日" })).toBeTruthy();
  api.list.mockResolvedValue([]);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "确认删除" })); });
  expect(api.delete).toHaveBeenLastCalledWith(context, "9", "original-token");
  expect(screen.getByText("日期已删除。")).toBeTruthy(); expect(screen.queryByRole("button", { name: "删除 生日" })).toBeNull();
});

it("切换会话清空旧草稿，旧请求返回不污染新会话，监听正确释放", async () => {
  let finish!: (value: SessionFestivalRecord) => void;
  const pending = new Promise<SessionFestivalRecord>(resolve => { finish = resolve; });
  const stop = vi.fn(); api.listen.mockResolvedValue(stop);
  const view = await show(); api.get.mockReturnValueOnce(pending);
  fireEvent.click(screen.getByRole("button", { name: "编辑 生日" }));
  api.list.mockResolvedValue([]);
  await act(async () => { view.rerender(<FestivalPanel key="3" context={{ ...context, sessionId: "3", contextRevision: 4 }} />); });
  await act(async () => { finish(record()); });
  expect(screen.queryByRole("form")).toBeNull(); expect(screen.queryByRole("button", { name: "编辑 生日" })).toBeNull();
  expect(stop).toHaveBeenCalledTimes(1);
  view.unmount(); expect(stop).toHaveBeenCalledTimes(2);
});

it("日期分页使用末条字符串ID，不限制总记录数", async () => {
  api.list.mockResolvedValue(Array.from({ length: 30 }, (_, index) => record(String(100 - index))));
  await show(); api.list.mockResolvedValueOnce([record("70")]);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "更多日期" })); });
  expect(api.list).toHaveBeenLastCalledWith(context, "71", 30);
  expect(screen.getAllByRole("button", { name: "编辑 生日" })).toHaveLength(31);
  expect(screen.queryByRole("button", { name: "更多日期" })).toBeNull();
});
it("聚焦和日期通知保留已展开的记录", async () => {
  let receive!: (event: { sessionId: string | null }) => void;
  api.listen.mockImplementation(async cb => { receive = cb; return () => {}; });
  let records = Array.from({ length: 31 }, (_, index) => record(String(100 - index)));
  api.list.mockImplementation(async (_context, before) => records.filter(row => !before || BigInt(row.id) < BigInt(before)).slice(0, 30));
  await show();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "更多日期" })); });
  await act(async () => { fireEvent.focus(window); });
  expect(screen.getAllByRole("button", { name: "编辑 生日" })).toHaveLength(31);
  records = [record("101"), ...records];
  await act(async () => { receive({ sessionId: "2" }); });
  expect(screen.getAllByRole("button", { name: "编辑 生日" })).toHaveLength(32);
  expect(screen.queryByRole("button", { name: "更多日期" })).toBeNull();
});
