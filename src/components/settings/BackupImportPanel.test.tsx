import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import BackupImportPanel from "./BackupImportPanel";
import type { BackupImportPreview } from "../../types";
const api = vi.hoisted(() => ({ prepare: vi.fn(), refresh: vi.fn(), cancel: vi.fn(), confirm: vi.fn() }));
vi.mock("../../api", () => ({ backupApi: { prepareImport: api.prepare, refreshImport: api.refresh, cancelImport: api.cancel, confirmImport: api.confirm }, errorMessage: (e: { message: string }) => e.message }));
const preview = (): BackupImportPreview => ({ previewId: "preview-1", kind: "session", exportedAt: "2026-10-13T00:00:00Z", counts: { sessions: 1, messages: 4 }, warnings: [], personas: [{ sourceId: "9007199254740993", name: "电子小猫", isBuiltin: false, candidates: [{ id: "2", name: "电子小猫", stamp: "snapshot", different: true }], suggestedTargetId: "2", requiresChoice: false }], profiles: [], plugins: [] });
afterEach(cleanup);
beforeEach(() => { vi.resetAllMocks(); api.prepare.mockResolvedValue(preview()); api.refresh.mockResolvedValue(preview()); api.cancel.mockResolvedValue(undefined); api.confirm.mockResolvedValue({ sessions: 1, personasCreated: 0, personasReused: 1, warnings: [] }); });
it("只获取预览，明确确认后才传递人设关联和字符串ID", async () => {
  render(<BackupImportPanel />); fireEvent.click(screen.getByRole("button", { name: "选择备份并预览" }));
  await screen.findByText(/两边设定有差异/); expect(api.confirm).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "确认导入" })); await screen.findByText("导入完成，已新增 1 个会话。");
  expect(api.confirm).toHaveBeenCalledWith({ previewId: "preview-1", personas: [{ sourceId: "9007199254740993", targetId: "2" }], profiles: [], confirmGlobalRestore: false });
});
it("多个同名候选必须选择，新建用null而不是猜测ID", async () => {
  const value = preview(); value.personas[0].candidates.push({ id: "3", name: "电子小猫", stamp: "other", different: false }); value.personas[0].requiresChoice = true; value.personas[0].suggestedTargetId = null;
  api.prepare.mockResolvedValue(value); render(<BackupImportPanel />); fireEvent.click(screen.getByRole("button", { name: "选择备份并预览" }));
  const select = await screen.findByLabelText("人设关联：电子小猫"); expect(screen.getByRole("button", { name: "确认导入" }).hasAttribute("disabled")).toBe(true);
  fireEvent.change(select, { target: { value: "new" } }); fireEvent.click(screen.getByRole("button", { name: "确认导入" }));
  await waitFor(() => expect(api.confirm).toHaveBeenCalledWith(expect.objectContaining({ personas: [{ sourceId: "9007199254740993", targetId: null }] })));
});
it("全量恢复必须确认全局替换范围，默认人设不可改绑", async () => {
  const value = preview(); value.kind = "full"; value.personas[0].isBuiltin = true; api.prepare.mockResolvedValue(value);
  render(<BackupImportPanel />); fireEvent.click(screen.getByRole("button", { name: "选择备份并预览" }));
  const check = await screen.findByRole("checkbox"); expect(screen.getByLabelText("人设关联：电子小猫").hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("button", { name: "确认导入" }).hasAttribute("disabled")).toBe(true);
  fireEvent.click(check); fireEvent.click(screen.getByRole("button", { name: "确认导入" })); await waitFor(() => expect(api.confirm).toHaveBeenCalledWith(expect.objectContaining({ confirmGlobalRestore: true })));
});
it("提交失败保留预览可刷新，不显示成功", async () => {
  api.confirm.mockRejectedValue({ message: "关联目标已变化" }); render(<BackupImportPanel />); fireEvent.click(screen.getByRole("button", { name: "选择备份并预览" }));
  fireEvent.click(await screen.findByRole("button", { name: "确认导入" })); await screen.findByRole("alert"); expect(screen.queryByText(/导入完成/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "刷新预览" })); await waitFor(() => expect(api.refresh).toHaveBeenCalledWith("preview-1"));
});
it("选择取消不提交，取消已有预览释放后端暂存", async () => {
  api.prepare.mockResolvedValueOnce(null); render(<BackupImportPanel />); fireEvent.click(screen.getByRole("button", { name: "选择备份并预览" }));
  await screen.findByText("已取消选择，当前数据未修改。"); expect(api.confirm).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "选择备份并预览" })); fireEvent.click(await screen.findByRole("button", { name: "取消导入" }));
  await screen.findByText("已取消导入，当前数据未修改。"); expect(api.cancel).toHaveBeenCalledWith("preview-1");
});
it("页面在选择完成前关闭，也会释放迟到的预览", async () => {
  let finish!: (value: BackupImportPreview) => void; api.prepare.mockImplementation(() => new Promise<BackupImportPreview>(resolve => { finish = resolve; }));
  const view = render(<BackupImportPanel />); fireEvent.click(screen.getByRole("button", { name: "选择备份并预览" })); view.unmount(); finish(preview());
  await waitFor(() => expect(api.cancel).toHaveBeenCalledWith("preview-1")); expect(api.confirm).not.toHaveBeenCalled();
});
