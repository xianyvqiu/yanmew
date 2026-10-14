import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useCurrentContext } from "./useCurrentContext";
import type { CurrentContext } from "../types";

const api = vi.hoisted(() => ({ get: vi.fn(), personas: vi.fn(), sessions: vi.fn(), profiles: vi.fn(), listen: vi.fn(), changes: vi.fn() }));
vi.mock("../api", () => ({ configurationApi: { getCurrentContext: api.get, listPersonas: api.personas, listenContext: api.listen, listenChanges: api.changes }, sessionApi: { list: api.sessions }, userProfileApi: { list: api.profiles }, errorMessage: () => "读取失败" }));
let receive: (context: CurrentContext) => void;
let current: CurrentContext;
const stop = vi.fn();
beforeEach(() => {
  vi.clearAllMocks(); current = { personaId: "1", sessionId: "1", contextRevision: 0 };
  api.get.mockImplementation(async () => ({ ...current }));
  api.personas.mockResolvedValue([{ id: "1", name: "喵喵" }]);
  api.sessions.mockResolvedValue([]);
  api.profiles.mockResolvedValue([{ id: "7", name: "测试资料", content: "资料正文" }]);
  api.listen.mockImplementation(async callback => { receive = callback; return stop; });
  api.changes.mockResolvedValue(stop);
});
afterEach(cleanup);
it("同一上下文聚焦刷新期间保留页面快照", async () => {
  const hook = renderHook(useCurrentContext);
  await waitFor(() => expect(hook.result.current.snapshot).not.toBeNull());
  const previous = hook.result.current.snapshot;
  let finish!: (value: CurrentContext) => void;
  api.get.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  act(() => window.dispatchEvent(new Event("focus")));
  expect(hook.result.current.snapshot).toBe(previous);
  await act(async () => finish(current));
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
});
it("先订阅再读取，聚焦补查且卸载释放监听", async () => {
  const hook = renderHook(useCurrentContext);
  await waitFor(() => expect(hook.result.current.snapshot?.context.contextRevision).toBe(0));
  expect(hook.result.current.snapshot?.userProfiles).toEqual([{ id: "7", name: "测试资料", content: "资料正文" }]);
  expect(api.listen.mock.invocationCallOrder[0]).toBeLessThan(api.get.mock.invocationCallOrder[0]);
  current = { ...current, contextRevision: 1, sessionId: "2" };
  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(hook.result.current.snapshot?.context.sessionId).toBe("2"));
  hook.unmount(); expect(stop).toHaveBeenCalledTimes(2);
});
it("新上下文优先，旧会话迟到结果不能覆盖", async () => {
  let resolveOld!: (value: unknown[]) => void;
  api.sessions.mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }));
  const hook = renderHook(useCurrentContext);
  await waitFor(() => expect(resolveOld).toBeDefined());
  current = { ...current, sessionId: "2", contextRevision: 2 };
  act(() => receive(current));
  await waitFor(() => expect(hook.result.current.snapshot?.context.sessionId).toBe("2"));
  await act(async () => resolveOld([{ id: "1", title: "旧会话" }]));
  expect(hook.result.current.snapshot?.context.sessionId).toBe("2");
  expect(hook.result.current.snapshot?.sessions).toEqual([]);
  act(() => receive({ ...current, sessionId: "1", contextRevision: 1 }));
  expect(hook.result.current.snapshot?.context.sessionId).toBe("2");
});
it("监听失败可重试建立订阅，StrictMode 式卸载不泄漏", async () => {
  api.listen.mockRejectedValueOnce(new Error("listen failed"));
  const hook = renderHook(useCurrentContext);
  await waitFor(() => expect(hook.result.current.error).toBe("读取失败"));
  act(() => hook.result.current.refresh());
  await waitFor(() => expect(hook.result.current.snapshot?.persona.name).toBe("喵喵"));
  hook.unmount(); expect(stop).toHaveBeenCalledTimes(2);
});
