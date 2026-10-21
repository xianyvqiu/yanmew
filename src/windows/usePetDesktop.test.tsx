import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { usePetDesktop } from "./usePetDesktop";
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), art: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("../api", () => ({ errorMessage: (e: Error) => e.message, petCreationApi: { listen: mocks.art }, configurationApi: { listenChanges: async () => () => {} } }));
afterEach(cleanup);
beforeEach(() => { vi.resetAllMocks(); mocks.listen.mockResolvedValue(() => {}); mocks.art.mockResolvedValue(() => {}); });
it("恢复保存的停靠与开关，切换提交实际开关值", async () => {
  mocks.invoke.mockImplementation(async (name: string) => name === "get_pet_desktop_state" ? { fullscreenWander: true, dock: "rest_sit", revision: 0 } : name === "get_pet_dock_expression" ? "坐下" : { fullscreenWander: false, dock: "rest_sit", revision: 1 });
  const r = renderHook(() => usePetDesktop("1", 0));
  await waitFor(() => expect(r.result.current.expression).toBe("坐下"));
  await act(async () => { await r.result.current.toggle(); });
  expect(mocks.invoke).toHaveBeenCalledWith("set_pet_fullscreen_wander", { enabled: false });
  expect(r.result.current.fullscreenWander).toBe(false); expect(r.result.current.dock).toBe("rest_sit");
});
it("迟到旧读取不能覆盖已经收到的新状态", async () => {
  let receive: (e: { payload: unknown }) => void = () => {}, complete: (v: unknown) => void = () => {};
  mocks.listen.mockImplementation(async (_: string, fn: typeof receive) => { receive = fn; return () => {}; });
  mocks.invoke.mockImplementation((name: string) => name === "get_pet_desktop_state" ? new Promise(resolve => { complete = resolve; }) : Promise.resolve("探头"));
  const r = renderHook(() => usePetDesktop("1", 0));
  await waitFor(() => expect(mocks.invoke).toHaveBeenCalledWith("get_pet_desktop_state"));
  await act(async () => { receive({ payload: { fullscreenWander: true, dock: "peek_left", revision: 2 } }); complete({ fullscreenWander: false, dock: "none", revision: 1 }); });
  expect(r.result.current.dock).toBe("peek_left"); expect(r.result.current.fullscreenWander).toBe(true);
});
it("切换人设后旧动作结果及旧监听不覆盖新角色", async () => {
  let oldResult: (v: string) => void = () => {};
  const callbacks: Array<() => void> = [];
  mocks.art.mockImplementation(async (fn: () => void) => { callbacks.push(fn); return () => {}; });
  mocks.invoke.mockImplementation((name: string, args?: { personaId?: string }) => name === "get_pet_desktop_state"
    ? Promise.resolve({ fullscreenWander: false, dock: "peek_left", revision: 0 })
    : args?.personaId === "1" ? new Promise(resolve => { oldResult = resolve; }) : Promise.resolve("新角色动作"));
  const r = renderHook(({ id }) => usePetDesktop(id, 0), { initialProps: { id: "1" } });
  await waitFor(() => expect(callbacks.length).toBe(1));
  r.rerender({ id: "2" });
  await waitFor(() => expect(r.result.current.expression).toBe("新角色动作"));
  const calls = mocks.invoke.mock.calls.length;
  await act(async () => { callbacks[0](); oldResult("旧角色动作"); });
  expect(r.result.current.expression).toBe("新角色动作"); expect(mocks.invoke.mock.calls.length).toBe(calls);
});

it("行走按方向读取动作，暂停后迟到的图片不能恢复行走表情", async () => {
  let receive: (e: { payload: unknown }) => void = () => {}, complete: (value: string) => void = () => {};
  mocks.listen.mockImplementation(async (_: string, fn: typeof receive) => { receive = fn; return () => {}; });
  mocks.invoke.mockImplementation((name: string) => name === "get_pet_desktop_state"
    ? Promise.resolve({ fullscreenWander: true, dock: "none", revision: 1, walking: null })
    : new Promise<string>(resolve => { complete = resolve; }));
  const r = renderHook(() => usePetDesktop("1", 0));
  await waitFor(() => expect(r.result.current.revision).toBe(1));
  await act(async () => { receive({ payload: { fullscreenWander: true, dock: "none", revision: 2, walking: { direction: "left", personaId: "1", sessionId: "1", contextRevision: 0 } } }); });
  expect(mocks.invoke).toHaveBeenCalledWith("get_pet_walk_expression", { personaId: "1", direction: "left" });
  await act(async () => { receive({ payload: { fullscreenWander: true, dock: "none", revision: 3, walking: null } }); });
  await act(async () => { complete("旧行走图"); });
  expect(r.result.current.expression).toBeUndefined(); expect(r.result.current.walking).toBeNull();
});
