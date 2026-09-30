import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReminderOccurrence } from "../types";
import MissedReminderSummaryDialog from "./MissedReminderSummaryDialog";

afterEach(cleanup);

const occurrence = (id: string, reminderId: string, name: string): ReminderOccurrence => ({
  id, reminderId, name, content: "提醒内容", scheduledAt: "2032-04-05T01:00:00.000Z",
  status: "expired", deliveryState: "missed", completedAt: null, createdAt: "2032-04-04T01:00:00.000Z",
});

describe("MissedReminderSummaryDialog", () => {
  it("按提醒 ID 去重名称、显示离线原句并调用操作回调", () => {
    const onDismiss = vi.fn();
    const onOpenReminders = vi.fn();
    render(<MissedReminderSummaryDialog occurrences={[
      occurrence("occ-1", "rem-1", "喝水"),
      occurrence("occ-2", "rem-1", "喝水"),
      occurrence("occ-3", "rem-2", "吃药"),
    ]} busy={false} error="" onDismiss={onDismiss} onOpenReminders={onOpenReminders} />);

    const dialog = screen.getByRole("dialog", { name: "离线提醒汇总" });
    expect(dialog.getAttribute("data-pet-interactive")).not.toBeNull();
    expect(dialog.hasAttribute("aria-modal")).toBe(false);
    expect(screen.getByText("你离开这段时间，喝水、吃药过期了！").textContent).toBe("你离开这段时间，喝水、吃药过期了！");
    fireEvent.click(screen.getByRole("button", { name: "知道了" }));
    fireEvent.click(screen.getByRole("button", { name: "查看提醒" }));
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(onOpenReminders).toHaveBeenCalledOnce();
  });

  it("忙碌时禁用操作并显示错误", () => {
    render(<MissedReminderSummaryDialog occurrences={[]} busy error="汇总失败" onDismiss={vi.fn()} onOpenReminders={vi.fn()} />);
    expect(screen.getByRole("alert").textContent).toContain("汇总失败");
    expect((screen.getByRole("button", { name: "知道了" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByRole("dialog")?.tagName).toBe("DIV");
  });
});
