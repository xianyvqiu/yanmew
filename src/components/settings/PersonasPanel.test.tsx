import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Persona, PersonaInput } from "../../types";
import PersonasPanel from "./PersonasPanel";

const api = vi.hoisted(() => ({
  listPersonas: vi.fn(),
  listenChanges: vi.fn(),
  listenContext: vi.fn(),
  listenExpressionFailure: vi.fn(),
  createPersona: vi.fn(),
  updatePersona: vi.fn(),
  deletePersona: vi.fn(),
  setPersona: vi.fn(),
  generateExpressions: vi.fn(),
  cancelExpressionGeneration: vi.fn(),
}));
vi.mock("../../api", () => ({ configurationApi: api, errorMessage: (error: { message: string }) => error.message }));

const systemPersona: Persona = {
  id: "system-1",
  name: "喵喵桌宠",
  prompt: "默认陪伴",
  initialPrompt: "默认陪伴",
  selfEvolveEnabled: true,
  lastEvolvedAt: null,
  color: "#777777",
  chatBgFile: null,
  chatBgOpacity: 85,
  chatBgBlur: 0,
  isActive: false,
  isBuiltin: true,
  expressions: null,
};
const currentPersona: Persona = {
  ...systemPersona,
  id: "persona-1",
  name: "小喵",
  prompt: "温柔地陪伴用户",
  initialPrompt: "最初的设定",
  color: "#dd88aa",
  chatBgFile: "background.webp",
  chatBgOpacity: 62,
  chatBgBlur: 12,
  isActive: true,
  isBuiltin: false,
  expressions: { click: { happy: ["开心旧表情"] }, hungry: ["饿旧表情"] },
};
const otherPersona: Persona = { ...systemPersona, id: "persona-2", name: "另一只猫" };

let rows: Persona[];
let expressionFailure: ((event: { personaId: string; error: { message: string } }) => void) | undefined;

beforeEach(() => {
  vi.clearAllMocks();
  rows = [systemPersona, currentPersona, otherPersona];
  expressionFailure = undefined;
  api.listPersonas.mockImplementation(async () => [...rows]);
  api.listenChanges.mockResolvedValue(() => {});
  api.listenContext.mockResolvedValue(() => {});
  api.listenExpressionFailure.mockImplementation(async (callback: typeof expressionFailure) => {
    expressionFailure = callback;
    return () => {};
  });
  api.createPersona.mockResolvedValue(currentPersona);
  api.updatePersona.mockResolvedValue(currentPersona);
  api.deletePersona.mockResolvedValue(undefined);
  api.setPersona.mockResolvedValue(undefined);
  api.generateExpressions.mockResolvedValue(undefined);
  api.cancelExpressionGeneration.mockResolvedValue(undefined);
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});

afterEach(cleanup);

async function loadPanel() {
  render(<PersonasPanel />);
  await screen.findByRole("article", { name: currentPersona.name });
}

