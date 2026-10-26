import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ChatBackgroundView, Persona } from "../types";
import ChatBackgroundSettings from "./ChatBackgroundSettings";
import ChatBackground from "./ChatBackground";
import { DEFAULT_CHAT_APPEARANCE } from "./chatAppearance";

vi.mock("./BackgroundCropDialog",()=>({default:({onSave,onCancel}:{onSave:(base64:string)=>void;onCancel:()=>void})=><div role="dialog" aria-label="裁剪聊天背景"><button onClick={()=>onSave("cG5n")}>保存裁剪</button><button onClick={onCancel}>取消裁剪</button></div>}));
const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn() }));
vi.mock("../api", () => ({ backgroundApi: api, errorMessage: (cause: { message: string }) => cause.message }));
const persona: Persona = {
  id: "1", name: "小喵", prompt: "设定", initialPrompt: "设定", selfEvolveEnabled: false,
  lastEvolvedAt: null, color: "#888888", chatBgFile: null, chatBgOpacity: 85, chatBgBlur: 0,
  isActive: true, isBuiltin: true, expressions: null,
};
const record = { personaId: "1", file: null, opacity: 85, blur: 0 };
const view: ChatBackgroundView = { record, imageDataUrl: null, imageError: null };
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
beforeEach(() => { api.get.mockReset().mockResolvedValue(view); api.save.mockReset().mockImplementation(async (_id, input) => ({ ...input.expected, opacity: input.opacity, blur: input.blur, appearance: input.appearance ?? input.expected.appearance })); });
afterEach(cleanup);
async function open() {
  fireEvent.click(screen.getByRole("button", { name: "聊天外观" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "保存外观" }).closest("fieldset")?.disabled).toBe(false));
}

it("气泡和双方文字颜色只预览，保存后可重读，恢复样式不清背景", async () => {
  const saved = vi.fn();
  render(<ChatBackgroundSettings persona={persona} onSaved={saved} />);
  await open();
  fireEvent.change(screen.getByLabelText("聊天气泡样式"), { target: { value: "frosted" } });
  fireEvent.change(screen.getByLabelText("用户文字颜色"), { target: { value: "#123456" } });
  fireEvent.change(screen.getByLabelText("AI文字颜色"), { target: { value: "#abcdef" } });
  const preview = screen.getByLabelText("聊天背景预览");
  expect(preview.dataset.bubbleStyle).toBe("frosted");
  expect(preview.style.getPropertyValue("--ym-chat-user-text")).toBe("#123456");
  expect(api.save).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存外观" }));
  await screen.findByText("聊天外观已保存");
  expect(api.save).toHaveBeenCalledWith("1", expect.objectContaining({ appearance: { bubbleStyle: "frosted", userTextColor: "#123456", assistantTextColor: "#abcdef" } }), undefined);
  fireEvent.click(screen.getByRole("button", { name: "恢复默认样式" }));
  expect(preview.dataset.bubbleStyle).toBe("solid");
  expect(preview.style.getPropertyValue("--ym-chat-user-text")).toBe("var(--ym-text)");
  expect(api.save).toHaveBeenCalledOnce();
});

it("恢复默认背景保留未保存的外观草稿，错误重读则恢复权威样式", async () => {
  api.save.mockResolvedValueOnce(record).mockRejectedValueOnce({ code: "CONFLICT", message: "外观已修改" });
  render(<ChatBackgroundSettings persona={persona} onSaved={() => {}} />);
  await open();
  fireEvent.change(screen.getByLabelText("聊天气泡样式"), { target: { value: "outline" } });
  fireEvent.click(screen.getByRole("button", { name: "恢复默认背景" }));
  await screen.findByText("已恢复默认背景；气泡和颜色的预览修改尚未保存");
  expect(screen.getByLabelText("聊天气泡样式")).toHaveProperty("value", "outline");
  expect(api.save.mock.calls[0][1].appearance).toBeUndefined();
  fireEvent.click(screen.getByRole("button", { name: "保存外观" }));
  await screen.findByText("外观已修改");
  api.get.mockResolvedValue({ ...view, record: { ...record, appearance: { ...DEFAULT_CHAT_APPEARANCE, bubbleStyle: "transparent" } } });
  fireEvent.click(screen.getByRole("button", { name: "放弃修改并重新加载" }));
  await waitFor(() => expect(screen.getByLabelText("聊天气泡样式")).toHaveProperty("value", "transparent"));
});

