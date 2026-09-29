import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import ChatComposer from "./ChatComposer";

afterEach(() => { cleanup(); });

function renderComposer(overrides: Partial<{
  value: string;
  onChange: (value: string) => void;
  onSend: () => void;
  onCancel: () => void;
  disabled: boolean;
  busy: boolean;
  cancelling: boolean;
}> = {}) {
  const props = {
    value: "",
    onChange: vi.fn(),
    onSend: vi.fn(),
    onCancel: vi.fn(),
    disabled: false,
    busy: false,
    cancelling: false,
    ...overrides,
  };

  render(<ChatComposer {...props} />);
  return props;
}

describe("ChatComposer", () => {
  it("点击发送按钮时发送非空消息", () => {
    const { onSend } = renderComposer({ value: "你好" });

    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("空白消息禁用发送按钮", () => {
    const { onSend } = renderComposer({ value: " \n  " });
    const sendButton = screen.getByRole("button", { name: "发送" });

    expect((sendButton as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(sendButton);
    expect(onSend).not.toHaveBeenCalled();
  });

  it("Enter 发送，Shift+Enter 保留换行", () => {
    const { onSend } = renderComposer({ value: "你好" });
    const input = screen.getByRole("textbox", { name: "聊天消息" });

    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSend).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("输入法组合期间不因 Enter 发送", () => {
    const { onSend } = renderComposer({ value: "你好" });
    const input = screen.getByRole("textbox", { name: "聊天消息" });

    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    fireEvent.keyDown(input, { key: "Enter", keyCode: 229 });

    expect(onSend).not.toHaveBeenCalled();
  });

  it("忙碌时显示暂停回复并允许编辑下一条草稿", () => {
    const { onChange, onCancel } = renderComposer({ value: "草稿", busy: true });
    const input = screen.getByRole("textbox", { name: "聊天消息" });

    expect((screen.getByRole("button", { name: "暂停回复" }) as HTMLButtonElement).disabled).toBe(false);
    expect((input as HTMLTextAreaElement).disabled).toBe(false);
    fireEvent.change(input, { target: { value: "新的草稿" } });
    expect(onChange).toHaveBeenCalledWith("新的草稿");

    fireEvent.click(screen.getByRole("button", { name: "暂停回复" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("正在暂停时禁用暂停按钮并显示状态文字", () => {
    renderComposer({ busy: true, cancelling: true });
    const pauseButton = screen.getByRole("button", { name: "正在暂停…" });

    expect((pauseButton as HTMLButtonElement).disabled).toBe(true);
  });
});
