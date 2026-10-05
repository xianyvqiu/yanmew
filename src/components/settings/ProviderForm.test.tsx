import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Provider, ProviderInput } from "../../types";
import ProviderForm from "./ProviderForm";

afterEach(() => { cleanup(); });

const provider: Provider = {
  id: "provider-1",
  name: "测试提供商",
  baseUrl: "https://example.com/v1",
  model: "test-model",
  hasApiKey: true,
  isActive: false,
  memoryIsActive: false,
  maxOutputTokens: 32768,
  contextWindowOverride: null,
  capacity: { contextWindow: null, maxOutputTokens: null, source: "unknown", estimated: true },
};

function renderForm(overrides: Partial<{
  provider: Provider | null;
  busy: boolean;
  error: { code: string; message: string; field?: string } | null;
}> = {}) {
  const onSave = vi.fn(async (_input: ProviderInput) => {});
  const onCancel = vi.fn();
  const props = {
    provider,
    busy: false,
    error: null,
    onSave,
    onCancel,
    ...overrides,
  };
  const view = render(<ProviderForm {...props} />);
  return { ...view, onSave, onCancel };
}

async function submit(container: HTMLElement) {
  await act(async () => {
    fireEvent.submit(container.querySelector("form")!);
  });
}

describe("ProviderForm", () => {
  it("新增时默认最大输出 32768，上下文容量自动且高级设置可折叠", async () => {
    const { container, onSave } = renderForm({ provider: null });
    expect(container.querySelector("details")?.open).toBe(false);
    expect(screen.getByLabelText("最大输出（tokens）")).toHaveProperty("value", "32768");
    expect(screen.getByLabelText("上下文容量（tokens）")).toHaveProperty("value", "");
    fireEvent.change(screen.getByLabelText("提供商名称"), { target: { value: "新配置" } });
    fireEvent.change(screen.getByLabelText("模型名称"), { target: { value: "new-model" } });
    await submit(container);
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ maxOutputTokens: 32768, contextWindowOverride: null }));
  });

  it("编辑时恢复手动容量，清空后提交自动模式", async () => {
    const { container, onSave } = renderForm({ provider: { ...provider, maxOutputTokens: 4096, contextWindowOverride: 128000 } });
    expect(screen.getByLabelText("最大输出（tokens）")).toHaveProperty("value", "4096");
    expect(screen.getByLabelText("上下文容量（tokens）")).toHaveProperty("value", "128000");
    await submit(container);
    expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({ maxOutputTokens: 4096, contextWindowOverride: 128000 }));
    fireEvent.change(screen.getByLabelText("上下文容量（tokens）"), { target: { value: "" } });
    await submit(container);
    expect(onSave).toHaveBeenLastCalledWith(expect.objectContaining({ contextWindowOverride: null }));
  });

  it("高级设置拒绝非正整数并保留输入", async () => {
    const { container, onSave } = renderForm();
    fireEvent.change(screen.getByLabelText("最大输出（tokens）"), { target: { value: "0" } });
    await submit(container);
    expect(screen.getByRole("alert").textContent).toBe("最大输出必须是正整数");
    expect(container.querySelector("details")?.open).toBe(true);
    expect(onSave).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("最大输出（tokens）"), { target: { value: "32768" } });
    fireEvent.change(screen.getByLabelText("上下文容量（tokens）"), { target: { value: "1.5" } });
    await submit(container);
    expect(screen.getByRole("alert").textContent).toBe("上下文容量应留空或填写正整数");
    expect(screen.getByLabelText("上下文容量（tokens）")).toHaveProperty("value", "1.5");
    expect(onSave).not.toHaveBeenCalled();
  });

  it("Embedding 表单不显示或提交聊天输出和上下文字段", async () => {
    const onSave = vi.fn(async (_input: ProviderInput) => {});
    const { container } = render(<ProviderForm embedding provider={provider} busy={false} error={null} onSave={onSave} onCancel={vi.fn()} />);
    expect(screen.queryByText("高级设置")).toBeNull();
    expect(screen.queryByLabelText("最大输出（tokens）")).toBeNull();
    await submit(container);
    expect(onSave).toHaveBeenCalledWith({ name: provider.name, baseUrl: provider.baseUrl, model: provider.model, apiKey: { action: "keep" } });
  });

  it("编辑时 API Key 留空会保留已保存密钥", async () => {
    const { container, onSave } = renderForm();

    await submit(container);

    expect(onSave).toHaveBeenCalledWith({
      name: provider.name,
      baseUrl: provider.baseUrl,
      model: provider.model,
      apiKey: { action: "keep" },
      maxOutputTokens: 32768,
      contextWindowOverride: null,
    });
  });

  it("新 API Key 按原文提交，不去除空白", async () => {
    const { container, onSave } = renderForm();
    const key = "  sk-test-key  ";

    fireEvent.change(screen.getByLabelText("API Key"), { target: { value: key } });
    await submit(container);

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({
      apiKey: { action: "set", value: key },
    }));
  });

  it("清除已保存密钥后提交 clear", async () => {
    const { container, onSave } = renderForm();

    fireEvent.click(screen.getByRole("button", { name: "清除已保存密钥" }));
    await submit(container);

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ apiKey: { action: "clear" } }));
  });

  it("撤销清除后提交 keep", async () => {
    const { container, onSave } = renderForm();

    fireEvent.click(screen.getByRole("button", { name: "清除已保存密钥" }));
    fireEvent.click(screen.getByRole("button", { name: "撤销清除" }));
    await submit(container);

    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ apiKey: { action: "keep" } }));
  });

  it("字段错误通过 aria-describedby 关联，并在提交后保留输入", async () => {
    const { container, onSave } = renderForm({
      error: { code: "invalid", message: "模型名称无效", field: "model" },
    });
    const modelInput = screen.getByLabelText("模型名称");

    fireEvent.change(modelInput, { target: { value: "draft-model" } });
    expect(modelInput.getAttribute("aria-invalid")).toBe("true");
    const errorId = modelInput.getAttribute("aria-describedby");
    expect(errorId).toBeTruthy();
    const errorMessage = document.getElementById(errorId!);
    expect(errorMessage?.textContent).toBe("模型名称无效");
    expect(modelInput.parentElement?.contains(errorMessage)).toBe(true);

    await submit(container);

    expect(modelInput).toHaveProperty("value", "draft-model");
    expect(onSave).toHaveBeenCalledWith(expect.objectContaining({ model: "draft-model" }));
  });

  it("忙碌时通过 fieldset 禁用表单控件", () => {
    const { container } = renderForm({ busy: true });

    expect(container.querySelector("fieldset")?.disabled).toBe(true);
    expect(screen.getByLabelText("API Key").matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "正在保存…" }).matches(":disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "取消" }).matches(":disabled")).toBe(true);
  });
});
