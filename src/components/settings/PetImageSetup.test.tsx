import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import PetImageSetup from "./PetImageSetup";
import type {PetArtwork,PetCreation} from "../../types";
import {basicPetImages} from "./petImageSetupTemplates";
const api=vi.hoisted(()=>({get:vi.fn(),save:vi.fn(),upload:vi.fn(),finish:vi.fn(),generate:vi.fn(),cancel:vi.fn(),enable:vi.fn(),config:vi.fn()}));
vi.mock("../../api",()=>({petCreationApi:api,imageGenerationApi:{get:api.config},errorMessage:(e:Error)=>e.message}));
vi.mock("./PetCreationPanel",()=>({PetArtworkImage:({item}:{item:PetArtwork})=><span>{item.name}图片</span>}));
let library:PetCreation;
const update=(id:string,change:Partial<PetArtwork>)=>{const old=library.items.find(a=>a.id===id)!;const item={...old,...change,revision:old.revision+1};library={...library,revision:library.revision+1,items:library.items.map(a=>a.id===id?item:a)};return item;};
const preview=async(_p:string,id:string)=>update(id,{previewFile:`${id}-preview.png`});
beforeEach(()=>{vi.resetAllMocks();library={enabled:false,revision:0,items:[]};api.get.mockImplementation(async()=>library);api.config.mockResolvedValue({enabled:true,activeId:"1"});api.save.mockImplementation(async(_p,_id,_rev,input)=>{const item={...input,id:String(library.items.length+1),personaId:"2",revision:0,imageFile:null,previewFile:null,createdAt:"a",updatedAt:"a"};library={...library,revision:library.revision+1,items:[...library.items,item]};return item;});api.upload.mockImplementation(preview);api.generate.mockImplementation(preview);api.finish.mockImplementation(async(_p,id)=>{const item=library.items.find(a=>a.id===id)!;update(id,{imageFile:item.previewFile,previewFile:null});});api.cancel.mockResolvedValue(undefined);api.enable.mockResolvedValue(undefined);});
afterEach(cleanup);
async function chooseGenerate(){const button=await screen.findByRole("button",{name:/一次性生成所有基础表情/});await waitFor(()=>expect((button as HTMLButtonElement).disabled).toBe(false));fireEvent.click(button);}
it("选择生成后准备全部基础预览，生成期间锁定编辑，用户确认前不接受表情",async()=>{
 let release!:()=>void;api.generate.mockImplementationOnce((_p,id)=>new Promise(resolve=>{release=()=>void preview(_p,id).then(resolve);}));
 render(<PetImageSetup personaId="2" personaName="新伙伴" onClose={vi.fn()}/>);await waitFor(()=>expect((screen.getByLabelText("上传初始图片") as HTMLInputElement).disabled).toBe(false));
 fireEvent.change(screen.getByLabelText("上传初始图片"),{target:{files:[new File(["png"],"base.png",{type:"image/png"})]}});
 await chooseGenerate();await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(screen.getByText("正在准备表情，请稍候")).toBeTruthy();expect((screen.getByLabelText("上传初始图片") as HTMLInputElement).closest("fieldset")?.disabled).toBe(true);
 expect(api.finish).toHaveBeenCalledTimes(1);await act(async()=>release());
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(basicPetImages.length));await waitFor(()=>expect(screen.queryByText("正在准备表情，请稍候")).toBeNull());
 expect(api.finish).toHaveBeenCalledTimes(1);expect(api.enable).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"接受 待机"})).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"接受全部预览并启用图片模式"}));await waitFor(()=>expect(api.enable).toHaveBeenCalledWith("2",true,expect.any(Number)));expect(api.finish).toHaveBeenCalledTimes(31);
});
it("服务失败保留已成功预览，可单独重试且不自动覆盖正式图",async()=>{
 api.generate.mockImplementationOnce(preview).mockRejectedValueOnce(new Error("参考图接口失败"));
 render(<PetImageSetup personaId="2" personaName="新伙伴" onClose={vi.fn()}/>);await waitFor(()=>expect(api.get).toHaveBeenCalled());
 fireEvent.change(screen.getByLabelText("上传初始图片"),{target:{files:[new File(["png"],"base.png")]}});
 await chooseGenerate();expect(await screen.findByRole("alert")).toBeTruthy();expect(screen.getByRole("button",{name:"接受 待机"})).toBeTruthy();expect(api.generate).toHaveBeenCalledTimes(2);
 fireEvent.click(screen.getByRole("button",{name:"重试 点击开心"}));await screen.findByRole("button",{name:"接受 点击开心"});expect(api.generate).toHaveBeenCalledTimes(3);expect(api.finish).toHaveBeenCalledTimes(1);
});
it("关闭初始化页面会取消当前生成且不继续付费队列",async()=>{
 let release!:()=>void;api.generate.mockImplementationOnce((_p,id)=>new Promise(resolve=>{release=()=>void preview(_p,id).then(resolve);}));
 const view=render(<PetImageSetup personaId="2" personaName="新伙伴" onClose={vi.fn()}/>);await waitFor(()=>expect(api.get).toHaveBeenCalled());fireEvent.change(screen.getByLabelText("上传初始图片"),{target:{files:[new File(["png"],"base.png")]}});
 await chooseGenerate();await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));view.unmount();expect(api.cancel).toHaveBeenCalledWith("2","2");await act(async()=>release());expect(api.generate).toHaveBeenCalledTimes(1);expect(api.enable).not.toHaveBeenCalled();
});

it("初始图上传后先选择，手动分支建立全部占位且不查询或调用生图",async()=>{
 render(<PetImageSetup personaId="2" personaName="新伙伴" onClose={vi.fn()}/>);await waitFor(()=>expect(api.get).toHaveBeenCalled());
 fireEvent.change(screen.getByLabelText("上传初始图片"),{target:{files:[new File(["png"],"base.png")]}});
 const manual=await screen.findByRole("button",{name:/自己上传表情/});await waitFor(()=>expect((manual as HTMLButtonElement).disabled).toBe(false));expect(api.generate).not.toHaveBeenCalled();expect(api.config).not.toHaveBeenCalled();
 fireEvent.click(manual);await waitFor(()=>expect(library.items).toHaveLength(basicPetImages.length+1));await waitFor(()=>expect(screen.queryByText("正在准备表情，请稍候")).toBeNull());
 expect(screen.getAllByText(/^待上传 · /)).toHaveLength(basicPetImages.length);expect(api.config).not.toHaveBeenCalled();expect(api.generate).not.toHaveBeenCalled();
 fireEvent.change(screen.getByLabelText("上传 待机 图片"),{target:{files:[new File(["png"],"standby.png")]}});await screen.findByRole("button",{name:"接受 待机"});expect(api.generate).not.toHaveBeenCalled();expect(api.config).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"接受全部预览并启用图片模式"}));await waitFor(()=>expect(api.enable).toHaveBeenCalled());
});

it("已有正式默认图时直接选择准备方式，不要求再次上传",async()=>{
 library.items=[{id:"1",personaId:"2",kind:"base",name:"初始形象",category:"normal",prompt:"",description:"",enabled:true,imageFile:"base.png",previewFile:null,revision:0,createdAt:"a",updatedAt:"a"}];
 render(<PetImageSetup personaId="2" personaName="新伙伴" onClose={vi.fn()}/>);await screen.findByRole("dialog",{name:"选择表情准备方式"});expect(api.upload).not.toHaveBeenCalled();expect(api.generate).not.toHaveBeenCalled();
});
