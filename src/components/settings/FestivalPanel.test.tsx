import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import FestivalPanel from "./FestivalPanel";
import type { CurrentContext, FestivalCatalogRecord, CustomFestivalRecord } from "../../types";

const api = vi.hoisted(() => ({ month: vi.fn(), catalog: vi.fn(), browse: vi.fn(), browserListen: vi.fn(), list: vi.fn(), get: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn(), listen: vi.fn() }));
vi.mock("../../api", () => ({ festivalsApi: api, cardBrowserApi: {browse: api.browse, listen: api.browserListen}, errorMessage: (cause: unknown) => cause instanceof Error ? cause.message : String(cause) }));
const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 3 };
const system: FestivalCatalogRecord = { id: "1", code: "solar_term_qingming", name: "清明", year: 2024, calendarType: "solar_term", solarMonth: 4, solarDay: 4,
  lunarMonth: null, lunarDay: null, isLeapMonth: false, solarTerm: "清明", blessing: "缅怀故人，珍惜当下。", isMemorial: true, note: null };
function record(id = "9"): CustomFestivalRecord { return { id, festivalId: null, kind: "birthday", name: "生日", year: 2024,
  calendarType: "lunar", solarMonth: 1, solarDay: 1, lunarMonth: 11, lunarDay: 20, isLeapMonth: false, blessing: "生日快乐", note: null, source: "ai",
  createdAt: "2024-01-01T00:00:00Z", updatedAt: "2024-01-01T00:00:00Z", isMemorial: false, revisionToken: "original-token", occurrences: ["2024-01-01", "2024-12-20"] }; }
beforeEach(() => {
  vi.resetAllMocks();
  api.month.mockResolvedValue({year:2026,month:10,days:[{date:"2026-10-15",lunar:"初六",solarTerm:null,events:[{id:system.id,scope:"catalog",name:system.name,blessing:system.blessing}]}]});
  api.catalog.mockResolvedValue([system]); api.list.mockResolvedValue([record()]); api.get.mockResolvedValue(record());
  api.create.mockResolvedValue(record()); api.update.mockResolvedValue(record()); api.delete.mockResolvedValue(undefined); api.listen.mockResolvedValue(vi.fn());
  api.browserListen.mockResolvedValue(() => {});
  api.browse.mockImplementation(async (_kind, scope, search, order, page) => {
    const rows = (await api.list(scope)).filter((row: Record<string, unknown>) => JSON.stringify(row).includes(search));
    if (order === "oldest") rows.reverse();
    const pages = Math.max(1, Math.ceil(rows.length / 12));
    const current = Math.min(page, pages);
    return {items: rows.slice((current - 1) * 12, current * 12), total: rows.length, page: current, pages};
  });
  HTMLDialogElement.prototype.showModal = vi.fn(function (this: HTMLDialogElement) { this.setAttribute("open", ""); });
  HTMLDialogElement.prototype.close = vi.fn(function (this: HTMLDialogElement) { this.removeAttribute("open"); });
});
afterEach(cleanup);
async function show(value = context) { let view!: ReturnType<typeof render>; await act(async () => { view = render(<FestivalPanel context={value} />); }); fireEvent.click(screen.getByText("管理自定义日期")); return view; }

it("无会话仍展示系统节日和全局日期，允许新增", async () => {
  await show({ ...context, sessionId: null });
  expect(screen.getAllByText("清明").length).toBeGreaterThan(0);
  fireEvent.click(screen.getByRole("button",{name:/2026-10-15/}));
  expect(screen.getByText(/缅怀故人/)).toBeTruthy();
  expect((within(screen.getByRole("region", {name:"自定义日期"})).getByRole("button", { name: "新增日期" }) as HTMLButtonElement).disabled).toBe(false);
  expect(api.list).toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "编辑 清明" })).toBeNull();
});

