import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Persona, PersonaInput } from "../../types";
import PersonaForm from "./PersonaForm";

afterEach(() => { cleanup(); });

const persona: Persona = {
  id: "persona-1",
  name: "小喵",
  prompt: "温柔地陪伴用户",
  initialPrompt: "最初的设定",
  selfEvolveEnabled: false,
  lastEvolvedAt: null,
  color: "#aabbcc",
  chatBgFile: "background.webp",
  chatBgOpacity: 62,
  chatBgBlur: 12,
  isActive: true,
  isBuiltin: false,
  expressions: null,
};

function renderForm(overrides: Partial<{
  persona: Persona | null;
  busy: boolean;
  error: { code: string; message: string; field?: string } | null;
}> = {}) {
  const onSave = vi.fn(async (_input: PersonaInput) => {});
  const onCancel = vi.fn();
  const props = { persona, busy: false, error: null, onSave, onCancel, ...overrides };
  const view = render(<PersonaForm {...props} />);
  return { ...view, onSave, onCancel };
}

async function submit(container: HTMLElement) {
  await act(async () => {
    fireEvent.submit(container.querySelector("form")!);
  });
}

describe("PersonaForm", () => {
  it("编辑时保留聊天背景设置且不覆盖初始设定", async () => {
    const { container, onSave } = renderForm();

    await submit(container);

    expect(onSave).toHaveBeenCalledWith({
      name: persona.name,
      prompt: persona.prompt,
      initialPrompt: persona.initialPrompt,
      selfEvolveEnabled: persona.selfEvolveEnabled,
      color: persona.color,
      chatBgOpacity: persona.chatBgOpacity,
      chatBgBlur: persona.chatBgBlur,
      saveAsInitial: false,
    });
  });

  it("新建时使用默认值且不请求单独保存初始设定", async () => {
    const { container, onSave } = renderForm({ persona: null });
    fireEvent.change(screen.getByLabelText("人设名称"), { target: { value: "新伙伴" } });
    fireEvent.change(screen.getByLabelText("当前设定"), { target: { value: "愿意倾听" } });

    await submit(container);

    expect(onSave).toHaveBeenCalledWith({
      name: "新伙伴",
      prompt: "愿意倾听",
      initialPrompt: "愿意倾听",
      selfEvolveEnabled: true,
      color: "#ffb6c1",
      chatBgOpacity: 85,
      chatBgBlur: 0,
      saveAsInitial: false,
    });
  });

  it("保存失败返回字段错误时保留草稿", async () => {
    const { container, onSave } = renderForm({
      error: { code: "invalid", message: "人设名称已存在", field: "name" },
    });
    const nameInput = screen.getByLabelText("人设名称");
    fireEvent.change(nameInput, { target: { value: "我的草稿" } });

    await submit(container);

    expect(nameInput).toHaveProperty("value", "我的草稿");
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ name: "我的草稿" }));
  });

  it("忙碌时由 fieldset 禁用控件并阻止提交", async () => {
    const { container, onSave } = renderForm({ busy: true });

    expect(container.querySelector("fieldset")?.disabled).toBe(true);
    expect(screen.getByLabelText("当前设定").matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "正在保存…" }).matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "取消" }).matches(":disabled")).toBe(true);

    await submit(container);

    expect(onSave).not.toHaveBeenCalled();
  });
});
