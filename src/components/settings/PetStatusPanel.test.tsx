import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import PetStatusPanel from "./PetStatusPanel";
import type { WorkspaceContext } from "../../hooks/useCurrentContext";
import type { DailySignInResult, SessionIntimacy, SignInStatus } from "../../types";

const api = vi.hoisted(() => ({ status: vi.fn(), signIn: vi.fn(), intimacy: vi.fn(),
  festivalListen: vi.fn(), walletListen: vi.fn(), configListen: vi.fn(), petListen: vi.fn(), chatListen: vi.fn(), messagesListen: vi.fn() }));
vi.mock("../../api", () => ({
  walletApi: { signInStatus: api.status, signIn: api.signIn, intimacy: api.intimacy, listen: api.walletListen },
  festivalsApi: { listen: api.festivalListen },
  configurationApi: { listenChanges: api.configListen }, petApi: { listen: api.petListen },
  chatApi: { listen: api.chatListen, listenMessages: api.messagesListen },
  errorMessage: (cause: unknown) => cause instanceof Error ? cause.message : String(cause),
}));
function snapshot(sessionId: string | null = "2", contextRevision = 1): WorkspaceContext {
  return { context: { personaId: "1", sessionId, contextRevision }, sessions: [], userProfiles: [],
    persona: { id: "1", name: "电子小猫", prompt: "", initialPrompt: "", selfEvolveEnabled: false,
      lastEvolvedAt: null, color: "#777777", chatBgFile: null, chatBgOpacity: 85, chatBgBlur: 0,
      isActive: true, isBuiltin: true, expressions: null } };
}
function status(coins = "0", signedIn = false, localDate = "2026-10-08"): SignInStatus {
  return { wallet: { coins, updatedAt: "2026-10-08T03:00:00Z" }, signedIn, localDate,
    rewardMin: "1", rewardMax: "10", intimacyThresholds: [10, 50, 150, 300], festivals: [] };
}
function intimacy(value = 42, sessionId = "2", contextRevision = 1): SessionIntimacy {
  return { personaId: "1", sessionId, contextRevision, intimacy: value, maximum: 999 };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(finish => { resolve = finish; });
  return { promise, resolve };
}
const listeners = [api.festivalListen, api.walletListen, api.configListen, api.petListen, api.chatListen, api.messagesListen];
beforeEach(() => {
  vi.resetAllMocks();
  api.status.mockResolvedValue(status()); api.intimacy.mockResolvedValue(intimacy());
  api.signIn.mockResolvedValue({ status: status("3", true), grantedCoins: "3" });
  listeners.forEach(listen => listen.mockResolvedValue(vi.fn()));
});
afterEach(() => { cleanup(); vi.useRealTimers(); });
async function show(value = snapshot()) {
  let result!: ReturnType<typeof render>;
  await act(async () => { result = render(<PetStatusPanel snapshot={value} />); });
  return result;
}

it("展示完整大整数余额、当前亲密度和下一档位，上限不隐藏状态", async () => {
  api.status.mockResolvedValue(status("9223372036854775807"));
  const view = await show();
  expect(screen.getByLabelText("喵币余额").textContent).toContain("9223372036854775807");
  expect(screen.getByLabelText("当前亲密度").textContent).toBe("42 / 999");
  expect(screen.getByText(/第 2 档 · 50 亲密度（还差 8）/)).toBeTruthy();
  api.intimacy.mockResolvedValue(intimacy(999, "3", 2));
  await act(async () => { view.rerender(<PetStatusPanel snapshot={snapshot("3", 2)} />); });
  expect(screen.getByText("已达上限")).toBeTruthy();
  expect(screen.getByText("所有亲密度档位已解锁。")).toBeTruthy();
});

it("无会话仍能签到，等待后台提交才更新余额，连点不重复请求", async () => {
  const request = deferred<DailySignInResult>(); api.signIn.mockReturnValue(request.promise);
  await show(snapshot(null));
  expect(api.intimacy).not.toHaveBeenCalled();
  expect(screen.getByText(/创建会话后/)).toBeTruthy();
  const button = screen.getByRole("button", { name: "今日签到" });
  fireEvent.click(button); fireEvent.click(button);
  expect(api.signIn).toHaveBeenCalledTimes(1);
  expect(screen.getByLabelText("喵币余额").textContent).toBe("0 喵币");
  await act(async () => { request.resolve({ status: status("0", true), grantedCoins: "0" }); });
  expect(screen.getByText("签到成功，获得 0 喵币。")).toBeTruthy();
  expect((screen.getByRole("button", { name: "今天已签到" }) as HTMLButtonElement).disabled).toBe(true);
});

it("另一窗口已领取时只显示后端现状，不再显示领取奖励", async () => {
  api.signIn.mockResolvedValue({ status: status("7", true), grantedCoins: null });
  await show();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "今日签到" })); });
  expect(screen.getByText("今天已经签到过了。")).toBeTruthy();
  expect(screen.getByLabelText("喵币余额").textContent).toBe("7 喵币");
});

it("签到失败保留原余额并允许再次点击", async () => {
  api.signIn.mockRejectedValueOnce(new Error("写入失败"));
  await show();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "今日签到" })); });
  expect(screen.getByRole("alert").textContent).toBe("写入失败");
  expect(screen.getByLabelText("喵币余额").textContent).toBe("0 喵币");
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "今日签到" })); });
  expect(screen.getByText("签到成功，获得 3 喵币。")).toBeTruthy();
  expect(api.signIn).toHaveBeenCalledTimes(2);
});

