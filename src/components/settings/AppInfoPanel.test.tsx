import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import AppInfoPanel from "./AppInfoPanel";
const api = vi.hoisted(() => ({ info: vi.fn(), exit: vi.fn() }));
vi.mock("../../api", () => ({ appApi: api, errorMessage: (error: Error) => error.message }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
it("展示后端实际版本，退出请求连点只发送一次", async () => {
  api.info.mockResolvedValue({name:"颜喵 YanMew",version:"0.1.0"}); api.exit.mockReturnValue(new Promise(() => {}));
  await act(async () => { render(<AppInfoPanel />); });
  expect(screen.getByText("颜喵 YanMew · 版本 0.1.0")).toBeTruthy();
  const exit = screen.getByRole("button",{name:"退出应用"}); fireEvent.click(exit); fireEvent.click(exit);
  expect(api.exit).toHaveBeenCalledTimes(1); expect(screen.getByRole("button",{name:"正在退出…"})).toBeTruthy();
});
it("版本读取失败可重试，退出失败仍允许再次操作", async () => {
  api.info.mockRejectedValueOnce(new Error("信息读取失败")).mockResolvedValue({name:"颜喵 YanMew",version:"0.2.0"});
  api.exit.mockRejectedValueOnce(new Error("退出请求失败")).mockResolvedValue(undefined);
  await act(async () => { render(<AppInfoPanel />); });
  await act(async () => { fireEvent.click(screen.getByRole("button",{name:"重新读取版本"})); });
  expect(screen.getByText("颜喵 YanMew · 版本 0.2.0")).toBeTruthy();
  await act(async () => { fireEvent.click(screen.getByRole("button",{name:"退出应用"})); });
  expect(screen.getByText("退出请求失败")).toBeTruthy();
  await act(async () => { fireEvent.click(screen.getByRole("button",{name:"退出应用"})); });
  expect(api.exit).toHaveBeenCalledTimes(2);
});
