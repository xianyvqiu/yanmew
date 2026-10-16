import {act, cleanup, fireEvent, render, screen, waitFor} from "@testing-library/react";
import {afterEach, beforeEach, expect, it, vi} from "vitest";
import PluginManagerPanel from "./PluginManagerPanel";
import type {GamePlugin, GamePluginDetail, GameRun, GamePluginLedger} from "../types";

const api = vi.hoisted(() => ({list:vi.fn(), detail:vi.fn(), permission:vi.fn(), open:vi.fn(), listen:vi.fn(), history:vi.fn(), ledger:vi.fn(), previewInstall:vi.fn(), confirmInstall:vi.fn(), cancelInstall:vi.fn(), enable:vi.fn(), clearData:vi.fn(), uninstall:vi.fn(), setting:vi.fn()}));
vi.mock("../api", () => ({gamePluginsApi:api, walletApi:{get:async()=>({coins:"123",updatedAt:""}),listen:async()=>()=>{}}, webSearchApi:{open:vi.fn()},errorMessage:(cause:unknown)=>String(cause)}));
const plugin = (id="rock-paper-scissors"):GamePlugin => ({pluginId:`com.yanmew.game.${id}`,name:id==="my-home"?"我的家":"剪刀石头布",description:"游戏简介",author:"颜喵",installedVersion:"1.0.0",apiVersion:"1.0.0",storageVersion:1,packagePath:"test",packageSha256:"test",repository:"https://github.com/xianyvqiu/yanmew",homepage:null,releasePage:null,networkOrigins:[],isBundled:true,packageStatus:"available",isEnabled:true,installedAt:"",updatedAt:""});
const detail = (p:GamePlugin):GamePluginDetail => ({plugin:p,manifest:null,permissions:[{permission:"wallet.modify",scope:"",decision:"pending",pluginVersion:"1.0.0",decidedAt:null}],usage:{storage_version:1,settings:{},data:{}}});
beforeEach(()=>{
  vi.clearAllMocks();
  api.list.mockResolvedValue([plugin()]);api.detail.mockImplementation(async id=>detail(plugin(id.split(".").pop())));
  api.listen.mockResolvedValue(()=>{});api.permission.mockResolvedValue(undefined);api.open.mockResolvedValue("game-test");
  api.history.mockResolvedValue([]);api.ledger.mockResolvedValue([]);
  HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};
  HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
});
afterEach(cleanup);
it("首次钱包权限必须明确选择，拒绝后正常打开纯玩游戏",async()=>{
  render(<PluginManagerPanel/>);fireEvent.click(await screen.findByRole("button",{name:"打开 剪刀石头布"}));
  await screen.findByText("剪刀石头布 · 权限确认");
  expect(api.open).not.toHaveBeenCalled();expect((screen.getByRole("button",{name:"保存权限并打开"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(screen.getByLabelText("修改全局喵币（可增加或扣除）"),{target:{value:"denied"}});
  fireEvent.click(screen.getByRole("button",{name:"保存权限并打开"}));
  await waitFor(()=>expect(api.open).toHaveBeenCalledWith("com.yanmew.game.rock-paper-scissors"));
  expect(api.permission).toHaveBeenCalledWith("com.yanmew.game.rock-paper-scissors","1.0.0",{permission:"wallet.modify",scope:"",decision:"denied"});
});
it("我的家拒绝钱包权限时不能以纯玩打开",async()=>{
  api.list.mockResolvedValue([plugin("my-home")]);render(<PluginManagerPanel/>);
  fireEvent.click(await screen.findByRole("button",{name:"打开 我的家"}));await screen.findByText("我的家 · 权限确认");
  fireEvent.change(screen.getByLabelText("修改全局喵币（可增加或扣除）"),{target:{value:"denied"}});
  expect((screen.getByRole("button",{name:"保存权限并打开"}) as HTMLButtonElement).disabled).toBe(true);
  expect(api.open).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("修改全局喵币（可增加或扣除）"),{target:{value:"allowed"}});
  fireEvent.click(screen.getByRole("button",{name:"保存权限并打开"}));await waitFor(()=>expect(api.open).toHaveBeenCalledWith("com.yanmew.game.my-home"));
});
it("自带插件不能卸载，存档和来源可看见，清理前要求确认",async()=>{
  render(<PluginManagerPanel/>);fireEvent.click(await screen.findByRole("button",{name:"剪刀石头布"}));
  await screen.findByRole("button",{name:"GitHub 源码"});expect((screen.getByRole("button",{name:"卸载程序"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getByRole("button",{name:"清除数据并重载"}));
  expect(api.clearData).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"确认清除"}));
  await waitFor(()=>expect(api.clearData).toHaveBeenCalledWith("com.yanmew.game.rock-paper-scissors","1.0.0"));
  expect(api.open).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("修改全局喵币（可增加或扣除）"),{target:{value:"denied"}});
  fireEvent.click(screen.getByRole("button",{name:"重新授权并打开"}));
  await waitFor(()=>expect(api.open).toHaveBeenCalledWith("com.yanmew.game.rock-paper-scissors"));
});
it("本地包预览后才能安装，联网与钱包逐项确认且取消不安装",async()=>{
  const manifest={id:"org.test.game",name:"测试游戏",author:"作者",description:"测试",version:"1.0.0",repository:"https://github.com/example/game"};
  api.previewInstall.mockResolvedValue({token:"install-token",manifest,installed:null,migrationRequired:false,permissions:[{permission:"wallet.modify",scope:"",decision:"pending"},{permission:"network.fetch",scope:"https://example.com",decision:"pending"}]});
  render(<PluginManagerPanel/>);const file=new File(["zip"],"game.yanmew-game");Object.defineProperty(file,"arrayBuffer",{value:async()=>new Uint8Array([1,2,3]).buffer});
  fireEvent.change(screen.getByLabelText("安装本地小游戏插件包"),{target:{files:[file]}});await screen.findByText("安装 测试游戏");
  expect(api.confirmInstall).not.toHaveBeenCalled();expect((screen.getByRole("button",{name:"确认安装"}) as HTMLButtonElement).disabled).toBe(true);
  await act(async()=>fireEvent.click(screen.getByRole("button",{name:"取消"})));
  expect(api.cancelInstall).toHaveBeenCalledWith("install-token");expect(api.confirmInstall).not.toHaveBeenCalled();
});
it("游戏保存通知不会覆盖管理页尚未提交的权限选择",async()=>{
  let notify: (()=>void) | undefined;
  api.listen.mockImplementation(async receive => {notify=receive;return ()=>{};});
  render(<PluginManagerPanel/>);fireEvent.click(await screen.findByRole("button",{name:"剪刀石头布"}));
  await screen.findByRole("button",{name:"GitHub 源码"});
  fireEvent.change(screen.getByLabelText("修改全局喵币（可增加或扣除）"),{target:{value:"allowed"}});
  await act(async()=>{notify?.();});
  expect((screen.getByLabelText("修改全局喵币（可增加或扣除）") as HTMLSelectElement).value).toBe("allowed");
});
it("清空插件选择后丢弃迟到详情，不恢复旧插件操作入口", async () => {
  let finish!: (value: GamePluginDetail) => void;
  api.detail.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  render(<PluginManagerPanel />);
  fireEvent.click(await screen.findByRole("button", { name: "剪刀石头布" }));
  await waitFor(() => expect(api.detail).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: "返回小游戏列表" }));
  await act(async () => { finish(detail(plugin())); });
  expect(screen.queryByRole("heading", { name: "游戏存档" })).toBeNull();
  expect(screen.queryByRole("button", { name: "保存权限" })).toBeNull();
});
it("刷新分别保留展开的运行历史和账单，切换插件重新从首页读取", async () => {
  api.list.mockResolvedValue([plugin(), plugin("my-home")]);
  const runs: GameRun[] = Array.from({ length: 31 }, (_, i) => ({ id: String(100 - i), pluginId: plugin().pluginId,
    pluginName: "剪刀石头布", pluginVersion: "1.0.0", walletPermissionGranted: true, status: "completed", outcome: null,
    details: null, coinDelta: "1", startedAt: `运行时间${100 - i}`, endedAt: null }));
  const ledger: GamePluginLedger[] = runs.map(row => ({ id: row.id, gameRunId: row.id, pluginId: row.pluginId,
    pluginName: row.pluginName, pluginVersion: row.pluginVersion, delta: "1", reason: `账单原因${row.id}`, requestId: `req${row.id}`, createdAt: "now" }));
  api.history.mockImplementation(async (id, before) => id === plugin().pluginId ? runs.filter(row => !before || BigInt(row.id) < BigInt(before)).slice(0, 30) : []);
  api.ledger.mockImplementation(async (id, before) => id === plugin().pluginId ? ledger.filter(row => !before || BigInt(row.id) < BigInt(before)).slice(0, 30) : []);
  render(<PluginManagerPanel />);
  fireEvent.click(await screen.findByRole("button", { name: "剪刀石头布" }));
  fireEvent.click(await screen.findByRole("button", { name: "加载更早的运行" }));
  await screen.findByText(/运行时间70/);
  await act(async () => { fireEvent.focus(window); });
  expect(screen.getByText(/运行时间70/)).toBeTruthy();
  expect(screen.queryByText(/账单原因70/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "加载更早的账单" }));
  await screen.findByText(/账单原因70/);
  await act(async () => { fireEvent.focus(window); });
  expect(screen.getByText(/运行时间70/)).toBeTruthy();
  expect(screen.getByText(/账单原因70/)).toBeTruthy();
  api.history.mockClear(); api.ledger.mockClear();
  fireEvent.click(screen.getByRole("button", { name: "返回小游戏列表" }));
  await act(async () => { fireEvent.click(await screen.findByRole("button", { name: "我的家" })); });
  expect(api.history).toHaveBeenCalledWith(plugin("my-home").pluginId);
  expect(api.history).toHaveBeenCalledTimes(1);
  expect(screen.queryByText(/运行时间70/)).toBeNull();
});
function configuredDetail(value: { mode: string } | null): GamePluginDetail {
  const p = plugin();
  return { ...detail(p), manifest: { id: p.pluginId, name: p.name, description: p.description, author: p.author,
    version: p.installedVersion, api_version: "1.0.0", entry: "ui/index.html", storage_version: 1,
    permissions: [], network_origins: [], repository: p.repository, homepage: null, release_page: null, migration: null,
    settings_schema: [{ key: "mode", title: "游戏模式", description: "", type: "enum", required: false,
      minimum: null, maximum: null, default: { mode: "normal" }, options: [
        { value: { mode: "normal" }, label: "普通" }, { value: { mode: "hard" }, label: "困难" }, { value: null, label: "自动" },
      ] }], }, usage: { storage_version: 1, data: {}, settings: { mode: value } } };
}
it("对象枚举的未保存草稿不被同值刷新覆盖，保存仍提交实际对象", async () => {
  api.list.mockImplementation(async () => [plugin()]);
  api.detail.mockImplementation(async () => configuredDetail({ mode: "normal" }));
  render(<PluginManagerPanel />);
  fireEvent.click(await screen.findByRole("button", { name: "剪刀石头布" }));
  const field = await screen.findByLabelText("游戏模式");
  fireEvent.change(field, { target: { value: JSON.stringify({ mode: "hard" }) } });
  await act(async () => { fireEvent.focus(window); });
  expect((field as HTMLSelectElement).value).toBe(JSON.stringify({ mode: "hard" }));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "保存 游戏模式" })); });
  expect(api.setting).toHaveBeenCalledWith(plugin().pluginId, "1.0.0", "mode", { mode: "hard" });
});
it("合法null枚举设置显示已保存选项，不替换成默认对象", async () => {
  api.detail.mockImplementation(async () => configuredDetail(null));
  render(<PluginManagerPanel />);
  fireEvent.click(await screen.findByRole("button", { name: "剪刀石头布" }));
  const field = await screen.findByLabelText("游戏模式");
  expect((field as HTMLSelectElement).value).toBe("null");
});

