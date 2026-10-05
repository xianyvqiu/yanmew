import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UserProfile } from "../../types";
import UserProfilePanel from "./UserProfilePanel";

const api = vi.hoisted(() => ({ list: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn(), listenChanges: vi.fn() }));
vi.mock("../../api", () => ({ userProfileApi: api, configurationApi: api, errorMessage: (error: { message: string }) => error.message }));

const profile: UserProfile = { id: "profile-1", name: "我的设定", content: "喜欢猫咪", createdAt: "2026-10-01T08:00:00Z", updatedAt: "2026-10-06T08:00:00Z" };
let rows: UserProfile[];
let changed: (() => void) | undefined;
let stop: ReturnType<typeof vi.fn>;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.resetAllMocks();
  rows = [profile];
  changed = undefined;
  stop = vi.fn();
  api.list.mockImplementation(async () => [...rows]);
  api.listenChanges.mockImplementation(async (callback: () => void) => { changed = callback; return stop; });
  api.create.mockImplementation(async (input: { name: string; content: string }) => {
    const result = { ...profile, id: "profile-2", ...input };
    rows = [...rows, result];
    return result;
  });
  api.update.mockImplementation(async (input: { id: string; name: string; content: string }) => {
    const result = { ...profile, ...input };
    rows = rows.map(item => item.id === result.id ? result : item);
    return result;
  });
  api.delete.mockImplementation(async (id: string) => { rows = rows.filter(item => item.id !== id); });
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);

async function loadPanel() {
  const view = render(<UserProfilePanel />);
  await screen.findByRole("article", { name: profile.name });
  return view;
}

