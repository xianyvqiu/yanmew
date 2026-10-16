import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import UserProfilePicker from "./UserProfilePicker";
const profiles = [{ id: "7", name: "日常", content: "喜欢猫咪", createdAt: "2026-10-15T00:00:00Z", updatedAt: "2026-10-15T00:00:00Z" }];
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);
it("绑定、详情和关闭是独立操作，详情不会隐式绑定", () => {
  const select=vi.fn(), details=vi.fn(), close=vi.fn();
  render(<UserProfilePicker profiles={profiles} selectedId={null} busy={false} error="" onSelect={select} onDetails={details} onClose={close} />);
  fireEvent.click(screen.getByRole("button", { name:"查看用户设定详情：日常" }));
  expect(details).toHaveBeenCalledWith("7"); expect(select).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name:"绑定 日常" })); expect(select).toHaveBeenCalledWith("7");
  fireEvent.click(screen.getByRole("button", { name:"关闭" })); expect(close).toHaveBeenCalledOnce();
});
it("当前绑定可识别且可解除，搜索过滤与错误提示保留", () => {
  const select=vi.fn();
  render(<UserProfilePicker profiles={profiles} selectedId="7" busy={false} error="绑定失败" onSelect={select} onDetails={() => {}} onClose={() => {}} />);
  expect(screen.getByRole("button", { name:"绑定 日常" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByRole("alert").textContent).toBe("绑定失败");
  fireEvent.click(screen.getByRole("button", { name:"解除当前绑定" })); expect(select).toHaveBeenCalledWith(null);
  fireEvent.change(screen.getByRole("searchbox"), {target:{value:"无匹配"}});
  expect(screen.queryByRole("button", {name:"绑定 日常"})).toBeNull();
});
