import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { usePetAppearance } from "./usePetAppearance";

const settings = vi.fn();
const personas = vi.fn();
const callbacks = new Map<string, () => void>();
let systemChange: () => void;
let dark = false;
vi.mock("../api", () => ({ configurationApi: { getSettings: () => settings(), listPersonas: () => personas() }, errorMessage: () => "读取失败" }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async (name: string, callback: () => void) => { callbacks.set(name, callback); return () => callbacks.delete(name); } }));
beforeEach(() => {
  callbacks.clear(); dark = false;
  settings.mockReset().mockResolvedValue({ theme_mode: "system" });
  personas.mockReset().mockResolvedValue([{ isActive: true, color: "#ffb6c1" }]);
  vi.stubGlobal("matchMedia", () => ({ get matches() { return dark; }, addEventListener: (_: string, callback: () => void) => { systemChange = callback; }, removeEventListener() {} }));
});
it("跟随系统并读取当前人设颜色，固定亮色不跟随系统改变", async () => {
  const hook = renderHook(usePetAppearance);
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  expect(hook.result.current.style).toEqual({ "--ym-pet-color": "#ffb6c1" });
  act(() => { dark = true; systemChange(); });
  expect(document.documentElement.dataset.theme).toBe("dark");
  settings.mockResolvedValue({ theme_mode: "light" });
  await act(async () => { callbacks.get("configuration_changed")?.(); });
  expect(document.documentElement.dataset.theme).toBe("light");
  act(() => { systemChange(); });
  expect(document.documentElement.dataset.theme).toBe("light");
  hook.unmount(); expect(callbacks.size).toBe(0);
});
it("配置失败仍结束加载并显示错误", async () => {
  settings.mockRejectedValue(new Error("storage"));
  const hook = renderHook(usePetAppearance);
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  expect(hook.result.current.error).toBe("读取失败");
  hook.unmount();
});