describe("PersonasPanel", () => {
  it("系统内置人设不显示删除按钮", async () => {
    await loadPanel();

    const card = within(screen.getByRole("article", { name: systemPersona.name }));
    expect(card.queryByRole("button", { name: `删除 ${systemPersona.name}` })).toBeNull();
    expect(card.getByText("系统默认人设不能删除。")).toBeTruthy();
  });

  it("删除当前普通人设前显示级联与回退说明，确认后删除并刷新", async () => {
    api.deletePersona.mockImplementation(async (id: string) => {
      expect(id).toBe(currentPersona.id);
      rows = [{ ...systemPersona, isActive: true }, otherPersona];
    });
    await loadPanel();

    fireEvent.click(screen.getByRole("button", { name: `删除 ${currentPersona.name}` }));
    const modal = within(screen.getByRole("dialog"));
    expect(modal.getByText(/全部会话、聊天、记忆、记事本、出行、游记、信件、伴手礼、饰品和节日记录/)).toBeTruthy();
    expect(modal.getByText(/当前使用项会切回系统默认人设/)).toBeTruthy();
    expect(api.deletePersona).not.toHaveBeenCalled();

    const refreshCount = api.listPersonas.mock.calls.length;
    fireEvent.click(modal.getByRole("button", { name: "确认" }));
    await waitFor(() => expect(api.deletePersona).toHaveBeenCalledWith(currentPersona.id));
    await waitFor(() => expect(api.listPersonas.mock.calls.length).toBeGreaterThan(refreshCount));
    expect(await within(screen.getByRole("article", { name: systemPersona.name })).findByText(/当前使用 · 系统默认/)).toBeTruthy();
  });

  it("切换人设后使用重新读取的后端 active 状态", async () => {
    api.setPersona.mockImplementation(async (id: string) => {
      rows = rows.map(persona => ({ ...persona, isActive: persona.id === id }));
    });
    await loadPanel();

    fireEvent.click(screen.getByRole("button", { name: `切换为 ${otherPersona.name}` }));

    await waitFor(() => expect(api.setPersona).toHaveBeenCalledWith(otherPersona.id));
    await waitFor(() => expect(screen.getByRole("article", { name: otherPersona.name }).textContent).toContain("当前使用"));
    expect(screen.getByRole("article", { name: currentPersona.name }).textContent).not.toContain("当前使用");
  });

  it("保存为初始设定和恢复操作都保留聊天背景参数", async () => {
    await loadPanel();
    const card = within(screen.getByRole("article", { name: currentPersona.name }));

    fireEvent.click(card.getByRole("button", { name: "保存为初始设定" }));
    let modal = within(screen.getByRole("dialog"));
    expect(modal.getByText(/替换初始设定/)).toBeTruthy();
    fireEvent.click(modal.getByRole("button", { name: "确认" }));
    await waitFor(() => expect(api.updatePersona).toHaveBeenNthCalledWith(1, currentPersona.id, {
      name: currentPersona.name,
      prompt: currentPersona.prompt,
      color: currentPersona.color,
      selfEvolveEnabled: currentPersona.selfEvolveEnabled,
      chatBgOpacity: currentPersona.chatBgOpacity,
      chatBgBlur: currentPersona.chatBgBlur,
      saveAsInitial: true,
    }));

    fireEvent.click(card.getByRole("button", { name: "从初始设定恢复" }));
    modal = within(screen.getByRole("dialog"));
    expect(modal.getByText(/聊天和记忆记录保留/)).toBeTruthy();
    fireEvent.click(modal.getByRole("button", { name: "确认" }));
    await waitFor(() => expect(api.updatePersona).toHaveBeenNthCalledWith(2, currentPersona.id, {
      name: currentPersona.name,
      prompt: currentPersona.initialPrompt,
      color: currentPersona.color,
      selfEvolveEnabled: currentPersona.selfEvolveEnabled,
      chatBgOpacity: currentPersona.chatBgOpacity,
      chatBgBlur: currentPersona.chatBgBlur,
      saveAsInitial: false,
    }));
  });

  it("按人设 ID 显示表情生成失败并保留已有表情", async () => {
    await loadPanel();
    expect(screen.getByText("开心旧表情")).toBeTruthy();

    await act(async () => {
      expressionFailure?.({ personaId: currentPersona.id, error: { message: "生成服务暂不可用" } });
    });

    expect(screen.getByText(/表情库生成未完成：生成服务暂不可用。已有表情库保留/)).toBeTruthy();
    expect(screen.getByText("开心旧表情")).toBeTruthy();
  });

  it("手动生成等待期间可用正确人设 ID 取消", async () => {
    let finish!: () => void;
    api.generateExpressions.mockImplementation(() => new Promise<void>(resolve => { finish = resolve; }));
    await loadPanel();

    fireEvent.click(screen.getByRole("button", { name: `重新生成 ${currentPersona.name} 表情库` }));
    await screen.findByRole("button", { name: "取消生成" });
    fireEvent.click(screen.getByRole("button", { name: "取消生成" }));
    await waitFor(() => expect(api.cancelExpressionGeneration).toHaveBeenCalledWith(currentPersona.id));
    await act(async () => finish());
  });

  it("卸载时取消仍在等待的表情生成", async () => {
    api.generateExpressions.mockImplementation(() => new Promise<void>(() => {}));
    const view = render(<PersonasPanel />);
    await screen.findByRole("article", { name: currentPersona.name });
    fireEvent.click(screen.getByRole("button", { name: `重新生成 ${currentPersona.name} 表情库` }));
    await screen.findByRole("button", { name: "取消生成" });

    view.unmount();

    expect(api.cancelExpressionGeneration).toHaveBeenCalledWith(currentPersona.id);
  });
});
