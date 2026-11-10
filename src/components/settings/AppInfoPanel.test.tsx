import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import AppInfoPanel from "./AppInfoPanel";
const api = vi.hoisted(() => ({info:vi.fn(),exit:vi.fn(),updateStatus:vi.fn(),checkUpdate:vi.fn(),update:vi.fn(),openInformation:vi.fn(),uninstall:vi.fn()}));
vi.mock("../../api",()=>({appApi:api,errorMessage:(error:Error)=>error.message}));
const info={name:"颜喵 YanMew",version:"0.1.0",buildId:"20261110-170000",mode:"portable",dataDirectory:"D:/颜喵/data",programDirectory:"D:/颜喵"};
const empty={phase:"no_release",message:"暂无可公开访问的正式发布版本",checkedAt:"2026-11-10T08:00:00Z",downloaded:0,total:0,candidate:null};
const available={...empty,phase:"available",message:"发现新构建",candidate:{manifest:{version:"0.1.0",buildId:"20261111-170000",assetName:"portable.zip",size:100,sha256:"a".repeat(64),schemaVersion:1},releaseUrl:"https://github.com/xianyvqiu/yanmew/releases/tag/test",notes:"修复收藏刷新"}};
beforeEach(()=>{
 vi.resetAllMocks();api.info.mockResolvedValue(info);api.updateStatus.mockResolvedValue(empty);api.checkUpdate.mockResolvedValue(empty);api.update.mockResolvedValue(undefined);api.exit.mockResolvedValue(undefined);api.openInformation.mockResolvedValue(undefined);api.uninstall.mockResolvedValue(undefined);
 HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
});
afterEach(cleanup);
it("实际版本保持0.1.0并显示构建编号，无Release不误报最新版或自动下载",async()=>{
 await act(async()=>{render(<AppInfoPanel/>);});
 expect(screen.getByText("0.1.0")).toBeTruthy();expect(screen.getByText(info.buildId)).toBeTruthy();expect(screen.getByText(info.dataDirectory)).toBeTruthy();
 expect(screen.getByText(empty.message)).toBeTruthy();expect(api.checkUpdate).not.toHaveBeenCalled();expect(api.update).not.toHaveBeenCalled();expect(screen.queryByRole("button",{name:"更新并重新启动"})).toBeNull();
});
it("退出请求连点只发送一次",async()=>{
 api.exit.mockReturnValue(new Promise(()=>{}));await act(async()=>{render(<AppInfoPanel/>);});const exit=screen.getByRole("button",{name:"退出应用"});fireEvent.click(exit);fireEvent.click(exit);expect(api.exit).toHaveBeenCalledTimes(1);expect(exit).toHaveProperty("disabled",true);
});
it("版本读取失败可重试，退出失败仍可再次操作",async()=>{
 api.info.mockRejectedValueOnce(new Error("信息读取失败")).mockResolvedValue(info);api.exit.mockRejectedValueOnce(new Error("退出请求失败")).mockResolvedValue(undefined);
 await act(async()=>{render(<AppInfoPanel/>);});fireEvent.click(screen.getByRole("button",{name:"重新读取信息"}));await screen.findByText("0.1.0");
 fireEvent.click(screen.getByRole("button",{name:"退出应用"}));await screen.findByText("退出请求失败");fireEvent.click(screen.getByRole("button",{name:"退出应用"}));await waitFor(()=>expect(api.exit).toHaveBeenCalledTimes(2));
});
it("手动检查失败显示实际错误，原程序不退出也不下载",async()=>{
 api.checkUpdate.mockResolvedValue({...empty,phase:"error",message:"检查更新失败，请检查网络或代理后重试"});await act(async()=>{render(<AppInfoPanel/>);});fireEvent.click(screen.getByRole("button",{name:"检查更新"}));await screen.findByText("检查更新失败，请检查网络或代理后重试");expect(api.update).not.toHaveBeenCalled();expect(api.exit).not.toHaveBeenCalled();
});
it("新构建可一键更新且防连点，失败后可重试",async()=>{
 api.updateStatus.mockResolvedValue(available);let reject!:(cause:Error)=>void;api.update.mockImplementationOnce(()=>new Promise((_,fail)=>{reject=fail;}));await act(async()=>{render(<AppInfoPanel/>);});
 const button=screen.getByRole("button",{name:"更新并重新启动"});fireEvent.click(button);fireEvent.click(button);expect(api.update).toHaveBeenCalledTimes(1);expect(button).toHaveProperty("disabled",true);
 await act(async()=>{reject(new Error("校验失败，旧程序未修改"));});await screen.findByText("校验失败，旧程序未修改");fireEvent.click(button);await waitFor(()=>expect(api.update).toHaveBeenCalledTimes(2));expect(api.uninstall).not.toHaveBeenCalled();
});
it("卸载必须确认永久删除，取消不请求，确认防连点",async()=>{
 await act(async()=>{render(<AppInfoPanel/>);});fireEvent.click(screen.getByRole("button",{name:"卸载颜喵"}));const dialog=screen.getByRole("dialog");expect(within(dialog).getByText(/无法恢复/)).toBeTruthy();fireEvent.click(within(dialog).getByRole("button",{name:"取消"}));expect(api.uninstall).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"卸载颜喵"}));api.uninstall.mockReturnValue(new Promise(()=>{}));const confirm=screen.getByRole("button",{name:"确认卸载并删除资料"});fireEvent.click(confirm);fireEvent.click(confirm);expect(api.uninstall).toHaveBeenCalledTimes(1);
});
it("仓库和资料入口使用固定类型，不传任意路径或URL给后端",async()=>{
 await act(async()=>{render(<AppInfoPanel/>);});fireEvent.click(screen.getByRole("link",{name:"主项目仓库"}));await waitFor(()=>expect(api.openInformation).toHaveBeenCalledWith("repository"));fireEvent.click(screen.getByRole("button",{name:"打开资料目录"}));await waitFor(()=>expect(api.openInformation).toHaveBeenCalledWith("data"));
});