it("折叠时不读图片，滑块只预览，保存附带人设和完整快照", async () => {
  const saved = vi.fn();
  render(<ChatBackgroundSettings persona={persona} contextRevision={7} onSaved={saved} />);
  expect(api.get).not.toHaveBeenCalled();
  await open();
  expect(api.get).toHaveBeenCalledWith("1", 7);
  expect(screen.queryByRole("button", { name: "放弃修改并重新加载" })).toBeNull();
  fireEvent.change(screen.getByLabelText("背景透明度：85%"), { target: { value: "60" } });
  fireEvent.change(screen.getByLabelText("背景模糊度：0px"), { target: { value: "8" } });
  expect(api.save).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存外观" }));
  await waitFor(() => expect(saved).toHaveBeenCalledOnce());
  expect(api.save).toHaveBeenCalledWith("1", { expected: record, opacity: 60, blur: 8, image: null, reset: false, appearance: DEFAULT_CHAT_APPEARANCE }, 7);
});

it("读取文件后显示预览，保存失败保留图片和滑块，不通知宿主", async () => {
  api.save.mockRejectedValue({ code: "VALIDATION", message: "图片损坏" });
  const saved = vi.fn();
  render(<ChatBackgroundSettings persona={persona} onSaved={saved} />);
  await open();
  fireEvent.change(screen.getByLabelText("选择背景图片"), { target: { files: [new File(["png"], "cat.png", { type: "image/png" })] } });
  fireEvent.click(await screen.findByRole("button",{name:"保存裁剪"}));
  await waitFor(() => expect(screen.getByLabelText("聊天背景预览").querySelector("img")?.src).toBe("data:image/png;base64,cG5n"));
  fireEvent.change(screen.getByLabelText("背景模糊度：0px"), { target: { value: "9" } });
  fireEvent.click(screen.getByRole("button", { name: "保存外观" }));
  await screen.findByText("图片损坏");
  expect(api.save).toHaveBeenCalledWith("1", { expected: record, opacity: 85, blur: 9, image: { fileName: "chat-background.png", base64: "cG5n" }, reset: false, appearance: DEFAULT_CHAT_APPEARANCE }, undefined);
  expect(screen.getByLabelText("背景模糊度：9px")).toHaveProperty("value", "9");
  expect(screen.getByLabelText("聊天背景预览").querySelector("img")).not.toBeNull();
  expect(saved).not.toHaveBeenCalled();
});

it("拒绝不支持格式和超过 10MiB 的文件，不调用保存", async () => {
  render(<ChatBackgroundSettings persona={persona} onSaved={() => {}} />);
  await open();
  const input = screen.getByLabelText("选择背景图片");
  fireEvent.change(input, { target: { files: [new File(["svg"], "cat.svg")] } });
  await screen.findByText("请选择不超过 10MB 的 JPG、PNG 或 WebP 图片");
  const large = new File(["png"], "cat.png");
  Object.defineProperty(large, "size", { value: 10 * 1024 * 1024 + 1 });
  fireEvent.change(input, { target: { files: [large] } });
  expect(screen.getByLabelText("聊天背景预览").querySelector("img")).toBeNull();
  expect(api.save).not.toHaveBeenCalled();
});

it("冲突不覆盖，明确重新读取后才允许保存", async () => {
  api.save.mockRejectedValue({ code: "CONFLICT", message: "背景已修改" });
  render(<ChatBackgroundSettings persona={persona} onSaved={() => {}} />);
  await open();
  fireEvent.change(screen.getByLabelText("背景透明度：85%"), { target: { value: "33" } });
  fireEvent.click(screen.getByRole("button", { name: "保存外观" }));
  await screen.findByText("背景已修改");
  expect(screen.getByRole("button", { name: "保存外观" })).toHaveProperty("disabled", true);
  expect(screen.getByLabelText("背景透明度：33%")) .toHaveProperty("value", "33");
  fireEvent.click(screen.getByRole("button", { name: "放弃修改并重新加载" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "保存外观" })).toHaveProperty("disabled", false));
  expect(screen.getByLabelText("背景透明度：85%")) .toHaveProperty("value", "85");
});

