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
afterEach(cleanup);
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
