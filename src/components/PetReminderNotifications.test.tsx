import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ReminderOccurrence } from "../types";
import PetReminderNotifications from "./PetReminderNotifications";
import PetStateBubble from "./PetStateBubble";
import type { PetActivity, PetStateNotice } from "../types";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), open: vi.fn(), hook: vi.fn(), listenStateBubble: vi.fn(), stopStateBubble: vi.fn() }));
vi.mock("../api", () => ({ petApi: { listenStateBubble: mocks.listenStateBubble } }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../platform/navigation", () => ({ navigation: { open: mocks.open } }));
vi.mock("../hooks/useReminderNotifications", () => ({ default: mocks.hook, noticeKey: (value: { item: ReminderOccurrence } | null) => value?.item.id ?? "" }));
const occurrence: ReminderOccurrence = { id: "1", reminderId: "2", name: "喝水", content: "一杯水", scheduledAt: "2032-01-01T01:00:00Z", status: "pending", deliveryState: "queued", createdAt: "2032-01-01T01:00:00Z", completedAt: null };
it("一分钟后只收起，文案更新不重置显示计时",async()=>{
  vi.useFakeTimers();
  try{
    value.notice={kind:"due",item:{...occurrence,status:"done",autoCompleted:true}};value.dismiss.mockResolvedValue(true);
    const view=render(<PetReminderNotifications ready hidden={false} unobscured/>);
    await act(async()=>{vi.advanceTimersByTime(30_000);});expect(value.dismiss).not.toHaveBeenCalled();
    value.notice={kind:"due",item:{...occurrence,status:"done",autoCompleted:true,content:"更新了显示内容"}};
    view.rerender(<PetReminderNotifications ready hidden={false} unobscured/>);
    await act(async()=>{vi.advanceTimersByTime(29_999);});expect(value.dismiss).not.toHaveBeenCalled();
    await act(async()=>{vi.advanceTimersByTime(1);});expect(value.dismiss).toHaveBeenCalledOnce();expect(value.complete).not.toHaveBeenCalled();
  }finally{vi.useRealTimers();}
});
let frame = 0;
let frames: Map<number, FrameRequestCallback>;
let value: { notice: { kind: "due"; item: ReminderOccurrence } | null; pendingIds: string[]; error: string; busy: boolean; shown: ReturnType<typeof vi.fn>; refresh: ReturnType<typeof vi.fn>; dismiss: ReturnType<typeof vi.fn>; complete: ReturnType<typeof vi.fn>; reportError: ReturnType<typeof vi.fn> };
let receiveStateBubble: (notice: PetStateNotice) => void;
const hungry: PetActivity = {
  personaId: "1", sessionId: "1", contextRevision: 0, revision: 2, activity: "hungry",
  expression: "(・ω・)", intimacy: 1, sleepUntil: null, error: null, canInteract: true, canWander: true,
};
const hungryNotice: PetStateNotice = {
  personaId: "1", sessionId: "1", contextRevision: 0, revision: 2, activity: "hungry", text: "好饿…",
};
beforeEach(() => {
  vi.clearAllMocks(); frame = 0; frames = new Map();
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { frames.set(++frame, callback); return frame; });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  mocks.invoke.mockResolvedValue(undefined); mocks.open.mockResolvedValue(undefined);
  value = { notice: { kind: "due", item: occurrence }, pendingIds: ["1"], error: "", busy: false,
    shown: vi.fn(async () => {}), refresh: vi.fn(async () => {}), dismiss: vi.fn(async () => true), complete: vi.fn(async () => true), reportError: vi.fn() };
  mocks.hook.mockImplementation(() => value);
  mocks.listenStateBubble.mockImplementation(async callback => { receiveStateBubble = callback; return mocks.stopStateBubble; });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
async function paint() {
  await act(async () => { for (let count = 0; count < 2; count++) { const batch = [...frames.values()]; frames.clear(); batch.forEach(callback => callback(0)); } });
}

it("隐藏时新提醒恢复宠窗一次，真正可见后才确认展示", async () => {
  const view = render(<PetReminderNotifications ready hidden unobscured />);
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("set_pet_hidden", { hidden: false }));
  expect(screen.queryByRole("region", { name: "到点提醒" })).toBeNull();
  await paint(); expect(value.shown).not.toHaveBeenCalled();
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured />);
  await paint(); expect(value.shown).toHaveBeenCalledTimes(1);
  view.rerender(<PetReminderNotifications ready hidden unobscured />);
  expect(mocks.invoke).toHaveBeenCalledTimes(1);
});

it("其他弹层遮挡或提交尚未结束时不确认，解除后补确认", async () => {
  const view = render(<PetReminderNotifications ready hidden={false} unobscured={false} />);
  await paint(); expect(value.shown).not.toHaveBeenCalled();
  value.busy = true;
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured />);
  await paint(); expect(value.shown).not.toHaveBeenCalled();
  value.busy = false;
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured />);
  await paint(); expect(value.shown).toHaveBeenCalledTimes(1);
});