it("其他窗口的修改不静默覆盖未保存草稿", async () => {
  const result = render(<ChatBackgroundSettings persona={persona} onSaved={() => {}} />);
  await open();
  fireEvent.change(screen.getByLabelText("背景透明度：85%"), { target: { value: "33" } });
  result.rerender(<ChatBackgroundSettings persona={{ ...persona, chatBgOpacity: 70 }} onSaved={() => {}} />);
  await screen.findByText("聊天外观已在其他窗口更新。重新读取会丢弃本页未保存的外观修改。");
  expect(api.get).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "保存外观" })).toHaveProperty("disabled", true);
});

it("恢复默认使用独立操作，清空预览并回到 85/0", async () => {
  api.get.mockResolvedValue({ record: { ...record, file: "cat.png", opacity: 60, blur: 10 }, imageDataUrl: "data:image/png;base64,cG5n", imageError: null });
  api.save.mockResolvedValue(record);
  render(<ChatBackgroundSettings persona={{ ...persona, chatBgFile: "cat.png", chatBgOpacity: 60, chatBgBlur: 10 }} onSaved={() => {}} />);
  await open();
  fireEvent.click(screen.getByRole("button", { name: "恢复默认背景" }));
  await screen.findByText("已恢复默认背景");
  expect(api.save.mock.calls[0][1]).toMatchObject({ reset: true, image: null });
  expect(screen.getByLabelText("聊天背景预览").querySelector("img")).toBeNull();
  expect(screen.getByLabelText("背景透明度：85%")) .toHaveProperty("value", "85");
});

it("聊天无图片时不读取，切换人设后丢弃旧图片的迟到结果", async () => {
  const late = deferred<ChatBackgroundView>();
  const result = render(<ChatBackground persona={persona} contextRevision={1} />);
  expect(api.get).not.toHaveBeenCalled();
  api.get.mockReturnValueOnce(late.promise).mockResolvedValueOnce({ record: { ...record, personaId: "2", file: "new.png" }, imageDataUrl: "data:image/png;base64,bmV3", imageError: null });
  result.rerender(<ChatBackground persona={{ ...persona, chatBgFile: "old.png" }} contextRevision={1} />);
  result.rerender(<ChatBackground persona={{ ...persona, id: "2", chatBgFile: "new.png", chatBgOpacity: 60, chatBgBlur: 8 }} contextRevision={2} />);
  await waitFor(() => expect(result.container.querySelector("img")?.src).toBe("data:image/png;base64,bmV3"));
  await act(async () => late.resolve({ record: { ...record, file: "old.png" }, imageDataUrl: "data:image/png;base64,b2xk", imageError: null }));
  expect(result.container.querySelector("img")?.src).toBe("data:image/png;base64,bmV3");
  const style = result.container.querySelector<HTMLElement>(".ym-background-image")!.style;
  expect(style.getPropertyValue("--ym-background-opacity")).toBe("0.6");
  expect(style.getPropertyValue("--ym-background-blur")).toBe("8px");
});

it("丢失图片显示可重试局部错误，成功后重新显示背景", async () => {
  const configured = { ...persona, chatBgFile: "cat.png" };
  api.get.mockResolvedValueOnce({ record: { ...record, file: "cat.png" }, imageDataUrl: null, imageError: { code: "BACKGROUND_MISSING", message: "图片不存在", retryable: false } })
    .mockResolvedValueOnce({ record: { ...record, file: "cat.png" }, imageDataUrl: "data:image/png;base64,cG5n", imageError: null });
  const result = render(<ChatBackground persona={configured} contextRevision={4} />);
  await screen.findByText("图片不存在，聊天仍可使用。");
  expect(result.container.querySelector("img")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "重试读取背景" }));
  await waitFor(() => expect(result.container.querySelector("img")).not.toBeNull());
  expect(screen.queryByRole("alert")).toBeNull();
});