describe("UserProfilePanel", () => {
  it("名称和内容去空白后必须非空，创建保存不限正文长度且取消不调用 API", async () => {
    await loadPanel();
    expect(screen.queryByRole("button", { name: /导入|导出/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "新建用户设定" }));
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "   " } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(screen.getByRole("alert").textContent).toBe("名称不能为空");
    expect(api.create).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: " 新设定 " } });
    fireEvent.change(screen.getByLabelText("内容"), { target: { value: " \n " } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(screen.getByRole("alert").textContent).toBe("内容不能为空");
    const content = "详细设定".repeat(3000);
    expect(screen.getByLabelText("内容").getAttribute("maxlength")).toBeNull();
    fireEvent.change(screen.getByLabelText("内容"), { target: { value: ` ${content}\n ` } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByRole("article", { name: "新设定" });
    expect(api.create).toHaveBeenCalledWith({ name: "新设定", content });
    fireEvent.click(screen.getByRole("button", { name: "新建用户设定" }));
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    expect(screen.queryByLabelText("名称")).toBeNull();
    expect(api.create).toHaveBeenCalledTimes(1);
  });

  it("保存期间禁止重复请求，后端错误保留草稿，配置补查不覆盖草稿，重试按 ID 保存", async () => {
    const saving = deferred<UserProfile>();
    api.update.mockReturnValueOnce(saving.promise);
    await loadPanel();
    fireEvent.click(screen.getByRole("button", { name: `编辑 ${profile.name}` }));
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: " 改后名称 " } });
    fireEvent.change(screen.getByLabelText("内容"), { target: { value: " 草稿正文 " } });
    const form = screen.getByLabelText("名称").closest("form")!;
    fireEvent.submit(form);
    fireEvent.submit(form);
    expect(api.update).toHaveBeenCalledTimes(1);
    expect((screen.getByRole("button", { name: "正在保存…" }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => saving.reject({ code: "REQUEST_BUSY", message: "此用户设定正在使用中，请稍后重试" }));
    expect(screen.getByRole("alert").textContent).toBe("此用户设定正在使用中，请稍后重试");
    rows = [{ ...profile, name: "另一窗口修改" }];
    await act(async () => changed?.());
    await screen.findByRole("article", { name: "另一窗口修改" });
    expect((screen.getByLabelText("名称") as HTMLInputElement).value).toBe(" 改后名称 ");
    expect((screen.getByLabelText("内容") as HTMLTextAreaElement).value).toBe(" 草稿正文 ");
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    await screen.findByRole("article", { name: "改后名称" });
    expect(api.update).toHaveBeenLastCalledWith({ id: profile.id, name: "改后名称", content: "草稿正文" });
    expect(screen.queryByLabelText("内容")).toBeNull();
  });

  it("删除确认说明会话与聊天记忆保留，REQUEST_BUSY 不删除卡片，确认重试才刷新", async () => {
    api.delete.mockRejectedValueOnce({ code: "REQUEST_BUSY", message: "请先停止使用此设定的请求" });
    await loadPanel();
    fireEvent.click(screen.getByRole("button", { name: `删除 ${profile.name}` }));
    const modal = within(screen.getByRole("dialog"));
    expect(modal.getByText(/绑定它的会话不会被删除，只会变为未绑定；聊天与记忆保留/)).toBeTruthy();
    expect(api.delete).not.toHaveBeenCalled();
    fireEvent.click(modal.getByRole("button", { name: "确认删除" }));
    expect(await modal.findByRole("alert")).toBeTruthy();
    expect(screen.getByRole("article", { name: profile.name })).toBeTruthy();
    expect(modal.getByRole("alert").textContent).toBe("请先停止使用此设定的请求");
    fireEvent.click(modal.getByRole("button", { name: "确认删除" }));
    await waitFor(() => expect(screen.queryByRole("article", { name: profile.name })).toBeNull());
    expect(api.delete).toHaveBeenCalledTimes(2);
    expect(api.delete).toHaveBeenLastCalledWith(profile.id);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("焦点和事件补查丢弃旧响应及旧错误，卸载清理监听", async () => {
    const view = await loadPanel();
    const older = deferred<UserProfile[]>();
    const newest = deferred<UserProfile[]>();
    api.list.mockReturnValueOnce(older.promise).mockReturnValueOnce(newest.promise);
    act(() => { fireEvent.focus(window); changed?.(); });
    await act(async () => newest.resolve([{ ...profile, name: "最新名称" }]));
    expect(screen.getByRole("article", { name: "最新名称" })).toBeTruthy();
    await act(async () => older.reject({ message: "旧请求错误" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("article", { name: "最新名称" })).toBeTruthy();
    const count = api.list.mock.calls.length;
    view.unmount();
    expect(stop).toHaveBeenCalledTimes(1);
    act(() => { fireEvent.focus(window); changed?.(); });
    expect(api.list).toHaveBeenCalledTimes(count);
  });

  it("保存使旧列表失效，操作中配置事件延后补查，不复活保存前内容", async () => {
    await loadPanel();
    fireEvent.click(screen.getByRole("button", { name: `编辑 ${profile.name}` }));
    fireEvent.change(screen.getByLabelText("名称"), { target: { value: "已保存名称" } });
    const older = deferred<UserProfile[]>();
    const saving = deferred<UserProfile>();
    api.list.mockReturnValueOnce(older.promise);
    api.update.mockReturnValueOnce(saving.promise);
    act(() => { fireEvent.focus(window); });
    fireEvent.submit(screen.getByLabelText("名称").closest("form")!);
    const count = api.list.mock.calls.length;
    act(() => changed?.());
    expect(api.list).toHaveBeenCalledTimes(count);
    const saved = { ...profile, name: "已保存名称" };
    rows = [saved];
    await act(async () => saving.resolve(saved));
    await screen.findByRole("article", { name: saved.name });
    await act(async () => older.resolve([profile]));
    expect(screen.queryByRole("article", { name: profile.name })).toBeNull();
    expect(screen.getByRole("article", { name: saved.name })).toBeTruthy();
  });

  it("卸载后才完成的订阅也被释放，读取失败可重试", async () => {
    const subscription = deferred<() => void>();
    api.listenChanges.mockReturnValueOnce(subscription.promise);
    const view = render(<UserProfilePanel />);
    view.unmount();
    await act(async () => subscription.resolve(stop));
    expect(stop).toHaveBeenCalledTimes(1);
    expect(api.list).not.toHaveBeenCalled();
    api.list.mockRejectedValueOnce({ message: "读取用户设定失败" });
    render(<UserProfilePanel />);
    expect((await screen.findByRole("alert")).textContent).toContain("读取用户设定失败");
    fireEvent.click(screen.getByRole("button", { name: "重新读取" }));
    expect(await screen.findByRole("article", { name: profile.name })).toBeTruthy();
  });
});
