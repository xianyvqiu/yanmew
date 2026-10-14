import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { AutostartStatus } from "../../types";
import AutostartPanel from "./AutostartPanel";
import GeneralPanel from "./GeneralPanel";
vi.mock("./BackupPanel", () => ({ default: () => null }));

vi.mock("./AppInfoPanel", () => ({ default: () => null }));

const api = vi.hoisted(() => ({ get: vi.fn(), set: vi.fn(), getSettings: vi.fn(), saveSettings: vi.fn() }));
vi.mock("../../api", () => ({
  autostartApi: { get: api.get, set: api.set },
  configurationApi: { getSettings: api.getSettings, saveSettings: api.saveSettings },
  errorMessage: (cause: { message: string }) => cause.message,
}));

const status = (overrides: Partial<AutostartStatus> = {}): AutostartStatus => ({
  supported: true, enabled: false, needsRepair: false, blockedBySystem: false, ...overrides,
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((resolveValue, rejectValue) => { resolve = resolveValue; reject = rejectValue; });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.resetAllMocks();
  api.get.mockResolvedValue(status());
  api.set.mockImplementation(async (enabled: boolean) => status({ enabled }));
  api.getSettings.mockResolvedValue({ theme_mode: "system" });
  api.saveSettings.mockResolvedValue({ theme_mode: "system" });
});
afterEach(cleanup);

it("首次读取前显示未知并禁用开关；未登记默认关闭且不写入", async () => {
  const read = deferred<AutostartStatus>();
  api.get.mockReturnValue(read.promise);
  render(<AutostartPanel />);
  expect(screen.getByText("开机自启状态：未知")).toBeTruthy();
  expect(screen.getByText("正在读取开机自启状态…")).toBeTruthy();
  expect((screen.getByRole("switch", { name: "开机自启" }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => read.resolve(status()));
  expect(screen.getByText("开机自启状态：已关闭")).toBeTruthy();
  expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("false");
  expect(api.get).toHaveBeenCalledTimes(1);
  expect(api.set).not.toHaveBeenCalled();
});

it("通用设置保留主题表单并接入独立自启面板", async () => {
  render(<GeneralPanel />);
  await screen.findByRole("combobox", { name: "主题" });
  await screen.findByText("开机自启状态：已关闭");
  fireEvent.click(screen.getByRole("button", { name: "保存主题" }));
  expect(await screen.findByText("主题已保存")).toBeTruthy();
  expect(api.saveSettings).toHaveBeenCalledWith({ theme_mode: "system" });
  expect(api.set).not.toHaveBeenCalled();
});

it("不支持的平台隐藏自启入口", async () => {
  api.get.mockResolvedValue(status({ supported: false }));
  render(<AutostartPanel />);
  await waitFor(() => expect(screen.queryByRole("region", { name: "开机自启" })).toBeNull());
  expect(api.set).not.toHaveBeenCalled();
});

it("主动开启后按真实返回显示开启并防止重复提交", async () => {
  const write = deferred<AutostartStatus>();
  api.set.mockReturnValue(write.promise);
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已关闭");
  fireEvent.click(screen.getByRole("switch"));
  fireEvent.click(screen.getByRole("switch"));
  expect((screen.getByRole("switch") as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByText("正在设置…")).toBeTruthy();
  expect(api.set).toHaveBeenCalledTimes(1);
  expect(api.set).toHaveBeenCalledWith(true);
  await act(async () => write.resolve(status({ enabled: true })));
  expect(screen.getByText("开机自启状态：已开启")).toBeTruthy();
  expect(screen.getByText("开机自启已开启。")).toBeTruthy();
});

it("主动关闭后按真实返回显示关闭", async () => {
  api.get.mockResolvedValue(status({ enabled: true }));
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已开启");
  fireEvent.click(screen.getByRole("switch"));
  expect(await screen.findByText("开机自启已关闭。")).toBeTruthy();
  expect(api.set).toHaveBeenCalledWith(false);
  expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("false");
});

it("开启请求返回关闭时不伪造成功并允许重新读取", async () => {
  api.set.mockResolvedValue(status());
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已关闭");
  fireEvent.click(screen.getByRole("switch"));
  expect(await screen.findByText("开机自启状态未达到预期，请重新读取后再试。")).toBeTruthy();
  expect(screen.queryByText("开机自启已开启。")).toBeNull();
  expect(screen.getByText("开机自启状态：已关闭")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
  await waitFor(() => expect(api.get).toHaveBeenCalledTimes(2));
});

it("关闭请求仍返回开启时不伪造关闭成功", async () => {
  api.get.mockResolvedValue(status({ enabled: true }));
  api.set.mockResolvedValue(status({ enabled: true }));
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已开启");
  fireEvent.click(screen.getByRole("switch"));
  await screen.findByText("开机自启状态未达到预期，请重新读取后再试。");
  expect(screen.queryByText("开机自启已关闭。")).toBeNull();
  expect(screen.getByText("开机自启状态：已开启")).toBeTruthy();
});

it("路径异常只读提示，明确点击修复才登记当前路径", async () => {
  api.get.mockResolvedValue(status({ needsRepair: true }));
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：路径待修复");
  expect(screen.getByText(/便携版目录已变更或登记路径异常/)).toBeTruthy();
  expect(api.set).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "修复自启路径" }));
  expect(await screen.findByText("开机自启已开启。")).toBeTruthy();
  expect(api.set).toHaveBeenCalledWith(true);
  expect(screen.queryByRole("button", { name: "修复自启路径" })).toBeNull();
});

it("路径异常仍允许关闭旧登记", async () => {
  api.get.mockResolvedValue(status({ needsRepair: true }));
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：路径待修复");
  fireEvent.click(screen.getByRole("switch"));
  expect(await screen.findByText("开机自启已关闭。")).toBeTruthy();
  expect(api.set).toHaveBeenCalledWith(false);
});

it("系统禁用时提示任务管理器，不自动修改且允许关闭登记", async () => {
  api.get.mockResolvedValue(status({ blockedBySystem: true }));
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已被系统禁用");
  expect(screen.getByText(/请在 Windows 任务管理器/)).toBeTruthy();
  expect(api.set).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("switch"));
  expect(await screen.findByText("开机自启已关闭。")).toBeTruthy();
  expect(api.set).toHaveBeenCalledWith(false);
});