it("同一张卡片同时提供启动和管理操作，停用和缺包不能启动",async()=>{
  api.list.mockResolvedValue([plugin(),{...plugin("my-home"),isEnabled:false}]);
  render(<PluginManagerPanel/>);
  await screen.findByRole("button",{name:"打开 剪刀石头布"});
  const card=screen.getByRole("button",{name:"剪刀石头布"}).closest("article")!;
  expect(card.textContent).toContain("清除数据并重载");expect(card.textContent).toContain("停用");expect(card.textContent).toContain("卸载");
  expect(screen.getByRole("button",{name:"打开 我的家"})).toHaveProperty("disabled",true);
  expect(screen.getByText("全局喵币 · 123")).toBeTruthy();
});
it("已有授权的游戏从同一卡片直接打开，不重复询问",async()=>{
  api.detail.mockResolvedValue({...detail(plugin()),permissions:[{...detail(plugin()).permissions[0],decision:"allowed"}]});
  render(<PluginManagerPanel/>);fireEvent.click(await screen.findByRole("button",{name:"打开 剪刀石头布"}));
  await waitFor(()=>expect(api.open).toHaveBeenCalledWith(plugin().pluginId));
  expect(screen.queryByRole("button",{name:"保存权限并打开"})).toBeNull();
});
