import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import CalendarView from "./CalendarView";
import WeatherCard,{weatherKind} from "./WeatherCard";
import ProvidersPanel from "./ProvidersPanel";
import PersonasPanel from "./PersonasPanel";
import PluginManagerPanel from "../PluginManagerPanel";
const api=vi.hoisted(()=>({listProviders:vi.fn(),listPersonas:vi.fn(),listenChanges:vi.fn(),listenContext:vi.fn(),listenExpressionFailure:vi.fn(),createProvider:vi.fn(),updateProvider:vi.fn(),roles:vi.fn(),saveRole:vi.fn(),updatePersona:vi.fn(),month:vi.fn(),listenDates:vi.fn(),detail:vi.fn(),history:vi.fn(),ledger:vi.fn()}));
vi.mock("../../api",()=>({configurationApi:api,festivalsApi:{month:api.month,listen:api.listenDates},gamePluginsApi:{detail:api.detail,history:api.history,ledger:api.ledger},webSearchApi:{open:vi.fn()},errorMessage:(e:{message:string})=>e.message}));
vi.mock("./EmbeddingProvidersPanel",()=>({default:()=>null}));
vi.mock("./WebSearchProvidersPanel",()=>({default:()=>null}));
vi.mock("../ChatBackgroundSettings",()=>({default:()=>null}));
vi.mock("../../hooks/useGamePlugins",()=>({useGamePlugins:()=>({plugins:[{pluginId:"built",name:"自带游戏",description:"小游戏",author:"作者",installedVersion:"1.0.0",installedAt:"2026-10-14T00:00:00Z",packageStatus:"available",isBundled:true,isEnabled:true},{pluginId:"community",name:"社区游戏",description:"小游戏",author:"作者",installedVersion:"1.0.0",installedAt:"2026-10-14T01:00:00Z",packageStatus:"available",isBundled:false,isEnabled:true}],loading:false,error:"",refresh:vi.fn()})}));
const provider={id:"1",name:"共享连接",baseUrl:"https://example.invalid/v1",model:"chat-model",hasApiKey:true,isActive:true,memoryIsActive:false,maxOutputTokens:32768,contextWindowOverride:null,capacity:{contextWindow:null,maxOutputTokens:null,source:"unknown",estimated:true},purposes:["chat"]};
const persona={id:"1",name:"电子小猫",prompt:"当前设定正文",initialPrompt:"初始设定正文",selfEvolveEnabled:true,lastEvolvedAt:null,color:"#ffb6c1",chatBgFile:null,chatBgOpacity:85,chatBgBlur:0,isActive:true,isBuiltin:true,expressions:null};
beforeEach(()=>{
  vi.resetAllMocks();api.listProviders.mockResolvedValue([provider]);api.listPersonas.mockResolvedValue([persona]);
  for(const fn of [api.listenChanges,api.listenContext,api.listenExpressionFailure,api.listenDates])fn.mockResolvedValue(()=>{});
  api.roles.mockResolvedValue({revision:3,roles:[{purpose:"chat",providerId:"1",model:"chat-model",enabled:true}]});
  api.createProvider.mockResolvedValue(provider);api.updatePersona.mockResolvedValue(persona);api.saveRole.mockResolvedValue({revision:4,roles:[]});
  HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
});
afterEach(()=>{cleanup();vi.useRealTimers();delete document.documentElement.dataset.theme;});
it("月历标出今天、农历和多个节日，点击后显示全部详情",async()=>{
  vi.useFakeTimers();vi.setSystemTime(new Date(2026,9,14,10));
  api.month.mockResolvedValue({year:2026,month:10,days:[{date:"2026-10-14",lunar:"初五",solarTerm:null,events:[{id:"1",scope:"catalog",name:"节日一",blessing:"祝福一"},{id:"2",scope:"custom",name:"纪念日",blessing:"祝福二"}]}]});
  const add=vi.fn(),edit=vi.fn();await act(async()=>{render(<CalendarView revision="1" onAdd={add} onEdit={edit}/>);});
  const day=screen.getByRole("button",{name:/2026-10-14 初五/});expect(day.getAttribute("data-today")).toBe("true");
  expect(within(day).getByText("初五")).toBeTruthy();
  expect(within(day).getByText("节日一 +1")).toBeTruthy();
  expect(screen.getByText("祝福二")).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"查看 / 编辑"}));expect(edit).toHaveBeenCalledWith("2");
  fireEvent.click(screen.getByRole("button",{name:"新增日期"}));expect(add).toHaveBeenCalledWith("2026-10-14");
});
it("新连接只填写连接信息，不再要求填写某一种用途的模型",async()=>{
  render(<ProvidersPanel/>);await screen.findByRole("button",{name:"共享连接"});fireEvent.click(screen.getByRole("button",{name:"新增提供商"}));
  expect(screen.queryByLabelText("模型名称")).toBeNull();fireEvent.change(screen.getByLabelText("提供商名称"),{target:{value:"新连接"}});fireEvent.click(screen.getByRole("button",{name:"保存"}));
  await waitFor(()=>expect(api.createProvider).toHaveBeenCalledWith({name:"新连接",baseUrl:null,apiKey:{action:"keep"}}));
});
it("记忆用途可以选择已添加连接并独立保存模型及生成预算",async()=>{
  render(<ProvidersPanel/>);fireEvent.click(await screen.findByRole("button",{name:"配置记忆"}));
  fireEvent.change(screen.getByLabelText("提供商"),{target:{value:"1"}});fireEvent.change(screen.getByLabelText("记忆模型"),{target:{value:"memory-model"}});
  fireEvent.change(screen.getByLabelText("最大输出（tokens）"),{target:{value:"65536"}});
  fireEvent.change(screen.getByLabelText("上下文容量（自动时留空）"),{target:{value:"128000"}});
  fireEvent.click(screen.getByRole("button",{name:"保存用途"}));
  await waitFor(()=>expect(api.saveRole).toHaveBeenCalledWith("memory","1","memory-model",false,3,65536,128000));
});
it("切换聊天连接恢复各自保存的模型和预算",async()=>{
  const other={...provider,id:"2",name:"另一个连接",isActive:false,model:"other-model",maxOutputTokens:4096,contextWindowOverride:128000};
  api.listProviders.mockResolvedValue([provider,other]);
  api.roles.mockResolvedValue({revision:3,roles:[{purpose:"chat",providerId:"1",model:"chat-model",enabled:true}],models:[{purpose:"chat",providerId:"1",model:"chat-model",maxOutputTokens:32768,contextWindowOverride:null},{purpose:"chat",providerId:"2",model:"other-model",maxOutputTokens:4096,contextWindowOverride:128000}]});
  render(<ProvidersPanel/>);fireEvent.click(await screen.findByRole("button",{name:"配置聊天"}));
  const select=screen.getByLabelText("提供商");
  fireEvent.change(select,{target:{value:"2"}});
  expect((screen.getByLabelText("聊天模型") as HTMLInputElement).value).toBe("other-model");
  expect((screen.getByLabelText("最大输出（tokens）") as HTMLInputElement).value).toBe("4096");
  expect((screen.getByLabelText("上下文容量（自动时留空）") as HTMLInputElement).value).toBe("128000");
  fireEvent.change(select,{target:{value:"1"}});
  expect((screen.getByLabelText("最大输出（tokens）") as HTMLInputElement).value).toBe("32768");
  expect((screen.getByLabelText("上下文容量（自动时留空）") as HTMLInputElement).value).toBe("");
});
it("人设列表仅显示摘要，进入详情后初始设定可以独立修改",async()=>{
  render(<PersonasPanel/>);const card=await screen.findByRole("button",{name:"电子小猫"});expect(screen.queryByRole("button",{name:"编辑 电子小猫"})).toBeNull();
  expect(screen.getByRole("button",{name:"使用中"}).getAttribute("disabled")).not.toBeNull();fireEvent.click(card);fireEvent.click(screen.getByRole("button",{name:"编辑 电子小猫"}));
  fireEvent.change(screen.getByLabelText("初始设定"),{target:{value:"新的初始"}});expect((screen.getByLabelText("当前设定") as HTMLTextAreaElement).value).toBe("当前设定正文");
  fireEvent.click(screen.getByRole("button",{name:"保存人设"}));await waitFor(()=>expect(api.updatePersona).toHaveBeenCalledWith("1",expect.objectContaining({initialPrompt:"新的初始",prompt:"当前设定正文",saveAsInitial:false})));
});
it("插件列表直接显示三个操作，自带卸载按钮灰色禁用",()=>{
  render(<PluginManagerPanel/>);const built=within(screen.getByRole("button",{name:"自带游戏"}).closest("article")!);expect((built.getByRole("button",{name:"卸载"}) as HTMLButtonElement).disabled).toBe(true);
  expect(built.getByRole("button",{name:"停用"})).toBeTruthy();expect(built.getByRole("button",{name:"清除数据并重载"})).toBeTruthy();
  const community=within(screen.getByRole("button",{name:"社区游戏"}).closest("article")!);expect((community.getByRole("button",{name:"卸载"}) as HTMLButtonElement).disabled).toBe(false);
});
it("天气随本地时刻切换昼夜，不修改应用主题",()=>{
  vi.useFakeTimers();vi.setSystemTime(new Date(2026,9,14,20));document.documentElement.dataset.theme="light";
  render(<WeatherCard weather={{availability:"unavailable",data:null,error:null,contextRevision:1}}/>);
  expect(screen.getByLabelText("当前天气卡片").getAttribute("data-period")).toBe("night");vi.setSystemTime(new Date(2026,9,15,9));act(()=>vi.advanceTimersByTime(30000));
  expect(screen.getByLabelText("当前天气卡片").getAttribute("data-period")).toBe("day");expect(document.documentElement.dataset.theme).toBe("light");expect(screen.getByText("暂无天气数据")).toBeTruthy();
});

it("天气缺少描述时显示未知，不把暂无数据判为晴天",()=>{
  expect(weatherKind("")).toBe("unknown");
  expect(weatherKind("  ")).toBe("unknown");
  expect(weatherKind("晴")).toBe("sun");
});
