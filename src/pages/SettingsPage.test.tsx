import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
const nav = vi.hoisted(() => ({ takeSettingsRequest: vi.fn(), listenSettings: vi.fn() }));
vi.mock("../platform/navigation", () => ({ navigation: nav }));
beforeEach(() => { nav.takeSettingsRequest.mockReset().mockResolvedValue(false); nav.listenSettings.mockReset().mockResolvedValue(() => {}); });
function openNavigation() {
  for (const name of ["全局设置", "基础服务", "扩展服务", "人设与会话设置"]) fireEvent.click(screen.getByRole("button", { name }));
}

vi.mock("../components/settings/AppInfoPanel",()=>({default:()=> <p>关于颜喵独立内容</p>}));
vi.mock("../components/settings/GeneralPanel", () => ({ default: () => <p>主题表单</p> }));
vi.mock("../components/settings/ProvidersPanel", () => ({ default: () => <p>提供商表单</p> }));
vi.mock("../components/settings/PersonasPanel", () => ({ default: ({detailRequest}:{detailRequest?:{id:string}}) => <p>人设表单{detailRequest&&<span data-testid="detail-id">{detailRequest.id}</span>}</p> }));
vi.mock("../components/settings/UserProfilePanel", () => ({ default: ({detailRequest}:{detailRequest?:{id:string}}) => <p>用户设定表单{detailRequest&&<span data-testid="detail-id">{detailRequest.id}</span>}</p> }));
vi.mock("../components/settings/ReminderPanel", () => ({ default: () => <p>提醒表单</p> }));
vi.mock("../components/settings/WeatherPanel", () => ({ default: ({ contextRevision }: { contextRevision: number }) => <p>天气表单：{contextRevision}</p> }));
vi.mock("../components/settings/PetTimingPanel", () => ({ default: () => <p>桌宠计时表单</p> }));
vi.mock("../components/settings/PetStatusPanel", () => ({ default: () => <p>桌宠状态内容</p> }));
vi.mock("../components/settings/MemorySettingsPanel", () => ({ default: () => <p>记忆设置表单</p> }));
vi.mock("../components/settings/StickerLibraryPanel",()=>({default:({requestedId}:{requestedId?:string})=><p>表情包库详情：{requestedId}</p>}));
vi.mock("../components/PluginManagerPanel", () => ({ default: () => <p>小游戏插件内容</p> }));
vi.mock("../components/settings/AccessoryPanel", () => ({ default: () => <p>饰品管理内容</p> }));
vi.mock("../components/TravelPanel", () => ({default:()=> <p>出行与邮箱内容</p>}));
vi.mock("../components/settings/FestivalPanel", () => ({ default: () => <p>节日管理内容</p> }));

