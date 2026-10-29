import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { Settings } from "../../types";
import MemorySettingsPanel from "./MemorySettingsPanel";

const api = vi.hoisted(() => ({ getSettings: vi.fn(), getSettingsDefaults: vi.fn(), saveSettings: vi.fn() }));
vi.mock("../../api", () => ({ configurationApi: api, errorMessage: (error: { message?: string }) => error.message ?? "读取失败" }));

const keys = [
  "short_memory_min_chars", "short_memory_max_chars", "long_memory_min_chars", "long_memory_max_chars",
  "memory_merge_count", "recent_message_count", "recent_long_memory_count",
  "travel_journal_min_chars", "travel_journal_max_chars", "notebook_max_chars",
] as const;
const values = ["12", "100", "150", "250", "20", "30", "30", "250", "350", "3", "4000"];
const makeSettings = (overrides: Partial<Settings> = {}) => ({
  ...Object.fromEntries(keys.map((key, index) => [key, values[index]])),
  theme_mode: "system",
  ...overrides,
}) as Settings;
const current = makeSettings();
const labels = {
  short_memory_min_chars: "短期记忆建议最少字数", short_memory_max_chars: "短期记忆建议最多字数",
  long_memory_min_chars: "长期记忆建议最少字数", long_memory_max_chars: "长期记忆建议最多字数",
  memory_merge_count: "每条长期记忆使用短期记忆数 k", recent_message_count: "进入上下文的最近消息原文数",
  recent_long_memory_count: "直接进入上下文的最近长期记忆数 m", travel_journal_min_chars: "旅行游记最少字数",
  travel_journal_max_chars: "旅行游记最多字数",
  notebook_max_chars: "记事本 AI 维护建议上限",
} satisfies Record<(typeof keys)[number], string>;

beforeEach(() => {
  vi.clearAllMocks();
  api.getSettings.mockResolvedValue(current);
  api.getSettingsDefaults.mockResolvedValue(makeSettings({ short_memory_min_chars: "15", notebook_max_chars: "3500" }));
  api.saveSettings.mockImplementation(async (changes: Partial<Settings>) => makeSettings(changes));
});
afterEach(cleanup);

it("读取真实全局配置并展示全部 11 个字段", async () => {
  render(<MemorySettingsPanel />);
  for (const key of keys) expect(await screen.findByLabelText(labels[key])).toHaveProperty("value", current[key]);
  expect(api.getSettings).toHaveBeenCalledTimes(1);
  expect(screen.getByText("短期记忆和长期记忆是中性事实记录，不使用人设口吻，不需要任何氛围总结或主观评价，必须只是如同剧本一样的叙事描述。")).toBeTruthy();
  expect(screen.getByText(/全局配置；短期记忆、长期记忆、记事本和旅行游记数据仍按会话分别保存/)).toBeTruthy();
});

