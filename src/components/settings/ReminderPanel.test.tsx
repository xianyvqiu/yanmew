import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Reminder, ReminderOccurrence } from "../../types";
import ReminderPanel from "./ReminderPanel";

const api = vi.hoisted(() => ({
  complete: vi.fn(), updateStatus: vi.fn(), acknowledge: vi.fn(), acknowledgeMissed: vi.fn(), delete: vi.fn(),
}));
const hook = vi.hoisted(() => ({ value: null as any }));
vi.mock("../../api", () => ({ reminderApi: api, errorMessage: (error: { message: string }) => error.message }));
vi.mock("../../hooks/useReminders", () => ({ default: () => hook.value }));

const dailyRule: Reminder = {
  id: "rule-1", name: "喝水", content: "喝一杯水", kind: "daily",
  schedule: { version: 1, repeat: "daily", time: "09:00", next_local_at: "2032-04-06T09:00:00" },
  fireAt: "2032-04-06T01:00:00.000Z", status: "pending", source: "manual", createdAt: "2032-01-01T00:00:00.000Z",
};
const makeOccurrence = (id: string, overrides: Partial<ReminderOccurrence> = {}): ReminderOccurrence => ({
  id, reminderId: dailyRule.id, name: dailyRule.name, content: dailyRule.content,
  scheduledAt: "2032-04-05T01:00:00.000Z", status: "pending", deliveryState: "shown",
  completedAt: null, createdAt: "2032-04-05T01:00:00.000Z", ...overrides,
});

