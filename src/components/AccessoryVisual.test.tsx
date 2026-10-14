import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AccessoryImage, { AccessoryVisual } from "./AccessoryVisual";
import type { Accessory } from "../types";
const api = vi.hoisted(() => ({ image: vi.fn() }));
vi.mock("../api", () => ({ accessoriesApi: api, errorMessage: (cause: Error) => cause.message }));
const context = { personaId: "1", sessionId: "2", contextRevision: 1 };
const accessory: Accessory = { id: "3", sessionId: "2", name: "小花", displayText: "✿", imageFile: "assets/accessories/flower.png", description: "花", cost: "20", source: "crafted", travelId: null, isWorn: true, createdAt: "now" };
beforeEach(() => { api.image.mockReset(); vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} }); });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
it("浏览器图片失败立即换成真实字符外观，不显示破图", () => {
  render(<AccessoryVisual name="小花" displayText="✿" dataUrl="data:image/png;base64,AA==" />);
  fireEvent.error(screen.getByRole("img", { name: "小花" }));
  expect(screen.getByRole("img", { name: "小花" }).tagName).toBe("SPAN");
  expect(screen.getByText("✿")).toBeTruthy();
  expect(screen.getByText(/图片无法显示/)).toBeTruthy();
});
it("后台缺图保留饰品，缩略图显示局部说明和字符外观", async () => {
  api.image.mockResolvedValue({ dataUrl: null, notice: "图片文件不存在，使用原图标。" });
  render(<AccessoryImage context={context} accessory={accessory} />);
  await screen.findByText("图片文件不存在，使用原图标。");
  expect(screen.getByText("✿")).toBeTruthy();
  expect(api.image).toHaveBeenCalledWith(context, "3");
});
it("切换上下文丢弃迟到图片，列表刷新会重新读取原文件", async () => {
  let finish!: (image: { dataUrl: string; notice: null }) => void;
  api.image.mockReturnValueOnce(new Promise(resolve => { finish = resolve; })).mockResolvedValue({ dataUrl: null, notice: "当前图片缺失" });
  const view = render(<AccessoryImage context={context} accessory={accessory} />);
  view.rerender(<AccessoryImage context={{ ...context, contextRevision: 2 }} accessory={accessory} />);
  await act(async () => { finish({ dataUrl: "data:image/png;base64,OLD", notice: null }); });
  expect(screen.queryByRole("img", { name: "小花" })?.tagName).toBe("SPAN");
  view.rerender(<AccessoryImage context={{ ...context, contextRevision: 2 }} accessory={{ ...accessory }} />);
  await screen.findByText("当前图片缺失");
  expect(api.image).toHaveBeenCalledTimes(3);
});