afterEach(cleanup);
it("人设关联卡片的详情事件切到表情包页并携带具体库ID",async()=>{
  render(<SettingsPage snapshot={null} loading={false} error="" onRefresh={()=>{}}/>);
  act(()=>{window.dispatchEvent(new CustomEvent("ym-open-sticker-library",{detail:"library-7"}));});
  expect(await screen.findByText("表情包库详情：library-7")).toBeTruthy();
});
it("人设页在切换上下文的空快照期间保留DOM、局部输入和滚动位置", async()=>{
 const snapshot:WorkspaceContext={context:{personaId:"1",sessionId:"1",contextRevision:1},sessions:[],userProfiles:[],persona:{id:"1",name:"小猫",prompt:"",initialPrompt:"",selfEvolveEnabled:false,lastEvolvedAt:null,color:"#123456",chatBgFile:null,chatBgOpacity:85,chatBgBlur:0,isActive:true,isBuiltin:true,expressions:null}};
 let view:ReturnType<typeof render>;
 await act(async()=>{view=render(<SettingsPage snapshot={snapshot} loading={false} error="" onRefresh={()=>{}}/>);});
 openNavigation();
 const main=screen.getByRole("main");main.scrollTop=360;
 fireEvent.click(screen.getByRole("button",{name:"人设"}));
 expect(main.scrollTop).toBe(0);
 const panel=screen.getByText("人设表单"),content=screen.getByRole("main");
 const input=document.createElement("textarea");input.value="未保存的输入";panel.append(input);content.scrollTop=480;
 view!.rerender(<SettingsPage snapshot={null} loading={true} error="" onRefresh={()=>{}}/>);
 expect(screen.getByText("人设表单")).toBe(panel);expect(input.isConnected).toBe(true);expect(content.scrollTop).toBe(480);
 view!.rerender(<SettingsPage snapshot={{...snapshot,context:{...snapshot.context,personaId:"2",sessionId:"2",contextRevision:2}}} loading={false} error="" onRefresh={()=>{}}/>);
 expect(screen.getByText("人设表单")).toBe(panel);expect(input.value).toBe("未保存的输入");expect(content.scrollTop).toBe(480);
 fireEvent.click(screen.getByRole("button",{name:"人设"}));expect(content.scrollTop).toBe(480);
 fireEvent.click(screen.getByRole("button",{name:"出行"}));
 expect(content.scrollTop).toBe(0);
 view!.rerender(<SettingsPage snapshot={null} loading={true} error="" onRefresh={()=>{}}/>);
 expect(screen.queryByText("出行与邮箱内容")).toBeNull();
});
it("设置页加载前收到的详情目标保留到数据就绪，并把ID传给正确面板", async () => {
  const snapshot:WorkspaceContext={context:{personaId:"1",sessionId:null,contextRevision:1},sessions:[],userProfiles:[],persona:{id:"1",name:"小猫",prompt:"",initialPrompt:"",selfEvolveEnabled:false,lastEvolvedAt:null,color:"#123456",chatBgFile:null,chatBgOpacity:85,chatBgBlur:0,isActive:true,isBuiltin:true,expressions:null}};
  for (const page of ["persona","userProfile"] as const) {
    nav.takeSettingsRequest.mockResolvedValueOnce({page,id:"7"});
    let view:ReturnType<typeof render>;
    await act(async()=>{view=render(<SettingsPage snapshot={null} loading={true} error="" onRefresh={()=>{}} />);});
    view!.rerender(<SettingsPage snapshot={snapshot} loading={false} error="" onRefresh={()=>{}} />);
    expect(screen.getByTestId("detail-id").textContent).toBe("7");
    expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe(page==="persona"?"人设":"用户设定");
    view!.unmount();
  }
});
it("设置窗就绪后消费提醒导航，即使上下文尚未加载也保留目标", async () => {
  nav.takeSettingsRequest.mockResolvedValueOnce({ page: "reminders" });
  await act(async () => { render(<SettingsPage snapshot={null} loading={true} error="" onRefresh={() => {}} />); });
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("提醒");
});
it("桌面导航和窄窗选择器同步切换已开放设置", async () => {
  const snapshot: WorkspaceContext = { context: { personaId: "1", sessionId: null, contextRevision: 0 }, sessions: [], userProfiles: [], persona: { id: "1", name: "颜喵", prompt: "", initialPrompt: "", selfEvolveEnabled: false, lastEvolvedAt: null, color: "#777777", chatBgFile: null, chatBgOpacity: 85, chatBgBlur: 0, isActive: true, isBuiltin: true, expressions: null } };
  await act(async () => { render(<SettingsPage snapshot={snapshot} loading={false} error="" onRefresh={() => {}} />); });
  expect(screen.getByText("主题表单")).toBeTruthy();
  openNavigation();
  fireEvent.click(screen.getByRole("button", { name: "提供商" }));
  expect(screen.getByText("提供商表单")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("提供商");
  fireEvent.change(screen.getByLabelText("设置页面"), { target: { value: "通用" } });
  expect(screen.getByText("主题表单")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "人设" }));
  expect(screen.getByText("人设表单")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "用户设定" }));
  expect(screen.getByText("用户设定表单")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("设置页面"), { target: { value: "用户设定" } });
  expect(screen.getByText("用户设定表单")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "提醒" }));
  expect(screen.getByText("提醒表单")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("提醒");
  expect(screen.queryByRole("button", { name: "游戏中心" })).toBeNull();
  expect(screen.queryByRole("option", { name: "游戏中心" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "日历" }));
  expect(screen.getByText("节日管理内容")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("日历");
  fireEvent.click(screen.getByRole("button", { name: "出行" }));
  expect(screen.getByText("出行与邮箱内容")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("出行");
  fireEvent.click(screen.getByRole("button", { name: "饰品" }));
  expect(screen.getByText("饰品管理内容")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("饰品");
  fireEvent.click(screen.getByRole("button", { name: "小游戏插件" }));
  expect(screen.getByText("小游戏插件内容")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("设置页面"), { target: { value: "时间与天气" } });
  expect(screen.getByText("天气表单：0")).toBeTruthy();
  expect(screen.getByRole("button", { name: "时间与天气" }).getAttribute("aria-current")).toBe("page");
  fireEvent.click(screen.getByRole("button", { name: "时间与桌宠" }));
  const timingEntry=screen.getByRole("button",{name:"时间与桌宠"});
  expect(screen.getByRole("button",{name:"基础服务"}).closest("section")?.contains(timingEntry)).toBe(true);
  expect(screen.getByRole("button",{name:"人设与会话设置"}).closest("section")?.contains(timingEntry)).toBe(false);
  expect(screen.getByText("桌宠计时表单")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("时间与桌宠");
  fireEvent.change(screen.getByLabelText("设置页面"), { target: { value: "记忆" } });
  expect(screen.getByText("记忆设置表单")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "桌宠状态" }));
  expect(screen.getByText("桌宠状态内容")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("桌宠状态");
  fireEvent.change(screen.getByLabelText("设置页面"), { target: { value: "通用" } });
  fireEvent.change(screen.getByLabelText("设置页面"), { target: { value: "桌宠状态" } });
  expect(screen.getByText("桌宠状态内容")).toBeTruthy();
});

it("饰品快捷导航在设置窗加载时恢复目标页面", async () => {
  nav.takeSettingsRequest.mockResolvedValueOnce({ page: "accessories" });
  await act(async () => { render(<SettingsPage snapshot={null} loading={true} error="" onRefresh={() => {}} />); });
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("饰品");
  expect(screen.getByRole("button", { name: "人设与会话设置" }).getAttribute("aria-expanded")).toBe("true");
  expect(screen.getByRole("button", { name: "饰品" }).getAttribute("aria-current")).toBe("page");
});

it("侧栏初始收起，基础服务在前且包含Skill，收起分组不卸载当前页面", async () => {
  await act(async () => { render(<SettingsPage snapshot={null} loading={false} error="" onRefresh={() => {}} />); });
  expect(screen.getByRole("button", { name: "全局设置" }).getAttribute("aria-expanded")).toBe("false");
  expect(screen.getByRole("button", { name: "人设与会话设置" }).getAttribute("aria-expanded")).toBe("false");
  expect(screen.queryByRole("button", { name: "基础服务" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "全局设置" }));
  const base = screen.getByRole("button", { name: "基础服务" }), extra = screen.getByRole("button", { name: "扩展服务" });
  expect(base.getAttribute("aria-expanded")).toBe("false");
  expect(extra.getAttribute("aria-expanded")).toBe("false");
  expect(base.compareDocumentPosition(extra) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  fireEvent.click(base);
  expect(screen.getByRole("button", { name: "Skill 仓库" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "食物" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "人设" }));
  const panel = screen.getByText("人设表单");
  const input = document.createElement("textarea"); input.value = "草稿"; panel.append(input);
  fireEvent.click(base);
  expect(screen.queryByRole("button", { name: "人设" })).toBeNull();
  expect(input.isConnected).toBe(true);
  fireEvent.click(base);
  expect(screen.getByRole("button", { name: "人设" }).getAttribute("aria-current")).toBe("page");
  expect(input.value).toBe("草稿");
  fireEvent.click(extra);
  expect(screen.getByRole("button", { name: "食物" })).toBeTruthy();
});

it("上下文加载失败时仍能打开手册，窄窗选择器保留入口", async () => {
  await act(async () => { render(<SettingsPage snapshot={null} loading={false} error="读取失败" onRefresh={() => {}} />); });
  fireEvent.click(screen.getByRole("button", {name:"帮助与使用手册"}));
  expect(screen.getByRole("heading", {name:"帮助与使用手册"})).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("帮助与使用手册");
  fireEvent.click(screen.getByText(/^第一次使用：/));
  expect(screen.getByText(/先配好聊天即可/)).toBeTruthy();
});

it("关于颜喵是独立入口，不依赖当前会话加载成功",async()=>{
 await act(async()=>{render(<SettingsPage snapshot={null} loading={true} error="上下文读取失败" onRefresh={()=>{}}/>);});fireEvent.click(screen.getByRole("button",{name:"关于颜喵"}));expect(screen.getByText("关于颜喵独立内容")).toBeTruthy();expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("关于颜喵");
});