it("提交前的迟到余额查询不覆盖提交后的新余额", async () => {
  await show();
  const stale = deferred<SignInStatus>(); api.status.mockReturnValueOnce(stale.promise);
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "今日签到" })); });
  await act(async () => { stale.resolve(status("0")); });
  expect(screen.getByLabelText("喵币余额").textContent).toBe("3 喵币");
  expect(screen.getByRole("button", { name: "今天已签到" })).toBeTruthy();
});

it("切换上下文后丢弃旧会话亲密度响应", async () => {
  const old = deferred<SessionIntimacy>(); api.intimacy.mockReturnValueOnce(old.promise);
  const view = await show();
  api.intimacy.mockResolvedValueOnce(intimacy(53, "3", 2));
  await act(async () => { view.rerender(<PetStatusPanel snapshot={snapshot("3", 2)} />); });
  await act(async () => { old.resolve(intimacy(12)); });
  expect(screen.getByLabelText("当前亲密度").textContent).toBe("53 / 999");
});

it("钱包、配置、互动和聊天通知重新读取，卸载释放监听", async () => {
  const stops = listeners.map(() => vi.fn());
  listeners.forEach((listen, index) => listen.mockResolvedValue(stops[index]));
  const view = await show();
  api.status.mockResolvedValue(status("20")); api.intimacy.mockResolvedValue(intimacy(43));
  await act(async () => {
    api.walletListen.mock.calls[0][0](); api.configListen.mock.calls[0][0]();
    api.petListen.mock.calls[0][0]({ sessionId: "2" });
    api.chatListen.mock.calls[0][0]({ sessionId: "2", type: "finished" });
    api.messagesListen.mock.calls[0][0]({ sessionId: "2" });
  });
  expect(screen.getByLabelText("喵币余额").textContent).toBe("20 喵币");
  expect(screen.getByLabelText("当前亲密度").textContent).toBe("43 / 999");
  view.unmount(); stops.forEach(stop => expect(stop).toHaveBeenCalledTimes(1));
});

it("跨日刷新后端日期和签到状态，不自动领奖，卸载停止定时读取", async () => {
  vi.useFakeTimers(); vi.setSystemTime(new Date(2026, 9, 8, 23, 59, 59, 950));
  api.status.mockResolvedValueOnce(status("3", true)).mockResolvedValue(status("3", false, "2026-10-09"));
  const view = await show();
  await act(async () => { await vi.advanceTimersByTimeAsync(250); });
  expect(screen.getByText("2026-10-09 · 尚未签到")).toBeTruthy();
  expect(screen.getByRole("button", { name: "今日签到" })).toBeTruthy();
  expect(api.signIn).not.toHaveBeenCalled();
  view.unmount(); const calls = api.status.mock.calls.length;
  await vi.advanceTimersByTimeAsync(120_000);
  expect(api.status).toHaveBeenCalledTimes(calls);
});

it("状态首次读取失败时禁用签到，可手动重新读取", async () => {
  api.status.mockRejectedValueOnce(new Error("数据库不可用"));
  await show();
  expect((screen.getByRole("button", { name: "今日签到" }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "重新读取" })); });
  expect((screen.getByRole("button", { name: "今日签到" }) as HTMLButtonElement).disabled).toBe(false);
});

it("节日与会话日期合并展示祝福，签到一次固定十币", async () => {
  const today = { ...status(), festivals: [
    { id: "1", scope: "catalog" as const, name: "清明", blessing: "缅怀故人。", isMemorial: true },
    { id: "9", scope: "session" as const, name: "纪念日", blessing: "珍惜相伴的时光。", isMemorial: false },
  ] };
  api.status.mockResolvedValue(today);
  api.signIn.mockResolvedValue({ status: { ...today, signedIn: true, wallet: { ...today.wallet, coins: "10" } }, grantedCoins: "10" });
  await show();
  expect(screen.getByLabelText("今日日期祝福").textContent).toContain("清明、纪念日");
  expect(screen.getByText("清明：缅怀故人。")).toBeTruthy();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "今日签到" })); });
  expect(screen.getByText("签到成功，获得 10 喵币。")).toBeTruthy();
  expect(screen.getByText("纪念日：珍惜相伴的时光。")).toBeTruthy();
  expect(api.signIn).toHaveBeenCalledTimes(1);
});

it("日期变更通知只刷新状态，不自动签到", async () => {
  await show(); const reads = api.status.mock.calls.length;
  const listener = api.festivalListen.mock.calls[0][0];
  await act(async () => { listener({ sessionId: "2" }); });
  expect(api.status.mock.calls.length).toBe(reads + 1); expect(api.signIn).not.toHaveBeenCalled();
});

it("签到等待期间切换会话，旧会话祝福不能覆盖目标会话", async () => {
  const pending = deferred<DailySignInResult>(); api.signIn.mockReturnValueOnce(pending.promise);
  const view = await show(); fireEvent.click(screen.getByRole("button", { name: "今日签到" }));
  api.status.mockResolvedValue(status("10", true));
  api.intimacy.mockResolvedValue(intimacy(5, "3", 2));
  await act(async () => { view.rerender(<PetStatusPanel snapshot={snapshot("3", 2)} />); });
  await act(async () => { pending.resolve({ status: { ...status("10", true), festivals: [
    { id: "9", scope: "session", name: "旧会话生日", blessing: "旧会话祝福", isMemorial: false },
  ] }, grantedCoins: "10" }); });
  expect(screen.queryByText(/旧会话生日/)).toBeNull();
  expect(screen.getByLabelText("喵币余额").textContent).toBe("10 喵币");
  expect(screen.getByRole("button", { name: "今天已签到" })).toBeTruthy();
});