beforeEach(() => {
  vi.clearAllMocks();
  hook.value = {
    rules: [dailyRule], occurrences: [makeOccurrence("occ-pending"), makeOccurrence("occ-done", { status: "done" }), makeOccurrence("occ-expired", { status: "expired" })],
    missed: [], selectedId: "", hasMoreRules: false, hasMoreOccurrences: false, loading: false, error: "",
    refresh: vi.fn(async () => {}), select: vi.fn(), moreRules: vi.fn(), moreOccurrences: vi.fn(),
  };
  api.complete.mockResolvedValue(undefined);
  api.updateStatus.mockResolvedValue(undefined);
  api.acknowledge.mockResolvedValue(undefined);
  api.acknowledgeMissed.mockResolvedValue(undefined);
  api.delete.mockResolvedValue(undefined);
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);

it("循环提醒完成只完成指定 occurrence，保留规则和下一次 fireAt", async () => {
  render(<ReminderPanel />);
  fireEvent.click(screen.getByRole("button", { name: "完成本次 occ-pending" }));

  await waitFor(() => expect(api.complete).toHaveBeenCalledWith("occ-pending"));
  expect(api.delete).not.toHaveBeenCalled();
  expect(api.updateStatus).not.toHaveBeenCalled();
  expect(hook.value.rules[0].fireAt).toBe("2032-04-06T01:00:00.000Z");
  expect(hook.value.refresh).toHaveBeenCalled();
});

it("手动状态变更对 pending、done、expired 分别传递 occurrence ID", async () => {
  render(<ReminderPanel />);
  fireEvent.change(screen.getByLabelText("喝水 occ-pending 本次状态"), { target: { value: "done" } });
  await waitFor(() => expect(api.updateStatus).toHaveBeenNthCalledWith(1, "occ-pending", "done"));
  fireEvent.change(screen.getByLabelText("喝水 occ-done 本次状态"), { target: { value: "pending" } });
  await waitFor(() => expect(api.updateStatus).toHaveBeenNthCalledWith(2, "occ-done", "pending"));
  fireEvent.change(screen.getByLabelText("喝水 occ-expired 本次状态"), { target: { value: "expired" } });
  await waitFor(() => expect(api.updateStatus).toHaveBeenNthCalledWith(3, "occ-expired", "expired"));
});

it("删除规则在确认前无调用，确认后删除并刷新列表", async () => {
  render(<ReminderPanel />);
  fireEvent.click(screen.getByRole("button", { name: "删除 喝水" }));
  const dialog = within(screen.getByRole("dialog"));
  expect(api.delete).not.toHaveBeenCalled();
  fireEvent.click(dialog.getByRole("button", { name: "确认删除" }));

  await waitFor(() => expect(api.delete).toHaveBeenCalledWith("rule-1"));
  await waitFor(() => expect(hook.value.refresh).toHaveBeenCalled());
});

it("读取时不自动确认离线汇总；点击后按最多 200 个 occurrence ID 分批确认", async () => {
  const missed = Array.from({ length: 205 }, (_, index) => makeOccurrence(`missed-${index + 1}`, {
    reminderId: index < 204 ? "same-rule" : "other-rule",
    name: index < 204 ? "循环喝水" : "复诊",
    deliveryState: "missed",
  }));
  hook.value.missed = missed;
  render(<ReminderPanel />);

  const summary = within(screen.getByRole("complementary", { name: "离线过期汇总" }));
  expect(summary.getByText("你离开这段时间，循环喝水、复诊过期了！")).toBeTruthy();
  expect(api.acknowledgeMissed).not.toHaveBeenCalled();

  fireEvent.click(summary.getByRole("button", { name: "知道了" }));
  await waitFor(() => expect(api.acknowledgeMissed).toHaveBeenCalledTimes(2));
  expect(api.acknowledgeMissed).toHaveBeenNthCalledWith(1, missed.slice(0, 200).map(item => item.id));
  expect(api.acknowledgeMissed).toHaveBeenNthCalledWith(2, missed.slice(200).map(item => item.id));
});

it("排队中的触发记录点击确认时只传 occurrence ID 与 queued 状态", async () => {
  hook.value.occurrences = [makeOccurrence("occ-queued", { deliveryState: "queued" })];
  render(<ReminderPanel />);
  fireEvent.click(screen.getByRole("button", { name: "确认已看到" }));

  await waitFor(() => expect(api.acknowledge).toHaveBeenCalledWith(["occ-queued"], "queued"));
  expect(api.complete).not.toHaveBeenCalled();
});


it("规则与记录独立分页，搜索可找到首屏以外的数据，筛选不执行写操作", () => {
  hook.value.rules = Array.from({length:14}, (_,i) => ({...dailyRule,id:`r-${i+1}`,name:`规则${i+1}`,content:i===0?"年度保险资料":"普通详情",kind:i%2===0?"once":"daily",status:i%3===0?"done":"pending",createdAt:`2032-04-${String(i+1).padStart(2,"0")}T00:00:00Z`}));
  hook.value.occurrences = hook.value.rules.map((rule:Reminder,i:number) => makeOccurrence(`o-${i+1}`,{reminderId:rule.id,name:rule.name,content:rule.content,status:rule.status,scheduledAt:rule.createdAt}));
  const view=render(<ReminderPanel/>);
  const rules=within(screen.getByRole("region",{name:"提醒规则列表"}));
  const records=within(screen.getByRole("region",{name:"提醒触发记录列表"}));
  expect(rules.getAllByRole("article")).toHaveLength(6);
  expect(records.getAllByRole("article")).toHaveLength(6);
  fireEvent.click(rules.getByRole("button",{name:"下一页"}));
  expect(rules.getByText("2 / 3 页 · 14 条")).toBeTruthy();
  expect(records.getByText("1 / 3 页 · 14 条")).toBeTruthy();
  fireEvent.change(rules.getByRole("searchbox",{name:"搜索提醒规则"}),{target:{value:"年度保险"}});
  expect(rules.getByRole("article",{name:"规则1"})).toBeTruthy();
  expect(rules.getByText("1 / 1 页 · 1 条")).toBeTruthy();
  fireEvent.click(rules.getByRole("button",{name:"清除规则筛选"}));
  fireEvent.change(rules.getByRole("combobox",{name:"提醒类型"}),{target:{value:"daily"}});
  fireEvent.change(rules.getByRole("combobox",{name:"规则状态"}),{target:{value:"done"}});
  expect(rules.getAllByRole("article").map(n=>n.getAttribute("aria-label"))).toEqual(["规则10","规则4"]);
  fireEvent.change(records.getByRole("combobox",{name:"记录类型"}),{target:{value:"once"}});
  fireEvent.change(records.getByRole("combobox",{name:"记录状态"}),{target:{value:"done"}});
  expect(records.getAllByRole("article").map(n=>n.getAttribute("aria-label"))).toEqual(["规则13 本次提醒","规则7 本次提醒","规则1 本次提醒"]);
  expect(api.complete).not.toHaveBeenCalled();expect(api.updateStatus).not.toHaveBeenCalled();expect(api.delete).not.toHaveBeenCalled();
  fireEvent.click(rules.getByRole("button",{name:"清除规则筛选"}));
  fireEvent.click(rules.getByRole("button",{name:"下一页"}));
  hook.value.rules=hook.value.rules.slice(0,2);
  view.rerender(<ReminderPanel/>);
  expect(rules.getByText("1 / 1 页 · 2 条")).toBeTruthy();
});

it("没有匹配的分类结果时显示说明，清除筛选恢复规则",()=>{
  render(<ReminderPanel/>);
  const rules=within(screen.getByRole("region",{name:"提醒规则列表"}));
  fireEvent.change(rules.getByRole("combobox",{name:"提醒类型"}),{target:{value:"monthly"}});
  expect(rules.getByText("没有符合筛选条件的提醒规则。")).toBeTruthy();
  expect(rules.queryByRole("article")).toBeNull();
  fireEvent.click(rules.getByRole("button",{name:"清除规则筛选"}));
  expect(rules.getByRole("article",{name:"喝水"})).toBeTruthy();
});