it("校验正整数上限与三组范围，并只提交本页修改字段", async () => {
  render(<MemorySettingsPanel />);
  const minShort = await screen.findByLabelText(labels.short_memory_min_chars);
  fireEvent.change(minShort, { target: { value: "0" } });
  expect(screen.getByText(/规范的十进制正整数/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(true);
  fireEvent.change(minShort, { target: { value: "12" } });
  fireEvent.change(screen.getByLabelText(labels.recent_message_count), { target: { value: "2147483648" } });
  expect(screen.getByText(/不能超过 2147483647/)).toBeTruthy();
  fireEvent.change(screen.getByLabelText(labels.recent_message_count), { target: { value: "30" } });
  fireEvent.change(minShort, { target: { value: "101" } });
  const maxShort = screen.getByLabelText(labels.short_memory_max_chars);
  expect(maxShort.getAttribute("aria-invalid")).toBe("true");
  expect(maxShort.getAttribute("aria-describedby")).toContain("-error");
  fireEvent.change(minShort, { target: { value: "12" } });
  fireEvent.change(screen.getByLabelText(labels.long_memory_min_chars), { target: { value: "251" } });
  expect(screen.getByLabelText(labels.long_memory_max_chars).getAttribute("aria-invalid")).toBe("true");
  fireEvent.change(screen.getByLabelText(labels.long_memory_min_chars), { target: { value: "150" } });
  fireEvent.change(screen.getByLabelText(labels.travel_journal_min_chars), { target: { value: "351" } });
  expect(screen.getByLabelText(labels.travel_journal_max_chars).getAttribute("aria-invalid")).toBe("true");
  fireEvent.change(screen.getByLabelText(labels.travel_journal_min_chars), { target: { value: "250" } });
  fireEvent.change(screen.getByLabelText(labels.memory_merge_count), { target: { value: "21" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ memory_merge_count: "21" }));
});

it("保存失败时保留草稿，后端字段错误关联到对应输入框", async () => {
  api.saveSettings.mockRejectedValueOnce({ field: "notebook_max_chars", message: "后端拒绝该上限" });
  render(<MemorySettingsPanel />);
  const input = await screen.findByLabelText(labels.notebook_max_chars);
  fireEvent.change(input, { target: { value: "4200" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(await screen.findByText("后端拒绝该上限")).toBeTruthy();
  expect(input).toHaveProperty("value", "4200");
  expect(input.getAttribute("aria-invalid")).toBe("true");
  expect(input.getAttribute("aria-describedby")).toContain("-error");
  fireEvent.change(input, { target: { value: "4300" } });
  expect(screen.queryByText("后端拒绝该上限")).toBeNull();
});

it("恢复服务端默认值只填入草稿，点击保存后才提交", async () => {
  render(<MemorySettingsPanel />);
  const input = await screen.findByLabelText(labels.short_memory_min_chars);
  fireEvent.click(screen.getByRole("button", { name: "恢复默认值" }));
  expect(await screen.findByLabelText(labels.short_memory_min_chars)).toHaveProperty("value", "15");
  expect(screen.getByRole("status").textContent).toContain("请点击保存后生效");
  expect(api.getSettingsDefaults).toHaveBeenCalledTimes(1);
  expect(api.saveSettings).not.toHaveBeenCalled();
});

it("默认值请求期间发生编辑时不覆盖新草稿", async () => {
  let resolveDefaults!: (settings: Settings) => void;
  api.getSettingsDefaults.mockReturnValueOnce(new Promise(resolve => { resolveDefaults = resolve; }));
  render(<MemorySettingsPanel />);
  const input = await screen.findByLabelText(labels.short_memory_min_chars);
  fireEvent.click(screen.getByRole("button", { name: "恢复默认值" }));
  fireEvent.change(input, { target: { value: "18" } });
  resolveDefaults(makeSettings({ short_memory_min_chars: "15" }));
  expect((await screen.findByRole("status")).textContent).toContain("草稿已修改，未覆盖当前内容");
  expect(input).toHaveProperty("value", "18");
});

it("加载失败时不提供空表单保存，并允许重试", async () => {
  api.getSettings.mockRejectedValueOnce(new Error("配置暂不可读"));
  render(<MemorySettingsPanel />);
  expect((await screen.findByRole("alert")).textContent).toContain("配置暂不可读");
  expect(screen.queryByRole("button", { name: "保存" })).toBeNull();
  expect(api.saveSettings).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "重试读取" }));
  const input = await screen.findByLabelText(labels.memory_merge_count);
  fireEvent.change(input, { target: { value: "21" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ memory_merge_count: "21" }));
});

it("卸载后忽略迟到的默认值结果", async () => {
  let resolveDefaults!: (settings: Settings) => void;
  api.getSettingsDefaults.mockReturnValueOnce(new Promise(resolve => { resolveDefaults = resolve; }));
  const view = render(<MemorySettingsPanel />);
  await screen.findByLabelText(labels.memory_merge_count);
  fireEvent.click(screen.getByRole("button", { name: "恢复默认值" }));
  view.unmount();
  resolveDefaults(makeSettings());
  await Promise.resolve();
  expect(api.getSettingsDefaults).toHaveBeenCalledTimes(1);
});
