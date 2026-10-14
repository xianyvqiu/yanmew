import { beforeEach, expect, it, vi } from "vitest";
import { backupApi } from "./api";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
beforeEach(() => { vi.resetAllMocks(); invoke.mockResolvedValue({ filename: "backup.zip", manifest: {} }); });
it("全量导出使用空选择且不增加会话包含开关", async () => {
  const input = { kind: "full" as const, ids: [], includeSessions: false };
  await backupApi.export(input);
  expect(invoke).toHaveBeenCalledWith("export_backup", { input });
});
it("用户设定包显式传递带会话开关和字符串ID", async () => {
  const input = { kind: "user_profile" as const, ids: ["9007199254740993"], includeSessions: true };
  await backupApi.export(input);
  expect(invoke).toHaveBeenCalledWith("export_backup", { input });
});
it("插件数据导出只传插件ID，不提交钱包或授权", async () => {
  await backupApi.exportPluginData(["test.game"]);
  expect(invoke).toHaveBeenCalledWith("export_game_plugin_data", { ids: ["test.game"] });
});
it("导出失败向调用页面保留原错误，不能伪造空包", async () => {
  const error = { code: "BACKUP_RELATION_INVALID", message: "关联不完整" };
  invoke.mockRejectedValue(error);
  await expect(backupApi.export({ kind: "session", ids: ["2"], includeSessions: false })).rejects.toEqual(error);
});

it("用户取消系统保存窗口时返回null", async () => {
  invoke.mockResolvedValue(null);
  await expect(backupApi.export({kind:"full",ids:[],includeSessions:false})).resolves.toBeNull();
});
it("导出选择读取真实对象列表，不传入数据库路径", async () => {
  await backupApi.options();
  expect(invoke).toHaveBeenCalledWith("get_backup_export_options");
});

it("导入经过后端选择和预览，确认只传关联决策", async () => {
  await backupApi.prepareImport(); expect(invoke).toHaveBeenCalledWith("prepare_backup_import");
  await backupApi.refreshImport("p"); expect(invoke).toHaveBeenCalledWith("refresh_backup_import", {previewId:"p"});
  const input={previewId:"p",personas:[{sourceId:"9007199254740993",targetId:null}],profiles:[],confirmGlobalRestore:false};
  await backupApi.confirmImport(input); expect(invoke).toHaveBeenCalledWith("confirm_backup_import",{input});
  await backupApi.cancelImport("p"); expect(invoke).toHaveBeenCalledWith("cancel_backup_import",{previewId:"p"});
});
