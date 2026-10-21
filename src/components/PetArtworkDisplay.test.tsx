import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import PetArtworkDisplay from "./PetArtworkDisplay";
import type { PetArtwork, PetCreation } from "../types";
const api = vi.hoisted(() => ({ get: vi.fn(), image: vi.fn(), listen: vi.fn() }));
vi.mock("../api", () => ({ petCreationApi: api }));
const base: PetArtwork = { id: "1", personaId: "2", kind: "base", name: "初始猫", prompt: "", description: "默认", category: "normal", enabled: true, imageFile: "base.png", previewFile: null, revision: 0, createdAt: "2026-10-16", updatedAt: "2026-10-16" };
const expression: PetArtwork = { ...base, id: "2", kind: "expression", name: "挥手猫", imageFile: "wave.png" };
const library: PetCreation = { enabled: true, revision: 1, items: [base, expression] };
let refresh: () => void;
beforeEach(() => { vi.resetAllMocks(); api.get.mockResolvedValue(library); api.image.mockImplementation(async (_persona, id) => `data:image/png;base64,${id}`); api.listen.mockImplementation(async callback => { refresh = callback; return () => {}; }); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });
it("当前图和初始图都缺失时各尝试一次，停止重试并显示兜底", async () => {
  api.image.mockRejectedValue(new Error("图片已丢失"));
  render(<PetArtworkDisplay personaId="2" expression="@pet-image:2" />);
  await waitFor(() => expect(api.image).toHaveBeenCalledTimes(2));
  await act(async () => {});
  expect(api.image.mock.calls.map(call => call[1])).toEqual(["2", "1"]);
  expect(screen.queryByRole("img")).toBeNull(); expect(screen.getByText("(・ω・)")).toBeTruthy();
});
it("浏览器无法显示两张图时也不在它们之间循环", async () => {
  render(<PetArtworkDisplay personaId="2" expression="@pet-image:2" />);
  fireEvent.error(await screen.findByRole("img", { name: "挥手猫" }));
  fireEvent.error(await screen.findByRole("img", { name: "初始猫" }));
  await waitFor(() => expect(screen.queryByRole("img")).toBeNull());
  expect(api.image).toHaveBeenCalledTimes(2);
});
it("较早的开启状态迟到时不能覆盖最新关闭状态", async () => {
  let resolveOld!: (value: PetCreation) => void;
  api.get.mockImplementationOnce(() => new Promise<PetCreation>(resolve => { resolveOld = resolve; }));
  api.get.mockResolvedValue({ ...library, enabled: false, revision: 2 });
  render(<PetArtworkDisplay personaId="2" expression="@pet-image:2" />);
  await waitFor(() => expect(api.get).toHaveBeenCalledTimes(1));
  await act(async () => { refresh(); });
  await act(async () => { resolveOld(library); });
  expect(screen.queryByRole("img")).toBeNull(); expect(api.image).not.toHaveBeenCalled();
});

it("底部停靠按原始透明边界计算，不随显示尺寸产生反馈偏移", async () => {
  const pixels = new Uint8ClampedArray(512 * 256 * 4);
  for (let y = 64; y < 192; y++) pixels[(y * 512 + 100) * 4 + 3] = 255;
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({ drawImage() {}, getImageData: () => ({ data: pixels }) } as unknown as CanvasRenderingContext2D);
  const r = render(<PetArtworkDisplay personaId="2" expression="@pet-image:2" dock="rest_lie" />);
  const image = await screen.findByRole("img", { name: "挥手猫" });
  Object.defineProperties(image, { naturalWidth: { value: 512 }, naturalHeight: { value: 256 }, complete: { value: true }, clientWidth: { value: 128, configurable: true } });
  fireEvent.load(image);
  const style = image.parentElement!.style;
  expect(style.getPropertyValue("--ym-art-top-ratio")).toBe("-0.125");
  expect(style.getPropertyValue("--ym-art-height-ratio")).toBe("0.5");
  expect(style.getPropertyValue("--ym-art-visible-ratio")).toBe("0.25");
  Object.defineProperty(image, "clientWidth", { value: 64 });
  fireEvent.load(image);
  expect(style.getPropertyValue("--ym-art-visible-ratio")).toBe("0.25");
  r.rerender(<PetArtworkDisplay personaId="2" expression="@pet-image:2" dock="none" />);
  expect(style.getPropertyValue("--ym-art-visible-ratio")).toBe("1");
});
it("图片尚未读取时保留图片占位，并通知窗口切换透明画布", async () => {
  api.image.mockImplementation(() => new Promise(() => {}));
  const mode = vi.fn();
  const r = render(<PetArtworkDisplay personaId="2" expression="@pet-image:2" onImageMode={mode} />);
  expect(r.container.firstElementChild?.classList.contains("pet-expression-image")).toBe(true);
  await waitFor(() => expect(mode).toHaveBeenCalledWith(true));
  api.get.mockResolvedValue({...library,enabled:false,revision:2});
  await act(async () => refresh());
  await waitFor(() => expect(mode).toHaveBeenLastCalledWith(false));
});

it("切图读取期间保留旧图，新图成功才替换，不显示颜文字闪帧",async()=>{
 let finish!:(url:string)=>void;api.image.mockImplementation((_persona,id)=>id==="2"?new Promise<string>(r=>{finish=r;}):Promise.resolve("data:image/png;base64,base"));
 const view=render(<PetArtworkDisplay personaId="2" expression="@pet-image:1"/>);await screen.findByRole("img",{name:"初始猫"});
 view.rerender(<PetArtworkDisplay personaId="2" expression="@pet-image:2"/>);await waitFor(()=>expect(finish).toBeTypeOf("function"));
 expect(screen.getByRole("img",{name:"初始猫"})).toBeTruthy();expect(screen.queryByText("(・ω・)")).toBeNull();
 await act(async()=>finish("data:image/png;base64,wave"));await screen.findByRole("img",{name:"挥手猫"});
});
