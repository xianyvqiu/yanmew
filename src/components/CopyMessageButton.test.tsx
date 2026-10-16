import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import CopyMessageButton from "./CopyMessageButton";

afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.useRealTimers(); });
function clipboard(writeText = vi.fn().mockResolvedValue(undefined)) {
  vi.stubGlobal("navigator", { clipboard: { writeText } });
  return writeText;
}
it("复制完整原文并在成功后显示反馈，流式更新后可复制最新文字", async () => {
  const write = clipboard();
  const text = "第一行\n(^_^) [来源](https://example.com)";
  const page = render(<CopyMessageButton content={text} />);
  fireEvent.click(screen.getByRole("button", { name: "复制" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "已复制" })).toBeTruthy());
  expect(write).toHaveBeenLastCalledWith(text);
  page.rerender(<CopyMessageButton content={text + "\n新增片段"} />);
  fireEvent.click(screen.getByRole("button", { name: "已复制" }));
  await waitFor(() => expect(write).toHaveBeenLastCalledWith(text + "\n新增片段"));
});
it("剪贴板拒绝时不显示假成功，允许用户重试", async () => {
  const write = clipboard(vi.fn().mockRejectedValueOnce(new Error("NotAllowedError")).mockResolvedValue(undefined));
  render(<CopyMessageButton content="消息正文" />);
  fireEvent.click(screen.getByRole("button", { name: "复制" }));
  await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("复制失败"));
  expect(screen.queryByRole("button", { name: "已复制" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "复制" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "已复制" })).toBeTruthy());
  expect(write).toHaveBeenCalledTimes(2);
});
it("没有正文时禁用复制，不复制占位提示或图片路径", () => {
  const write = clipboard();
  render(<CopyMessageButton content="  " />);
  const button = screen.getByRole("button", { name: "复制" }) as HTMLButtonElement;
  expect(button.disabled).toBe(true);
  fireEvent.click(button);
  expect(write).not.toHaveBeenCalled();
});
it("成功提示两秒后恢复，卸载清理提示计时器", async () => {
  vi.useFakeTimers(); clipboard();
  const page = render(<CopyMessageButton content="消息" />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "复制" })); });
  expect(screen.getByRole("button", { name: "已复制" })).toBeTruthy();
  act(() => { vi.advanceTimersByTime(2000); });
  expect(screen.getByRole("button", { name: "复制" })).toBeTruthy();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "复制" })); });
  page.unmount();
  expect(vi.getTimerCount()).toBe(0);
});
