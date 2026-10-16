import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
const nav = vi.hoisted(() => ({ takeSettingsRequest: vi.fn(), listenSettings: vi.fn() }));
vi.mock("../platform/navigation", () => ({ navigation: nav }));
beforeEach(() => { nav.takeSettingsRequest.mockReset().mockResolvedValue(false); nav.listenSettings.mockReset().mockResolvedValue(() => {}); });

vi.mock("../components/settings/GeneralPanel", () => ({ default: () => <p>主题表单</p> }));
vi.mock("../components/settings/ProvidersPanel", () => ({ default: () => <p>提供商表单</p> }));
vi.mock("../components/settings/PersonasPanel", () => ({ default: ({detailRequest}:{detailRequest?:{id:string}}) => <p>人设表单{detailRequest&&<span data-testid="detail-id">{detailRequest.id}</span>}</p> }));
vi.mock("../components/settings/UserProfilePanel", () => ({ default: ({detailRequest}:{detailRequest?:{id:string}}) => <p>用户设定表单{detailRequest&&<span data-testid="detail-id">{detailRequest.id}</span>}</p> }));
vi.mock("../components/settings/ReminderPanel", () => ({ default: () => <p>提醒表单</p> }));
vi.mock("../components/settings/WeatherPanel", () => ({ default: ({ contextRevision }: { contextRevision: number }) => <p>天气表单：{contextRevision}</p> }));
vi.mock("../components/settings/PetTimingPanel", () => ({ default: () => <p>桌宠计时表单</p> }));
vi.mock("../components/settings/PetStatusPanel", () => ({ default: () => <p>桌宠状态内容</p> }));
vi.mock("../components/settings/MemorySettingsPanel", () => ({ default: () => <p>记忆设置表单</p> }));
vi.mock("../components/PluginManagerPanel", () => ({ default: () => <p>小游戏插件内容</p> }));
vi.mock("../components/settings/AccessoryPanel", () => ({ default: () => <p>饰品管理内容</p> }));
vi.mock("../components/TravelPanel", () => ({default:()=> <p>出行与邮箱内容</p>}));
vi.mock("../components/settings/FestivalPanel", () => ({ default: () => <p>节日管理内容</p> }));

afterEach(cleanup);
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
  fireEvent.click(screen.getByRole("button", { name: "节日" }));
  expect(screen.getByText("节日管理内容")).toBeTruthy();
  expect((screen.getByLabelText("设置页面") as HTMLSelectElement).value).toBe("节日");
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
});
