import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { WeatherSettings, WeatherView } from "../../types";
import WeatherPanel from "./WeatherPanel";

const api = vi.hoisted(() => ({ get: vi.fn(), refresh: vi.fn(), cancel: vi.fn(), getWeatherSettings: vi.fn(), saveWeatherSettings: vi.fn() }));
vi.mock("../../api", () => ({
  weatherApi: { get: api.get, refresh: api.refresh, cancel: api.cancel },
  configurationApi: { getWeatherSettings: api.getWeatherSettings, saveWeatherSettings: api.saveWeatherSettings },
  errorMessage: (error: { message: string }) => error.message,
}));

const settings: WeatherSettings = { apiHost: "https://example.qweatherapi.com", hasApiKey: true };
const weather = (overrides: Partial<WeatherView> = {}): WeatherView => ({
  contextRevision: 7,
  availability: "cached",
  data: {
    position: { latitude: 31.2304, longitude: 121.4737, locatedAt: "2032-04-05T08:00:00.000Z" },
    observation: { observedAt: "2032-04-05T08:05:00.000Z", description: "晴", temperatureCelsius: 21, source: "openMeteo" },
    fetchedAt: "2032-04-05T08:06:00.000Z",
  },
  error: null,
  ...overrides,
});
let cached: WeatherView;

beforeEach(() => {
  vi.clearAllMocks();
  cached = weather();
  api.get.mockImplementation(async () => cached);
  api.refresh.mockResolvedValue(weather({ availability: "available" }));
  api.cancel.mockResolvedValue(undefined);
  api.getWeatherSettings.mockResolvedValue(settings);
  api.saveWeatherSettings.mockResolvedValue(settings);
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

it("打开时只读缓存，不自动定位；展示缓存坐标、来源与观测信息", async () => {
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("状态：缓存天气");

  expect(api.get).toHaveBeenCalledTimes(1);
  expect(api.refresh).not.toHaveBeenCalled();
  expect(screen.getByText("31.2304, 121.4737")).toBeTruthy();
  expect(screen.getByText("Open-Meteo")).toBeTruthy();
  expect(screen.getByText("21 °C")).toBeTruthy();
});

it("刷新拒绝时保留旧缓存和位置，并显示失败原因", async () => {
  api.refresh.mockRejectedValue({ message: "定位权限被拒绝" });
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("31.2304, 121.4737");
  fireEvent.click(screen.getByRole("button", { name: "刷新天气" }));

  expect(await screen.findByText("刷新天气失败：定位权限被拒绝")).toBeTruthy();
  expect(screen.getByText("31.2304, 121.4737")).toBeTruthy();
});

it("取消后忽略较晚返回的天气结果并保留缓存", async () => {
  let finish!: (view: WeatherView) => void;
  api.refresh.mockImplementation(() => new Promise<WeatherView>(resolve => { finish = resolve; }));
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("31.2304, 121.4737");
  fireEvent.click(screen.getByRole("button", { name: "刷新天气" }));
  fireEvent.click(await screen.findByRole("button", { name: "取消刷新" }));
  await waitFor(() => expect(api.cancel).toHaveBeenCalledTimes(1));

  await act(async () => finish(weather({ availability: "available", data: { ...weather().data!, position: { latitude: 1, longitude: 2, locatedAt: "2032-04-05T08:00:00.000Z" } } })));
  expect(screen.getByText("31.2304, 121.4737")).toBeTruthy();
  expect(screen.queryByText("1, 2")).toBeNull();
});

it("卸载时取消进行中的页面刷新，并丢弃晚返回结果", async () => {
  let finish!: (view: WeatherView) => void;
  api.refresh.mockImplementation(() => new Promise<WeatherView>(resolve => { finish = resolve; }));
  const view = render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("31.2304, 121.4737");
  fireEvent.click(screen.getByRole("button", { name: "刷新天气" }));
  view.unmount();

  expect(api.cancel).toHaveBeenCalledTimes(1);
  await act(async () => finish(weather({ availability: "available" })));
  expect(screen.queryByText("状态：最新天气")).toBeNull();
});

it("不显示 revision 不匹配的缓存", async () => {
  api.get.mockResolvedValue(weather({ contextRevision: 6 }));
  render(<WeatherPanel contextRevision={7} />);

  expect(await screen.findByText("缓存属于其他会话上下文，已隐藏；请刷新当前天气。")).toBeTruthy();
  expect(screen.queryByText("31.2304, 121.4737")).toBeNull();
});

it("在当前 revision 下显示 null 温度", async () => {
  api.get.mockResolvedValue(weather({ data: { ...weather().data!, observation: { ...weather().data!.observation, temperatureCelsius: null } } }));
  render(<WeatherPanel contextRevision={7} />);

  expect(await screen.findByText("暂无温度数据")).toBeTruthy();
  expect(screen.getByText("31.2304, 121.4737")).toBeTruthy();
});

it("配置初读失败时禁止保存、允许刷新，并可重试读取配置", async () => {
  api.getWeatherSettings.mockRejectedValueOnce({ message: "配置暂不可读" });
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("读取天气配置失败：配置暂不可读");

  expect(screen.getByRole("button", { name: "保存天气配置" }).matches(":disabled")).toBe(true);
  expect(screen.getByRole("button", { name: "刷新天气" }).matches(":disabled")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "刷新天气" }));
  await screen.findByText("状态：最新天气");
  expect(api.saveWeatherSettings).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "重试读取配置" }));
  await waitFor(() => expect(api.getWeatherSettings).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.getByRole("button", { name: "保存天气配置" }).matches(":disabled")).toBe(false));
});

