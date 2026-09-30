import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ReminderOccurrence } from "../types";
import PetReminderNotifications from "./PetReminderNotifications";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), open: vi.fn(), hook: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("../platform/navigation", () => ({ navigation: { open: mocks.open } }));
vi.mock("../hooks/useReminderNotifications", () => ({ default: mocks.hook, noticeKey: (value: { item: ReminderOccurrence } | null) => value?.item.id ?? "" }));
const occurrence: ReminderOccurrence = { id: "1", reminderId: "2", name: "喝水", content: "一杯水", scheduledAt: "2032-01-01T01:00:00Z", status: "pending", deliveryState: "queued", createdAt: "2032-01-01T01:00:00Z", completedAt: null };
let frame = 0;
let frames: Map<number, FrameRequestCallback>;
let value: { notice: { kind: "due"; item: ReminderOccurrence }; pendingIds: string[]; error: string; busy: boolean; shown: ReturnType<typeof vi.fn>; refresh: ReturnType<typeof vi.fn>; dismiss: ReturnType<typeof vi.fn>; complete: ReturnType<typeof vi.fn>; reportError: ReturnType<typeof vi.fn> };
beforeEach(() => {
  vi.clearAllMocks(); frame = 0; frames = new Map();
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { frames.set(++frame, callback); return frame; });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  mocks.invoke.mockResolvedValue(undefined); mocks.open.mockResolvedValue(undefined);
  value = { notice: { kind: "due", item: occurrence }, pendingIds: ["1"], error: "", busy: false,
    shown: vi.fn(async () => {}), refresh: vi.fn(async () => {}), dismiss: vi.fn(async () => true), complete: vi.fn(async () => true), reportError: vi.fn() };
  mocks.hook.mockImplementation(() => value);
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
