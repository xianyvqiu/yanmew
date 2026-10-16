import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import PetCreationPanel from "./PetCreationPanel";
import type { PetArtwork, PetCreation } from "../../types";
const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn(), enable: vi.fn(), image: vi.fn(), upload: vi.fn(), finish: vi.fn(), generate: vi.fn(), cancel: vi.fn(), listen: vi.fn() }));
vi.mock("../../api", () => ({ petCreationApi: api, errorMessage: (e: Error) => e.message }));
const base: PetArtwork = { id: "1", personaId: "2", kind: "base", name: "初始形象", prompt: "", description: "默认", category: "normal", enabled: true, imageFile: "base.png", previewFile: null, revision: 0, createdAt: "2026-10-16T00:00:00Z", updatedAt: "2026-10-16T00:00:00Z" };
const expression: PetArtwork = { ...base, id: "2", kind: "expression", name: "挥手", prompt: "开心挥手", description: "欢迎时", category: "chat", imageFile: "formal.png", previewFile: "preview.png" };
let library: PetCreation;
beforeEach(() => { vi.resetAllMocks(); library = { enabled: false, revision: 3, items: [base, expression] }; api.get.mockImplementation(async () => library); api.listen.mockResolvedValue(() => { }); api.image.mockResolvedValue("data:image/png;base64,AQ=="); api.finish.mockResolvedValue(undefined); api.enable.mockResolvedValue(undefined); api.cancel.mockResolvedValue(undefined); });
afterEach(cleanup);
it("只有初始图或未接受的待机预览不能开启，并提供待机分类入口", async () => {
  render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByRole("button", { name: "添加待机表情" });
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
  expect(screen.getByText(/尚缺可用待机图/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "添加待机表情" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith("2", null, null, expect.objectContaining({ category: "standby", enabled: true, kind: "expression" })));
});
it("待机图尚未接受或已停用时不能开启", async () => {
  library.items.push({ ...expression, id: "3", category: "standby", imageFile: null, previewFile: "standby-preview.png", name: "待机预览" });
  const first = render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByRole("heading", { name: "待机预览" });
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
  first.unmount();
  library.items[2] = { ...library.items[2], imageFile: "standby.png", enabled: false };
  render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByRole("heading", { name: "待机预览" });
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
});
it("旧版图片模式缺待机图时显示补图提示，仍允许主动关闭", async () => {
  library.enabled = true;
  render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByText(/旧版已开启的图片模式保留/);
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("switch"));
  await waitFor(() => expect(api.enable).toHaveBeenCalledWith("2", false, 3));
});
it("按当前人设启用，且接受预览是独立操作", async () => { library.items.push({ ...expression, id: "3", name: "待机图", category: "standby", previewFile: null }); render(<PetCreationPanel personaId="2" personaName="猫猫"/>); await screen.findByText("挥手"); fireEvent.click(screen.getByRole("switch")); await waitFor(() => expect(api.enable).toHaveBeenCalledWith("2", true, 3)); fireEvent.click(screen.getByRole("button", { name: "接受预览" })); await waitFor(() => expect(api.finish).toHaveBeenCalledWith("2", "2", 0, "accept")); expect(api.generate).not.toHaveBeenCalled(); });
it("没有已接受的初始形象时不能开启图片模式", async () => { library = { ...library, items: [] }; render(<PetCreationPanel personaId="2" personaName="猫猫"/>); await screen.findByRole("button", { name: "添加初始形象" }); expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true); });
it("多行要求逐条建库后选中，批量生成每项分别处理", async () => { let next = 3; api.save.mockImplementation(async (_p, _id, _rev, input) => { const value = { ...expression, ...input, id: String(next++), imageFile: null, previewFile: null }; library = { ...library, items: [...library.items, value] }; return value; }); api.generate.mockResolvedValue(expression); render(<PetCreationPanel personaId="2" personaName="猫猫"/>); await screen.findByText("挥手"); fireEvent.change(screen.getByLabelText("每行一个要求，可一次添加多个"), { target: { value: "抱枕睡觉\n吃小鱼干" } }); fireEvent.click(screen.getByRole("button", { name: "添加要求" })); await screen.findByRole("heading", { name: "吃小鱼干" }); expect(api.save).toHaveBeenCalledTimes(2); fireEvent.click(screen.getByRole("button", { name: "生成选中表情 · 2 项" })); await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(2)); expect(api.finish).not.toHaveBeenCalled(); });
it("重新生成失败仍保留旧预览和已接受图片", async () => { api.generate.mockRejectedValue(new Error("服务不支持参考图编辑")); render(<PetCreationPanel personaId="2" personaName="猫猫"/>); await screen.findByText("挥手"); fireEvent.click(screen.getByRole("button", { name: "重新生成" })); await screen.findByText("服务不支持参考图编辑"); expect(screen.getByRole("button", { name: "接受预览" })).toBeTruthy(); expect(library.items[1].imageFile).toBe("formal.png"); expect(library.items[1].previewFile).toBe("preview.png"); expect(api.finish).not.toHaveBeenCalled(); });

it("批量新建中途失败会展示已保存项，仅保留尚未保存的要求", async () => {
  let calls = 0;
  api.save.mockImplementation(async (_persona, _id, _revision, input) => {
    calls++;
    if (calls === 2) throw new Error("保存第二条失败");
    const item = { ...expression, ...input, id: String(calls + 2), imageFile: null, previewFile: null };
    library = { ...library, revision: library.revision + 1, items: [...library.items, item] };
    return item;
  });
  render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByText("挥手");
  fireEvent.change(screen.getByLabelText("每行一个要求，可一次添加多个"), { target: { value: "要求一\n要求二" } });
  fireEvent.click(screen.getByRole("button", { name: "添加要求" }));
  await screen.findByText("保存第二条失败");
  await screen.findByRole("heading", { name: "要求一" });
  expect((screen.getByLabelText("每行一个要求，可一次添加多个") as HTMLTextAreaElement).value).toBe("要求二");
  fireEvent.click(screen.getByRole("button", { name: "添加要求" }));
  await screen.findByRole("heading", { name: "要求二" });
  expect(api.save.mock.calls.map(call => call[3].prompt)).toEqual(["要求一", "要求二", "要求二"]);
  expect(library.items.filter(item => item.prompt === "要求一")).toHaveLength(1);
});
