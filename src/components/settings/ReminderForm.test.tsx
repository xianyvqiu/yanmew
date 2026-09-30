import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Reminder, ReminderInput } from "../../types";
import ReminderForm, { parseLocalDateTime } from "./ReminderForm";

afterEach(() => { cleanup(); vi.unstubAllEnvs(); });

const reminder: Reminder = {
  id: "reminder-1",
  name: "交报告",
  content: "发送给团队",
  kind: "once",
  schedule: null,
  fireAt: "2032-04-05T06:07:08.456+08:00",
  status: "pending",
  source: "manual",
  createdAt: "2032-01-01T00:00:00.000Z",
};

function renderForm(overrides: Partial<{
  reminder: Reminder | null;
  busy: boolean;
  error: { code: string; message: string; field?: string } | null;
}> = {}) {
  const onSave = vi.fn(async (_input: ReminderInput) => {});
  const onCancel = vi.fn();
  const view = render(<ReminderForm reminder={reminder} busy={false} error={null} onSave={onSave} onCancel={onCancel} {...overrides} />);
  return { ...view, onSave, onCancel };
}

async function submit(container: HTMLElement) {
  await act(async () => { fireEvent.submit(container.querySelector("form")!); });
}

function futureLocalDateTime() {
  const date = new Date();
  date.setFullYear(date.getFullYear() + 1, 0, 15);
  date.setHours(12, 34, 0, 0);
  const pad = (part: number) => String(part).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

describe("ReminderForm", () => {
  it("未改动的一次性时间保留原始 RFC3339 精度与偏移", async () => {
    const { container, onSave } = renderForm();

    await submit(container);

    expect(onSave).toHaveBeenCalledWith({
      name: reminder.name,
      content: reminder.content,
      schedule: null,
      fireAt: reminder.fireAt,
    });
  });

  it("修改一次性时间时输出带时区的 ISO 字符串", async () => {
    const { container, onSave } = renderForm({ reminder: null });
    const value = futureLocalDateTime();
    fireEvent.change(screen.getByLabelText("提醒名称"), { target: { value: "新提醒" } });
    fireEvent.change(screen.getByLabelText("提醒时间"), { target: { value } });

    await submit(container);

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ fireAt: new Date(value).toISOString(), schedule: null }));
  });

  it("已触发的一次性提醒可以只编辑文字并保持 fireAt 为 null", async () => {
    const triggered = { ...reminder, fireAt: null, status: "done" as const };
    const { container, onSave } = renderForm({ reminder: triggered });
    fireEvent.change(screen.getByLabelText("提醒详情"), { target: { value: "更新后的详情" } });

    await submit(container);

    expect(onSave).toHaveBeenCalledWith({ name: reminder.name, content: "更新后的详情", schedule: null, fireAt: null });
    expect(screen.getByText(/留空可只修改文字/)).toBeTruthy();
  });

  it("拒绝无效日历日期和不存在的当地时刻", () => {
    expect(parseLocalDateTime("2032-02-30T12:00")).toBeNull();
    vi.stubEnv("TZ", "America/New_York");
    expect(parseLocalDateTime("2032-03-14T02:30")).toBeNull();
  });

  it("循环规则不传下次时间并保留未改星期顺序", async () => {
    const weekly: Reminder = {
      ...reminder,
      kind: "weekly",
      schedule: { version: 1, repeat: "weekly", time: "08:30", weekdays: [5, 1, 3], next_local_at: "2032-04-05T08:30:00" },
      fireAt: "2032-04-05T00:30:00.000Z",
    };
    const { container, onSave } = renderForm({ reminder: weekly });

    await submit(container);

    expect(onSave).toHaveBeenCalledWith({
      name: weekly.name,
      content: weekly.content,
      schedule: { version: 1, repeat: "weekly", time: "08:30", weekdays: [5, 1, 3] },
      fireAt: null,
    });
  });

  it("每周多选并允许月度日期选择到 31 日", async () => {
    const { container, onSave } = renderForm({ reminder: null });
    fireEvent.change(screen.getByLabelText("提醒名称"), { target: { value: "重复提醒" } });
    fireEvent.change(screen.getByLabelText("提醒类型"), { target: { value: "weekly" } });
    fireEvent.click(screen.getByLabelText("周一"));
    fireEvent.click(screen.getByLabelText("周日"));
    await submit(container);
    expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({
      schedule: { version: 1, repeat: "weekly", time: "09:00", weekdays: [1, 7] }, fireAt: null,
    }));

    fireEvent.change(screen.getByLabelText("提醒类型"), { target: { value: "monthly" } });
    expect(screen.getByLabelText("31日")).toBeTruthy();
    fireEvent.click(screen.getByLabelText("31日"));
    await submit(container);
    expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({
      schedule: { version: 1, repeat: "monthly", time: "09:00", days: [31] }, fireAt: null,
    }));
  });

  it("忙碌时禁用控件并阻止提交，同时字段错误靠近控件且保留草稿", async () => {
    const { container, onSave, rerender } = renderForm({ error: { code: "invalid", message: "名称已占用", field: "name" } });
    const nameInput = screen.getByLabelText("提醒名称");
    fireEvent.change(nameInput, { target: { value: "我的草稿" } });
    expect(nameInput.getAttribute("aria-invalid")).toBe("true");
    const errorId = nameInput.getAttribute("aria-describedby");
    expect(document.getElementById(errorId!)?.textContent).toBe("名称已占用");

    rerender(<ReminderForm reminder={reminder} busy onSave={onSave} onCancel={() => {}} error={null} />);
    expect(container.querySelector("fieldset")?.disabled).toBe(true);
    expect(screen.getByLabelText("提醒名称").matches(":disabled")).toBe(true);
    await submit(container);
    expect(onSave).not.toHaveBeenCalled();
  });
});
