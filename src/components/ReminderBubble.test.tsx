import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReminderOccurrence } from "../types";
import ReminderBubble from "./ReminderBubble";

afterEach(cleanup);

const occurrence: ReminderOccurrence = {
  id: "occ-1", reminderId: "rem-1", name: "喝水", content: "记得喝一杯水。",
  scheduledAt: "2032-04-05T01:00:00.000Z", status: "pending", deliveryState: "shown",
  completedAt: null, createdAt: "2032-04-04T01:00:00.000Z",
};

describe("ReminderBubble", () => {
  it("显示提醒内容和可访问操作，并调用对应回调", () => {
    const onComplete = vi.fn();
    const onDismiss = vi.fn();
    const onOpenReminders = vi.fn();
    render(<ReminderBubble occurrence={occurrence} busy={false} error="" onComplete={onComplete} onDismiss={onDismiss} onOpenReminders={onOpenReminders} />);

    expect(screen.getByRole("region", { name: "到点提醒" }).getAttribute("data-pet-interactive")).not.toBeNull();
    expect(screen.getByText(/喝水事件提醒：记得喝一杯水。/).textContent).toContain("喝水事件提醒：记得喝一杯水。");
    fireEvent.click(screen.getByRole("button", { name: "完成" }));
    fireEvent.click(screen.getByRole("button", { name: "收起" }));
    fireEvent.click(screen.getByRole("button", { name: "查看提醒" }));

    expect(onComplete).toHaveBeenCalledOnce();
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(onOpenReminders).toHaveBeenCalledOnce();
  });

  it("完成状态隐藏，忙碌时禁用操作并显示错误", () => {
    const { rerender } = render(<ReminderBubble occurrence={occurrence} busy={true} error="保存失败" onComplete={vi.fn()} onDismiss={vi.fn()} onOpenReminders={vi.fn()} />);
    expect(screen.getByRole("alert").textContent).toContain("保存失败");
    expect((screen.getByRole("button", { name: "完成" }) as HTMLButtonElement).disabled).toBe(true);

    rerender(<ReminderBubble occurrence={{ ...occurrence, status: "done" }} busy={false} error="" onComplete={vi.fn()} onDismiss={vi.fn()} onOpenReminders={vi.fn()} />);
    expect(screen.queryByRole("region", { name: "到点提醒" })).toBeNull();
  });
});