it("手动新增只提交固定月日和非空祝福，没有母表关联或出生年份", async () => {
  await show(); fireEvent.click(within(screen.getByRole("region", {name:"自定义日期"})).getByRole("button", { name: "新增日期" }));
  const form = screen.getByRole("form", { name: "新增日期" });
  fireEvent.change(within(form).getByLabelText("名称"), { target: { value: "相识纪念日" } });
  fireEvent.change(within(form).getByLabelText("祝福词"), { target: { value: "愿往后的每一天都值得期待。" } });
  fireEvent.change(within(form).getByLabelText("类型"), { target: { value: "anniversary" } });
  fireEvent.change(within(form).getByLabelText("月份"), { target: { value: "2" } });
  fireEvent.change(within(form).getByLabelText("日期"), { target: { value: "29" } });
  await act(async () => { fireEvent.submit(form); });
  expect(api.create).toHaveBeenCalledWith({ kind: "anniversary", name: "相识纪念日", calendarType: "solar", month: 2, day: 29, isLeapMonth: false, blessing: "愿往后的每一天都值得期待。", note: null });
  expect(screen.queryByRole("form")).toBeNull(); expect(screen.getByText("日期已保存。")).toBeTruthy();
});

it("农历两次日期完整展示，编辑冲突及刷新均保留草稿和原快照", async () => {
  await show(); fireEvent.click(screen.getByRole("button",{name:"生日"})); expect(screen.getByText(/2024-01-01、2024-12-20/)).toBeTruthy();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "编辑 生日" })); });
  fireEvent.change(screen.getByLabelText("祝福词"), { target: { value: "用户未保存的祝福" } });
  api.update.mockRejectedValueOnce(new Error("记录已修改，请重新读取"));
  await act(async () => { fireEvent.submit(screen.getByRole("form", { name: "编辑日期" })); });
  expect(api.update.mock.calls[0][1]).toBe("original-token");
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
  await show(); fireEvent.click(screen.getByRole("button",{name:"生日"})); fireEvent.click(screen.getByRole("button", { name: "删除 生日" }));
  expect(api.delete).not.toHaveBeenCalled();
  api.delete.mockRejectedValueOnce(new Error("记录已修改"));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "确认删除" })); });
  expect(screen.getByRole("dialog").hasAttribute("open")).toBe(true);
  expect(screen.getByRole("button", { name: "删除 生日" })).toBeTruthy();
  api.list.mockResolvedValue([]);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "确认删除" })); });
  expect(api.delete).toHaveBeenLastCalledWith("9", "original-token");
  expect(screen.getByText("日期已删除。")).toBeTruthy(); expect(screen.queryByRole("button", { name: "删除 生日" })).toBeNull();
});

it("切换会话保留全局日期和未保存草稿，卸载才释放监听", async () => {
  const stop=vi.fn();api.listen.mockResolvedValue(stop);
  const view=await show();fireEvent.click(screen.getByRole("button",{name:"生日"}));
  await act(async()=>fireEvent.click(screen.getByRole("button",{name:"编辑 生日"})));
  fireEvent.change(screen.getByLabelText("祝福词"),{target:{value:"保留这份草稿"}});
  await act(async()=>view.rerender(<FestivalPanel context={{...context,sessionId:null,contextRevision:4}}/>));
  expect((screen.getByLabelText("祝福词") as HTMLTextAreaElement).value).toBe("保留这份草稿");
  expect(stop).not.toHaveBeenCalled();
  view.unmount();expect(stop).toHaveBeenCalledTimes(1);
});

it("日期按后端完整集合搜索、排序及分页，聚焦不重置条件", async () => {
  api.list.mockResolvedValue(Array.from({length:25},(_,i)=>({...record(String(100-i)),name:`生日编号${i}`})));
  await show();
  fireEvent.click(screen.getByRole("button",{name:"下一页"}));
  await screen.findByRole("button",{name:"生日编号12"});
  await act(async()=>fireEvent.focus(window));
  expect(await screen.findByRole("button",{name:"生日编号12"})).toBeTruthy();
  fireEvent.change(screen.getByRole("searchbox"),{target:{value:"生日编号24"}});
  await screen.findByRole("button",{name:"生日编号24"});
  expect(api.browse).toHaveBeenLastCalledWith("dates",context,"生日编号24","newest",1);
  expect(api.catalog).not.toHaveBeenCalled();
});