it("开启返回系统禁用时不显示已开启成功", async () => {
  api.set.mockResolvedValue(status({ blockedBySystem: true }));
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已关闭");
  fireEvent.click(screen.getByRole("switch"));
  await screen.findByText("开机自启状态：已被系统禁用");
  expect(screen.queryByText("开机自启已开启。")).toBeNull();
});

it("读取失败显示未知且重试只查询，不把失败显示成关闭", async () => {
  api.get.mockRejectedValueOnce({ message: "注册表读取失败" });
  render(<AutostartPanel />);
  await screen.findByText("读取开机自启状态失败：注册表读取失败");
  expect(screen.getByText("开机自启状态：未知")).toBeTruthy();
  expect(screen.queryByText("开机自启状态：已关闭")).toBeNull();
  expect((screen.getByRole("switch") as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
  await screen.findByText("开机自启状态：已关闭");
  expect(api.get).toHaveBeenCalledTimes(2);
  expect(api.set).not.toHaveBeenCalled();
});

it("写入失败重新查询真实状态，即使状态已改变也不显示成功", async () => {
  api.get.mockResolvedValueOnce(status()).mockResolvedValueOnce(status({ enabled: true }));
  api.set.mockRejectedValue({ message: "写入确认失败" });
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已关闭");
  fireEvent.click(screen.getByRole("switch"));
  await screen.findByText("设置开机自启失败：写入确认失败");
  await screen.findByText("开机自启状态：已开启");
  expect(api.get).toHaveBeenCalledTimes(2);
  expect(screen.queryByText("开机自启已开启。")).toBeNull();
});

it("写入失败且补查失败清除旧状态，读取期间所有操作禁用", async () => {
  const read = deferred<AutostartStatus>();
  api.get.mockResolvedValueOnce(status({ enabled: true })).mockReturnValueOnce(read.promise);
  api.set.mockRejectedValue({ message: "无法删除登记" });
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已开启");
  fireEvent.click(screen.getByRole("switch"));
  await screen.findByText("正在读取开机自启状态…");
  expect((screen.getByRole("switch") as HTMLButtonElement).disabled).toBe(true);
  expect((screen.getByRole("button", { name: "重新读取" }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => read.reject({ message: "查询失败" }));
  expect(screen.getByText("开机自启状态：未知")).toBeTruthy();
  expect(screen.queryByText("开机自启状态：已开启")).toBeNull();
  expect(screen.getByText("设置开机自启失败：无法删除登记；重新读取状态失败：查询失败")).toBeTruthy();
  expect(screen.queryByText("开机自启已关闭。")).toBeNull();
});

it("焦点刷新保留旧状态并禁用操作，读取失败则显示未知", async () => {
  const read = deferred<AutostartStatus>();
  api.get.mockResolvedValueOnce(status({ enabled: true })).mockReturnValueOnce(read.promise);
  render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已开启");
  fireEvent.focus(window);
  expect(screen.getByText("开机自启状态：已开启")).toBeTruthy();
  expect(screen.getByText("正在读取开机自启状态…")).toBeTruthy();
  expect((screen.getByRole("switch") as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("switch"));
  expect(api.set).not.toHaveBeenCalled();
  await act(async () => read.reject({ message: "焦点刷新失败" }));
  expect(screen.getByText("开机自启状态：未知")).toBeTruthy();
  expect(screen.queryByText("开机自启状态：已开启")).toBeNull();
});

it("焦点刷新读取系统新状态且忽略较早的迟到响应", async () => {
  const oldRead = deferred<AutostartStatus>();
  api.get.mockReturnValueOnce(oldRead.promise).mockResolvedValueOnce(status({ blockedBySystem: true }));
  render(<AutostartPanel />);
  fireEvent.focus(window);
  await screen.findByText("开机自启状态：已被系统禁用");
  await act(async () => oldRead.resolve(status({ enabled: true })));
  expect(screen.getByText("开机自启状态：已被系统禁用")).toBeTruthy();
  expect(api.set).not.toHaveBeenCalled();
});

it("写入期间焦点不发起竞争读取，旧读取也不能覆盖写入结果", async () => {
  const oldRead = deferred<AutostartStatus>();
  const write = deferred<AutostartStatus>();
  api.get.mockReturnValueOnce(oldRead.promise).mockResolvedValueOnce(status());
  api.set.mockReturnValue(write.promise);
  render(<AutostartPanel />);
  fireEvent.focus(window);
  await screen.findByText("开机自启状态：已关闭");
  fireEvent.click(screen.getByRole("switch"));
  fireEvent.focus(window);
  expect(api.get).toHaveBeenCalledTimes(2);
  await act(async () => write.resolve(status({ enabled: true })));
  await act(async () => oldRead.resolve(status()));
  expect(screen.getByText("开机自启状态：已开启")).toBeTruthy();
  expect(screen.getByText("开机自启已开启。")).toBeTruthy();
});

it("卸载后迟到读取不更新页面，并移除焦点监听", async () => {
  const read = deferred<AutostartStatus>();
  api.get.mockReturnValue(read.promise);
  const view = render(<AutostartPanel />);
  view.unmount();
  await act(async () => read.resolve(status({ enabled: true })));
  fireEvent.focus(window);
  expect(api.get).toHaveBeenCalledTimes(1);
  expect(screen.queryByText("开机自启状态：已开启")).toBeNull();
});

it("卸载后写入失败不补查也不更新页面", async () => {
  const write = deferred<AutostartStatus>();
  api.set.mockReturnValue(write.promise);
  const view = render(<AutostartPanel />);
  await screen.findByText("开机自启状态：已关闭");
  fireEvent.click(screen.getByRole("switch"));
  view.unmount();
  await act(async () => write.reject({ message: "迟到失败" }));
  expect(api.get).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("alert")).toBeNull();
});
