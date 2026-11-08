import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import PetCreationPanel from "./PetCreationPanel";
import type { PetArtwork, PetCreation } from "../../types";
const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn(), enable: vi.fn(), image: vi.fn(), upload: vi.fn(), finish: vi.fn(), generate: vi.fn(), cancel: vi.fn(), listen: vi.fn() }));
vi.mock("../../api", () => ({ petCreationApi: api, errorMessage: (e: Error) => e.message }));
const base: PetArtwork = { id: "1", personaId: "2", kind: "base", name: "初始形象", prompt: "", description: "默认", category: "normal", enabled: true, imageFile: "base.png", previewFile: null, revision: 0, createdAt: "2026-10-16T00:00:00Z", updatedAt: "2026-10-16T00:00:00Z" };
const expression: PetArtwork = { ...base, id: "2", kind: "expression", name: "挥手", prompt: "开心挥手", description: "欢迎时", category: "chat", imageFile: "formal.png", previewFile: "preview.png" };
let library: PetCreation;
beforeEach(() => { vi.resetAllMocks(); library = { enabled: false, revision: 3, items: [base, expression] }; api.get.mockImplementation(async () => library); api.listen.mockResolvedValue(() => { }); api.image.mockResolvedValue("data:image/png;base64,AQ=="); api.finish.mockResolvedValue(undefined); api.enable.mockResolvedValue(undefined); api.cancel.mockResolvedValue(undefined); });
afterEach(cleanup);
it("只有初始图或未接受的待机预览不能开启，并提供待机分类入口", async () => {
  render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByRole("button", { name: "添加待机表情" });
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
  expect(screen.getByText(/尚缺可用待机图/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "添加待机表情" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith("2", null, null, expect.objectContaining({ category: "standby", enabled: true, kind: "expression" })));
});
it("待机图尚未接受或已停用时不能开启", async () => {
  library.items.push({ ...expression, id: "3", category: "standby", imageFile: null, previewFile: "standby-preview.png", name: "待机预览" });
  const first = render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByRole("heading", { name: "待机预览" });
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
  first.unmount();
  library.items[2] = { ...library.items[2], imageFile: "standby.png", enabled: false };
  render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByRole("heading", { name: "待机预览" });
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(true);
});
it("旧版图片模式缺待机图时显示补图提示，仍允许主动关闭", async () => {
  library.enabled = true;
  render(<PetCreationPanel personaId="2" personaName="猫猫" />);
  await screen.findByText(/旧版已开启的图片模式保留/);
  expect((screen.getByRole("switch") as HTMLInputElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("switch"));
  await waitFor(() => expect(api.enable).toHaveBeenCalledWith("2", false, 3));
});

function card(name:string){return screen.getByRole("heading",{name,level:3}).closest("article")!;}
function open(name="挥手"){fireEvent.click(within(card(name)).getByRole("button",{name:"查看与操作"}));return screen.getByRole("dialog",{name:`${name} · 表情操作`});}
it("两列表情摘要只保留查看入口，编辑与预览在弹窗",async()=>{render(<PetCreationPanel personaId="2" personaName="猫猫"/>);await screen.findByText("挥手");expect(within(card("挥手")).queryByRole("button",{name:"重新生成"})).toBeNull();const dialog=open();expect(within(dialog).getByText("本次默认参考图")).toBeTruthy();fireEvent.click(within(dialog).getByRole("button",{name:"编辑要求与说明"}));expect(within(dialog).getByLabelText("具体动作与画面要求")).toBeTruthy();fireEvent.click(within(dialog).getByRole("button",{name:"取消修改"}));fireEvent.click(within(dialog).getByRole("button",{name:"接受预览"}));await waitFor(()=>expect(api.finish).toHaveBeenCalledWith("2","2",0,"accept"));expect(api.generate).not.toHaveBeenCalled();});
it("新表情分项填写且不自动收费生成",async()=>{api.save.mockImplementation(async(_p,_id,_r,input)=>{const value={...expression,...input,id:"3"};library={...library,items:[...library.items,value]};return value;});render(<PetCreationPanel personaId="2" personaName="猫猫"/>);await screen.findByText("挥手");expect(screen.queryByLabelText("表情名称")).toBeNull();fireEvent.click(screen.getByRole("button",{name:"创建新表情"}));fireEvent.change(screen.getByLabelText("表情名称"),{target:{value:"新的动作"}});fireEvent.change(screen.getByLabelText("具体动作与画面要求"),{target:{value:"双手轻轻挥动"}});fireEvent.click(screen.getByRole("button",{name:"添加表情"}));await screen.findByRole("heading",{name:"新的动作"});expect(api.save).toHaveBeenCalledWith("2",null,null,expect.objectContaining({name:"新的动作",prompt:"双手轻轻挥动"}));expect(api.generate).not.toHaveBeenCalled();});
it("生成进度在弹窗和对应卡片，关闭弹窗后失败保留原图和预览",async()=>{let reject!:(e:Error)=>void;api.generate.mockImplementation(()=>new Promise((_r,j)=>{reject=j;}));render(<PetCreationPanel personaId="2" personaName="猫猫"/>);await screen.findByText("挥手");const dialog=open();fireEvent.click(within(dialog).getByRole("button",{name:"重新生成"}));await within(card("挥手")).findByText(/正在生成：/);expect(within(dialog).getByText(/正在生成：/)).toBeTruthy();fireEvent.click(within(dialog).getByRole("button",{name:"关闭表情操作"}));expect(screen.queryByRole("dialog")).toBeNull();reject(new Error("服务错误 HTTP 400"));await within(card("挥手")).findByRole("alert");expect(library.items[1].imageFile).toBe("formal.png");expect(library.items[1].previewFile).toBe("preview.png");expect(api.finish).not.toHaveBeenCalled();});
it("批量逐张生成，停止当前项后不请求剩余项",async()=>{library.items.push({...expression,id:"3",name:"跳跃"});let reject!:(e:Error)=>void;api.generate.mockImplementation(()=>new Promise((_r,j)=>{reject=j;}));render(<PetCreationPanel personaId="2" personaName="猫猫"/>);await screen.findByText("挥手");for(const cb of screen.getAllByRole("checkbox",{name:"加入批量生成"}))fireEvent.click(cb);fireEvent.click(screen.getByRole("button",{name:"生成选中表情 · 2 项"}));await within(card("挥手")).findByText(/正在生成：/);fireEvent.click(within(card("挥手")).getByRole("button",{name:"停止生成"}));await waitFor(()=>expect(api.cancel).toHaveBeenCalledWith("2","2"));reject(new Error("已取消"));await within(card("挥手")).findByText("已取消");expect(api.generate).toHaveBeenCalledTimes(1);expect(screen.getByText("已停止，尚未请求生图服务。")).toBeTruthy();});
it("卸载仍取消生成，接受默认预览后进入准备方式",async()=>{library.items[0]={...base,previewFile:"new-base.png"};const setup=vi.fn();const view=render(<PetCreationPanel personaId="2" personaName="猫猫" onStartSetup={setup}/>);await screen.findByText("挥手");const dialog=open("初始形象");fireEvent.click(within(dialog).getByRole("button",{name:"接受预览"}));await waitFor(()=>expect(setup).toHaveBeenCalledTimes(1));fireEvent.click(within(dialog).getByRole("button",{name:"关闭表情操作"}));api.generate.mockImplementation(()=>new Promise(()=>{}));fireEvent.click(within(open()).getByRole("button",{name:"重新生成"}));await within(card("挥手")).findByText(/正在生成：/);view.unmount();expect(api.cancel).toHaveBeenCalledWith("2","2");});
