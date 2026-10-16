import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Settings } from "../../types";
import PetTimingPanel from "./PetTimingPanel";

const api = vi.hoisted(() => ({ getSettings: vi.fn(), getSettingsDefaults: vi.fn(), saveSettings: vi.fn() }));
vi.mock("../../api", () => ({ configurationApi: api, errorMessage: (error: { message?: string }) => error.message ?? "读取失败" }));

const keys = [
  "hungry_minutes", "standby_minutes", "idle_event_minutes", "sleep_min_minutes", "sleep_max_minutes",
  "travel_min_online_hours", "travel_max_online_hours",
  "intimacy_threshold_1", "intimacy_threshold_2", "intimacy_threshold_3", "intimacy_threshold_4",
  "accessory_cost_1", "accessory_cost_2", "accessory_cost_3", "accessory_cost_4",
] as const;
const values = ["120", "10", "45", "60", "120", "2", "4", "10", "50", "150", "300", "20", "50", "120", "300"];
const makeSettings = (overrides: Partial<Settings> = {}) => ({
  ...Object.fromEntries(keys.map((key, index) => [key, values[index]])),
  theme_mode: "system",
  short_memory_min_chars: "12",
  ...overrides,
}) as Settings;
const current = makeSettings();

beforeEach(() => {
  vi.clearAllMocks();
  api.getSettings.mockResolvedValue(current);
  api.getSettingsDefaults.mockResolvedValue(makeSettings({ hungry_minutes: "100", intimacy_threshold_1: "15" }));
  api.saveSettings.mockImplementation(async (changes: Partial<Settings>) => makeSettings(changes));
});
afterEach(cleanup);

it("共同触发时间只有一个输入，待机与睡眠持续时间独立保存", async () => {
  render(<PetTimingPanel />);
  const standby = await screen.findByLabelText("待机等待时间");
  const shared = screen.getByLabelText("无聊／睡觉触发时间");
  expect((standby as HTMLInputElement).value).toBe("10");
  expect((shared as HTMLInputElement).value).toBe("45");
  expect(screen.queryByLabelText("无聊触发")).toBeNull();
  expect(screen.queryByLabelText("无互动入睡")).toBeNull();
  expect(screen.getByText(/各50%随机进入无聊或睡觉/)).toBeTruthy();
  fireEvent.change(standby, { target: { value: "5" } });
  fireEvent.change(shared, { target: { value: "35" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ standby_minutes: "5", idle_event_minutes: "35" }));
});

it("读取真实配置并展示全部 15 个可编辑字段", async () => {
  render(<PetTimingPanel />);
  await screen.findByRole("heading", { name: "时间与桌宠" });
  expect((await screen.findByLabelText("饥饿触发")).getAttribute("value")).toBe("120");
  expect(api.getSettings).toHaveBeenCalledTimes(1);
  const fields = keys.map(key => screen.getByLabelText({
    hungry_minutes: "饥饿触发", standby_minutes: "待机等待时间", idle_event_minutes: "无聊／睡觉触发时间",
    sleep_min_minutes: "睡眠最短时长", sleep_max_minutes: "睡眠最长时长",
    travel_min_online_hours: "出行最短在线时长", travel_max_online_hours: "出行最长在线时长",
    intimacy_threshold_1: "档位 1 亲密度", intimacy_threshold_2: "档位 2 亲密度",
    intimacy_threshold_3: "档位 3 亲密度", intimacy_threshold_4: "档位 4 亲密度",
    accessory_cost_1: "档位 1 制作价格", accessory_cost_2: "档位 2 制作价格",
    accessory_cost_3: "档位 3 制作价格", accessory_cost_4: "档位 4 制作价格",
  }[key]));
  expect(fields.length).toBe(15);
});

