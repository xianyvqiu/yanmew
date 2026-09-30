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
  it("编辑时 API Key 留空会保留已保存密钥", async () => {
    const { container, onSave } = renderForm();

    await submit(container);

    expect(onSave).toHaveBeenCalledWith({
      name: provider.name,
      baseUrl: provider.baseUrl,
      model: provider.model,
      apiKey: { action: "keep" },
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