it("刷新失败期间的慢缓存读取不能覆盖失败状态或显示旧天气", async () => {
  let finishCache!: (view: WeatherView) => void;
  api.get.mockImplementationOnce(() => new Promise<WeatherView>(resolve => { finishCache = resolve; }));
  api.refresh.mockRejectedValue({ message: "定位失败" });
  render(<WeatherPanel contextRevision={7} />);
  fireEvent.click(screen.getByRole("button", { name: "刷新天气" }));

  expect(await screen.findByText("刷新天气失败：定位失败")).toBeTruthy();
  await act(async () => finishCache(weather()));
  expect(screen.queryByText("31.2304, 121.4737")).toBeNull();
  expect(screen.getByText("刷新天气失败：定位失败")).toBeTruthy();
});

it("取消接口失败时不报告取消并保持刷新锁，可再次取消", async () => {
  api.cancel.mockRejectedValueOnce({ message: "取消暂不可用" }).mockResolvedValueOnce(undefined);
  let finishRefresh!: (view: WeatherView) => void;
  api.refresh.mockImplementation(() => new Promise<WeatherView>(resolve => { finishRefresh = resolve; }));
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("31.2304, 121.4737");
  fireEvent.click(screen.getByRole("button", { name: "刷新天气" }));
  fireEvent.click(await screen.findByRole("button", { name: "取消刷新" }));
  expect(await screen.findByText("取消天气刷新失败：取消暂不可用")).toBeTruthy();
  expect(screen.queryByText("已取消天气刷新；现有缓存已保留。")).toBeNull();
  expect(screen.getByRole("button", { name: "正在刷新…" }).matches(":disabled")).toBe(true);

  fireEvent.click(screen.getByRole("button", { name: "取消刷新" }));
  await screen.findByText("已取消天气刷新；现有缓存已保留。");
  await act(async () => finishRefresh(weather({ availability: "available" })));
  expect(screen.getByText("31.2304, 121.4737")).toBeTruthy();
  expect(screen.queryByText("状态：最新天气")).toBeNull();
});

it("保存失败保留 Host 和密钥草稿；保存成功提交 set 并清空密钥输入", async () => {
  api.saveWeatherSettings.mockRejectedValueOnce({ field: "apiHost", message: "Host 不可用" });
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("31.2304, 121.4737");
  fireEvent.change(screen.getByLabelText("和风天气 API Host"), { target: { value: "https://custom.qweatherapi.com" } });
  fireEvent.change(screen.getByLabelText("和风天气 API Key"), { target: { value: "new-secret" } });
  fireEvent.click(screen.getByRole("button", { name: "保存天气配置" }));

  expect(await screen.findByText("Host 不可用")).toBeTruthy();
  expect(screen.getByLabelText("和风天气 API Host")).toHaveProperty("value", "https://custom.qweatherapi.com");
  expect(screen.getByLabelText("和风天气 API Key")).toHaveProperty("value", "new-secret");
  expect(api.saveWeatherSettings).toHaveBeenCalledWith({ apiHost: "https://custom.qweatherapi.com", apiKey: { action: "set", value: "new-secret" } });

  api.saveWeatherSettings.mockResolvedValueOnce({ apiHost: "https://custom.qweatherapi.com", hasApiKey: true });
  fireEvent.click(screen.getByRole("button", { name: "保存天气配置" }));
  await screen.findByText(/查询仍优先使用 Open-Meteo/);
  expect(screen.getByLabelText("和风天气 API Key")).toHaveProperty("value", "");
});

it("保留密钥和独立清除密钥分别提交 keep 与 clear", async () => {
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("31.2304, 121.4737");
  fireEvent.click(screen.getByRole("button", { name: "保存天气配置" }));
  await waitFor(() => expect(api.saveWeatherSettings).toHaveBeenCalledWith({ apiHost: settings.apiHost, apiKey: { action: "keep" } }));

  fireEvent.click(screen.getByRole("button", { name: "清除已保存密钥" }));
  fireEvent.click(screen.getByRole("button", { name: "保存天气配置" }));
  await waitFor(() => expect(api.saveWeatherSettings).toHaveBeenLastCalledWith({ apiHost: settings.apiHost, apiKey: { action: "clear" } }));
});

it("配置保存后立即隐藏被清理的缓存，缓存重读失败时不恢复旧天气", async () => {
  render(<WeatherPanel contextRevision={7} />);
  await screen.findByText("31.2304, 121.4737");
  api.get.mockRejectedValueOnce(new Error("缓存暂不可读"));
  fireEvent.click(screen.getByRole("button", { name: "保存天气配置" }));

  expect(await screen.findByText("配置已保存，但读取天气缓存失败：缓存暂不可读")).toBeTruthy();
  expect(screen.queryByText("31.2304, 121.4737")).toBeNull();
});