it("校验整数格式、档位顺序和计时范围后仅保存本页变更", async () => {
  render(<PetTimingPanel />);
  const first = await screen.findByLabelText("档位 1 亲密度");
  fireEvent.change(first, { target: { value: "010" } });
  expect(screen.getByText(/规范的十进制整数/).getAttribute("role")).toBe("alert");
  expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(true);
  fireEvent.change(first, { target: { value: "51" } });
  expect(screen.getByLabelText("档位 2 亲密度").getAttribute("aria-invalid")).toBe("true");
  fireEvent.change(screen.getByLabelText("档位 2 亲密度"), { target: { value: "60" } });
  fireEvent.change(screen.getByLabelText("睡眠最短时长"), { target: { value: "121" } });
  expect(screen.getByLabelText("睡眠最长时长").getAttribute("aria-invalid")).toBe("true");
  fireEvent.change(screen.getByLabelText("睡眠最短时长"), { target: { value: "60" } });
  fireEvent.change(screen.getByLabelText("档位 1 制作价格"), { target: { value: "0" } });
  expect(screen.getByLabelText("档位 1 制作价格").getAttribute("aria-invalid")).toBe("false");
  expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({
    intimacy_threshold_1: "51", intimacy_threshold_2: "60", accessory_cost_1: "0",
  }));
  const submitted = api.saveSettings.mock.calls[0][0] as Record<string, string>;
  expect(Object.keys(submitted).every(key => keys.includes(key as (typeof keys)[number]))).toBe(true);
});

it("保存失败时保留当前草稿", async () => {
  api.saveSettings.mockRejectedValueOnce(new Error("暂时无法保存"));
  render(<PetTimingPanel />);
  const input = await screen.findByLabelText("饥饿触发");
  fireEvent.change(input, { target: { value: "121" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect((await screen.findByRole("alert")).textContent).toContain("暂时无法保存");
  expect(input.getAttribute("value")).toBe("121");
});

it("恢复默认值只填入草稿而不立即保存", async () => {
  render(<PetTimingPanel />);
  await screen.findByLabelText("饥饿触发");
  fireEvent.click(screen.getByRole("button", { name: "恢复默认值" }));
  expect((await screen.findByLabelText("饥饿触发")).getAttribute("value")).toBe("100");
  expect(screen.getByRole("status").textContent).toContain("请点击保存后生效");
  expect(api.saveSettings).not.toHaveBeenCalled();
});

it("卸载后忽略迟到的初次读取结果", async () => {
  let resolveRead!: (settings: Settings) => void;
  api.getSettings.mockReturnValueOnce(new Promise(resolve => { resolveRead = resolve; }));
  const view = render(<PetTimingPanel />);
  view.unmount();
  resolveRead(current);
  await Promise.resolve();
  expect(api.getSettings).toHaveBeenCalledTimes(1);
});

it("初次读取失败时不能提交，并可手动重试读取", async () => {
  api.getSettings.mockRejectedValueOnce(new Error("配置暂不可读"));
  render(<PetTimingPanel />);
  expect((await screen.findByRole("alert")).textContent).toContain("配置暂不可读");
  expect(screen.queryByRole("button", { name: "保存" })).toBeNull();
  expect(api.saveSettings).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "重试读取" }));
  await screen.findByLabelText("饥饿触发");
  fireEvent.change(screen.getByLabelText("饥饿触发"), { target: { value: "121" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ hungry_minutes: "121" }));
});

it("后端字段错误显示在对应输入框并保留草稿，编辑后清除该错误", async () => {
  api.saveSettings.mockRejectedValueOnce({ field: "hungry_minutes", message: "后端拒绝该时长" });
  render(<PetTimingPanel />);
  const input = await screen.findByLabelText("饥饿触发");
  fireEvent.change(input, { target: { value: "121" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(await screen.findByText("后端拒绝该时长")).toBeTruthy();
  expect(input.getAttribute("aria-invalid")).toBe("true");
  expect(input.getAttribute("value")).toBe("121");
  expect(screen.queryByText(/操作失败/)).toBeNull();
  fireEvent.change(input, { target: { value: "122" } });
  expect(screen.queryByText("后端拒绝该时长")).toBeNull();
  expect(input.getAttribute("aria-invalid")).toBe("false");
});
