import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import PersonaPromptDiff from "./PersonaPromptDiff";

afterEach(cleanup);

function open() {
  const button = screen.getByRole("button", { name: "查看与初始设定的差异" });
  button.focus();
  expect(document.activeElement).toBe(button);
  fireEvent.click(button);
  expect(button.getAttribute("aria-expanded")).toBe("true");
  expect(document.getElementById(button.getAttribute("aria-controls")!)).toBeTruthy();
}

describe("PersonaPromptDiff", () => {
  it("默认收起，展开后以文字和 ins/del 语义标记新增删除，安全显示原文", () => {
    const { container } = render(<PersonaPromptDiff initialPrompt="表达冷淡。" currentPrompt="表达克制。<script>文字</script>" />);
    expect(screen.queryByRole("heading", { name: "当前与初始设定的累计差异" })).toBeNull();
    open();
    expect(screen.getByText(/包含 AI 自调和手动编辑/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "行内对比" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getAllByText("新增").length).toBeGreaterThan(0);
    expect(screen.getByText("删除").closest("del")?.textContent).toContain("冷淡");
    expect(container.querySelector("ins.ym-prompt-diff-added")).toBeTruthy();
    expect(container.querySelector("del.ym-prompt-diff-removed")).toBeTruthy();
    expect(container.querySelector("script")).toBeNull();
    expect(screen.getByLabelText("行内差异").textContent).toContain("<script>文字</script>");
    fireEvent.click(screen.getByRole("button", { name: "查看与初始设定的差异" }));
    expect(screen.queryByLabelText("行内差异")).toBeNull();
  });

  it("左右对比各自保留完整对应文本及明确标题，不混入另一侧改动", () => {
    const { container } = render(<PersonaPromptDiff initialPrompt="表达冷淡。" currentPrompt="表达克制。" />);
    open();
    fireEvent.click(screen.getByRole("button", { name: "左右对比" }));
    const before = screen.getByRole("region", { name: "初始设定对比" });
    const after = screen.getByRole("region", { name: "当前设定对比" });
    expect(within(before).getByText("删除")).toBeTruthy();
    expect(within(before).queryByText("新增")).toBeNull();
    expect(within(after).getByText("新增")).toBeTruthy();
    expect(within(after).queryByText("删除")).toBeNull();
    expect(before.textContent).not.toContain("克制");
    expect(after.textContent).not.toContain("冷淡");
    expect(container.querySelector(".ym-prompt-diff-columns")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "行内对比" }));
    expect(screen.getByLabelText("行内差异")).toBeTruthy();
  });

  it("新的已保存文本刷新差异，重设基准后即时显示一致状态", () => {
    const { rerender } = render(<PersonaPromptDiff initialPrompt="冷淡。" currentPrompt="克制。" />);
    open();
    rerender(<PersonaPromptDiff initialPrompt="冷淡。" currentPrompt="冷淡但耐心。" />);
    expect(screen.getByLabelText("行内差异").textContent).toContain("但耐心");
    rerender(<PersonaPromptDiff initialPrompt="冷淡但耐心。" currentPrompt="冷淡但耐心。" />);
    expect(screen.getByRole("status").textContent).toBe("当前人设与初始设定一致");
    expect(screen.queryByText("新增")).toBeNull();
    expect(screen.queryByRole("group", { name: "差异显示方式" })).toBeNull();
  });

  it.each([["", ""], ["慢热。 \r\n耐心。", "慢热。\n耐心。"]])("空文本或规范化后相同文本使用一致状态", (initialPrompt, currentPrompt) => {
    render(<PersonaPromptDiff initialPrompt={initialPrompt} currentPrompt={currentPrompt} />);
    open();
    expect(screen.getByRole("status").textContent).toBe("当前人设与初始设定一致");
  });
});