it("查看提醒导航失败保留气泡，成功后才收起", async () => {
  mocks.open.mockRejectedValueOnce({ message: "窗口打开失败" });
  render(<PetReminderNotifications ready hidden={false} unobscured />);
  fireEvent.click(screen.getByRole("button", { name: "查看提醒" }));
  await waitFor(() => expect(value.reportError).toHaveBeenCalled());
  expect(value.dismiss).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "查看提醒" }));
  await waitFor(() => expect(value.dismiss).toHaveBeenCalledTimes(1));
  expect(mocks.open).toHaveBeenLastCalledWith("reminders");
});

it("提醒到来撤下普通状态提示，收起提醒后不补弹旧提示", async () => {
  value.notice = null; value.pendingIds = [];
  const bubble = <PetStateBubble view={hungry} durationMs={8000} />;
  const view = render(<PetReminderNotifications ready hidden={false} unobscured stateBubble={bubble} />);
  await act(async () => {});
  act(() => receiveStateBubble(hungryNotice));
  expect(screen.getByRole("status").textContent).toContain("好饿…");
  value.notice = { kind: "due", item: occurrence }; value.pendingIds = ["1"];
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured stateBubble={bubble} />);
  expect(screen.getByRole("region", { name: "到点提醒" })).toBeTruthy();
  expect(screen.queryByRole("status")).toBeNull();
  expect(mocks.stopStateBubble).toHaveBeenCalledOnce();
  // 被卸载的旧订阅即使晚到，也不能复活被略过的普通提示。
  const oldReceive = receiveStateBubble;
  value.notice = null; value.pendingIds = [];
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured stateBubble={bubble} />);
  await act(async () => { oldReceive(hungryNotice); });
  expect(screen.queryByRole("status")).toBeNull();
});

it("隐藏和拖动撤下普通提示，恢复时不补弹也不自动唤醒宠窗", async () => {
  value.notice = null; value.pendingIds = [];
  const bubble = <PetStateBubble view={hungry} durationMs={8000} />;
  const view = render(<PetReminderNotifications ready hidden={false} unobscured stateBubble={bubble} />);
  await act(async () => {});
  act(() => receiveStateBubble(hungryNotice));
  const oldReceive = receiveStateBubble;
  view.rerender(<PetReminderNotifications ready hidden unobscured stateBubble={bubble} />);
  expect(screen.queryByRole("status")).toBeNull();
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured stateBubble={bubble} />);
  await act(async () => { oldReceive(hungryNotice); });
  expect(screen.queryByRole("status")).toBeNull();
  act(() => receiveStateBubble(hungryNotice));
  expect(screen.getByRole("status")).toBeTruthy();
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured stateBubble={false} />);
  expect(screen.queryByRole("status")).toBeNull();
  view.rerender(<PetReminderNotifications ready hidden={false} unobscured stateBubble={bubble} />);
  await act(async () => {});
  expect(screen.queryByRole("status")).toBeNull();
  expect(mocks.invoke).not.toHaveBeenCalled();
});

it("知道了直接收起当前提醒，不调用完成接口",async()=>{
 value.notice={kind:"due",item:{...occurrence,status:"done",autoCompleted:true}};
 render(<PetReminderNotifications ready hidden={false} unobscured/>);
 await act(async()=>{screen.getByRole("button",{name:"知道了"}).click();});
 expect(value.dismiss).toHaveBeenCalledWith("1");expect(value.complete).not.toHaveBeenCalled();
});
