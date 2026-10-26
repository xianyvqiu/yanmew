import {act,cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import ProvidersPanel from "./ProvidersPanel";
import type {Provider} from "../../types";
vi.mock("./EmbeddingProvidersPanel",()=>({default:()=>null}));
vi.mock("./WebSearchProvidersPanel",()=>({default:()=>null}));
const api=vi.hoisted(()=>({listProviders:vi.fn(),listenChanges:vi.fn(),roles:vi.fn(),saveRole:vi.fn(),createProvider:vi.fn(),updateProvider:vi.fn(),deleteProvider:vi.fn(),testProvider:vi.fn()}));
vi.mock("../../api",()=>({configurationApi:api,errorMessage:(e:{message:string})=>e.message}));
const first:Provider={id:"1",name:"模型一",model:"chat-one",baseUrl:null,hasApiKey:true,isActive:true,memoryIsActive:false,maxOutputTokens:32768,contextWindowOverride:null,capacity:{contextWindow:null,maxOutputTokens:null,source:"unknown",estimated:true}};
const second:Provider={...first,id:"2",name:"模型二",model:"chat-two",isActive:false};
let rows:Provider[];
beforeEach(()=>{
  vi.resetAllMocks();rows=[first,second];api.listProviders.mockImplementation(async()=>[...rows]);api.listenChanges.mockResolvedValue(()=>{});
  api.roles.mockResolvedValue({revision:3,roles:[{purpose:"chat",providerId:"1",model:"chat-one",enabled:true}],models:[]});
  api.saveRole.mockResolvedValue({revision:4,roles:[],models:[]});api.createProvider.mockResolvedValue(second);api.updateProvider.mockResolvedValue(first);api.deleteProvider.mockResolvedValue(undefined);api.testProvider.mockResolvedValue(undefined);
  HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
});
afterEach(cleanup);
async function card(name:string){return within((await screen.findByRole("heading",{name})).closest("article")!);}
it("连接卡片直接显示完整信息与操作，无需进入详情",async()=>{
  render(<ProvidersPanel/>);const firstCard=await card("模型一");
  expect(firstCard.getByRole("button",{name:"编辑连接"})).toBeTruthy();
  expect(firstCard.getByRole("button",{name:"删除连接"})).toBeTruthy();
  expect(screen.getByRole("searchbox")).toBeTruthy();
  expect(screen.queryByRole("button",{name:"返回提供商列表"})).toBeNull();
});
it("新增仅提交连接信息，字段错误保留输入",async()=>{
  rows=[];api.createProvider.mockRejectedValue({code:"VALIDATION_ERROR",field:"name",message:"提供商名称无效"});
  render(<ProvidersPanel/>);await screen.findByText("暂无提供商，请先新增。");fireEvent.click(screen.getByRole("button",{name:"新增提供商"}));
  fireEvent.change(screen.getByLabelText("提供商名称"),{target:{value:"我的模型"}});
  expect(screen.queryByLabelText("模型名称")).toBeNull();fireEvent.click(screen.getByRole("button",{name:"保存"}));
  await screen.findByText("提供商名称无效");expect(screen.getByLabelText("提供商名称")).toHaveProperty("value","我的模型");
  expect(api.createProvider).toHaveBeenCalledWith({name:"我的模型",baseUrl:null,apiKey:{action:"keep"}});
});
it("正在使用的连接不能删除，其他连接确认后删除",async()=>{
  render(<ProvidersPanel/>);const firstCard=await card("模型一");fireEvent.click(firstCard.getByRole("button",{name:"删除连接"}));
  let modal=within(screen.getByRole("dialog"));expect(modal.getByText("此提供商正在使用")).toBeTruthy();expect(modal.queryByRole("button",{name:"确认删除"})).toBeNull();
  fireEvent.click(modal.getByRole("button",{name:"知道了"}));fireEvent.click((await card("模型二")).getByRole("button",{name:"删除连接"}));
  modal=within(screen.getByRole("dialog"));expect(api.deleteProvider).not.toHaveBeenCalled();rows=[first];fireEvent.click(modal.getByRole("button",{name:"确认删除"}));
  await waitFor(()=>expect(api.deleteProvider).toHaveBeenCalledWith("2"));await waitFor(()=>expect(screen.queryByRole("heading",{name:"模型二"})).toBeNull());
});
it("模型用途单独选择连接，记忆允许跟随聊天",async()=>{
  render(<ProvidersPanel/>);fireEvent.click(await screen.findByRole("button",{name:"配置记忆"}));fireEvent.change(screen.getByLabelText("提供商"),{target:{value:"2"}});fireEvent.change(screen.getByLabelText("记忆模型"),{target:{value:"memory-two"}});fireEvent.click(screen.getByRole("button",{name:"保存用途"}));
  await waitFor(()=>expect(api.saveRole).toHaveBeenCalledWith("memory","2","memory-two",false,3,32768,null));
  fireEvent.click(screen.getByRole("button",{name:"配置记忆"}));fireEvent.change(screen.getByLabelText("提供商"),{target:{value:""}});fireEvent.click(screen.getByRole("button",{name:"保存用途"}));
  await waitFor(()=>expect(api.saveRole).toHaveBeenLastCalledWith("memory",null,"",false,4,undefined,undefined));
});
it("测试使用已保存连接ID且等待期间禁止重复请求",async()=>{
  let finish!:()=>void;api.testProvider.mockImplementation(()=>new Promise<void>(resolve=>{finish=resolve;}));
  render(<ProvidersPanel/>);const firstCard=await card("模型一");const button=firstCard.getByRole("button",{name:"测试连接"});fireEvent.click(button);fireEvent.click(button);
  expect(api.testProvider).toHaveBeenCalledTimes(1);expect(api.testProvider).toHaveBeenCalledWith("1");await act(async()=>finish());await screen.findByText("模型一：连接成功（已保存配置）");
});
it("新增成功后的刷新失败不会再次提交或留下保存表单",async()=>{
  render(<ProvidersPanel/>);await screen.findByRole("heading",{name:"模型一"});fireEvent.click(screen.getByRole("button",{name:"新增提供商"}));fireEvent.change(screen.getByLabelText("提供商名称"),{target:{value:"新增"}});api.listProviders.mockRejectedValueOnce({message:"读取失败"});fireEvent.click(screen.getByRole("button",{name:"保存"}));
  await screen.findByText("读取失败");expect(screen.queryByRole("button",{name:"保存"})).toBeNull();expect(api.createProvider).toHaveBeenCalledTimes(1);
});
it("从连接列表打开编辑后会定位表单并聚焦名称，保留原连接内容",async()=>{
 const scroll=vi.fn();Object.defineProperty(HTMLElement.prototype,"scrollIntoView",{configurable:true,value:scroll});
 try{
  render(<ProvidersPanel/>);fireEvent.click((await card("模型二")).getByRole("button",{name:"编辑连接"}));
  const name=screen.getByLabelText("提供商名称");await waitFor(()=>expect(document.activeElement).toBe(name));
  expect(name).toHaveProperty("value","模型二");expect(scroll).toHaveBeenCalledWith({block:"start",behavior:"smooth"});
 }finally{delete (HTMLElement.prototype as Partial<HTMLElement>).scrollIntoView;}
});
it("纯生图连接也能点击测试连接，并明确没有生成图片",async()=>{
 rows=[{...first,model:"未配置",purposes:["image"]}];render(<ProvidersPanel/>);const button=(await card("模型一")).getByRole("button",{name:"测试连接"});
 expect(button).toHaveProperty("disabled",false);fireEvent.click(button);await waitFor(()=>expect(api.testProvider).toHaveBeenCalledWith("1"));expect(await screen.findByText(/未生成图片或验证透明输出/)).toBeTruthy();
});
