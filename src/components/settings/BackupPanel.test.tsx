import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import BackupPanel from "./BackupPanel";
const api = vi.hoisted(() => ({ options: vi.fn(), save: vi.fn() }));
vi.mock("../../api", () => ({ backupApi: { options: api.options, export: api.save }, errorMessage: (e: {message:string}) => e.message }));
afterEach(cleanup);
beforeEach(() => {
  vi.resetAllMocks();
  api.options.mockResolvedValue({personas:[{id:"2",name:"电子小猫"}],sessions:[{id:"9007199254740993",name:"电子小猫 · 会话A"}],userProfiles:[{id:"10",name:"资料A"}],plugins:[{id:"test.game",name:"测试游戏"}]});
  api.save.mockResolvedValue({filename:"backup.zip",size:1024,manifest:{record_counts:{sessions:1,messages:8},files:[],warnings:[]}});
});
it("全量默认不读对象列表，通过后端系统窗口保存并展示摘要", async () => {
  render(<BackupPanel />);
  expect(api.options).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"选择位置并导出"}));
  await screen.findByText("备份已保存：backup.zip");
  expect(api.save).toHaveBeenCalledWith({kind:"full",ids:[],includeSessions:false});
  expect(screen.getByText(/8 条消息/)).toBeTruthy();
});
it("指定会话保留大ID字符串，未选择时不能提交", async () => {
  render(<BackupPanel />);
  fireEvent.change(screen.getByLabelText("备份范围"),{target:{value:"session"}});
  const item=await screen.findByRole("checkbox",{name:"电子小猫 · 会话A"});
  expect(screen.getByRole("button",{name:"选择位置并导出"}).hasAttribute("disabled")).toBe(true);
  fireEvent.click(item);fireEvent.click(screen.getByRole("button",{name:"选择位置并导出"}));
  await screen.findByText("备份已保存：backup.zip");
  expect(api.save).toHaveBeenCalledWith({kind:"session",ids:["9007199254740993"],includeSessions:false});
});
it("用户设定带会话开关只在该类型提交，切换范围清空旧选择", async () => {
  render(<BackupPanel />);
  fireEvent.change(screen.getByLabelText("备份范围"),{target:{value:"user_profile"}});
  fireEvent.click(await screen.findByRole("checkbox",{name:"资料A"}));
  fireEvent.click(screen.getByRole("checkbox",{name:/同时备份/}));
  fireEvent.click(screen.getByRole("button",{name:"选择位置并导出"}));
  await screen.findByText("备份已保存：backup.zip");
  expect(api.save).toHaveBeenCalledWith({kind:"user_profile",ids:["10"],includeSessions:true});
  fireEvent.change(screen.getByLabelText("备份范围"),{target:{value:"plugin_data"}});
  const game=await screen.findByRole("checkbox",{name:"测试游戏"});
  expect((game as HTMLInputElement).checked).toBe(false);
  expect(screen.queryByText("备份已保存：backup.zip")).toBeNull();
});
it("取消保存与失败都保留选择，不伪造成功", async () => {
  api.save.mockResolvedValueOnce(null).mockRejectedValueOnce({message:"磁盘写入失败"});
  render(<BackupPanel />);
  fireEvent.click(screen.getByRole("button",{name:"选择位置并导出"}));
  await screen.findByText(/已取消保存/);
  fireEvent.click(screen.getByRole("button",{name:"选择位置并导出"}));
  expect(await screen.findByRole("alert")).toHaveProperty("textContent","磁盘写入失败");
  expect(screen.queryByText("备份已保存：backup.zip")).toBeNull();
});
it("保存期间禁用再次提交，完成后展示缺图警告", async () => {
  let resolve!:(value:unknown)=>void;api.save.mockReturnValueOnce(new Promise(done=>{resolve=done;}));
  render(<BackupPanel />);fireEvent.click(screen.getByRole("button",{name:"选择位置并导出"}));
  expect(screen.getByRole("button",{name:"正在保存备份…"}).hasAttribute("disabled")).toBe(true);
  resolve({filename:"backup.zip",size:1024,manifest:{record_counts:{},files:[],warnings:[{message:"缺图，恢复默认背景"}]}});
  await waitFor(()=>expect(screen.getByText("缺图，恢复默认背景")).toBeTruthy());
});
