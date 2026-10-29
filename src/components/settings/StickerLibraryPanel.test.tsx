import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import StickerLibraryPanel from "./StickerLibraryPanel";
import StickerBindings from "./StickerBindings";
import type {ChatSticker,StickerLibrary,StickerTag} from "../../stickerApi";
const api=vi.hoisted(()=>({query:vi.fn(),mutate:vi.fn(),import:vi.fn(),image:vi.fn(),listen:vi.fn()}));
vi.mock("../../stickerApi",()=>({stickerApi:api}));
vi.mock("../../api",()=>({errorMessage:(e:Error)=>e.message,configurationApi:{listPersonas:async()=>[{id:"1",name:"小猫"},{id:"2",name:"小鱼"}]}}));
let libraries:StickerLibrary[],tags:StickerTag[],stickers:ChatSticker[];
beforeEach(()=>{
  vi.resetAllMocks();libraries=[{id:"1",title:"猫猫收藏",description:"日常猫猫",revision:0,createdAt:"2026-10-28",updatedAt:"2026-10-28",count:2,personaIds:["1"]}];
  tags=[{id:"1",name:"开心",revision:0,createdAt:"",updatedAt:""},{id:"2",name:"猫猫",revision:0,createdAt:"",updatedAt:""}];
  stickers=[{id:"1",libraryId:"1",title:"挥手猫",description:"欢迎回来",filename:"猫.gif",mediaType:"image/gif",revision:0,createdAt:"",updatedAt:"",tags:[tags[0],tags[1]]},{id:"2",libraryId:"1",title:"笑脸",description:"微笑",filename:"笑脸.png",mediaType:"image/png",revision:0,createdAt:"",updatedAt:"",tags:[tags[0]]}];
  api.listen.mockResolvedValue(()=>{});api.image.mockResolvedValue("data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==");api.import.mockResolvedValue(undefined);api.mutate.mockResolvedValue({saved:true,id:"3"});
  api.query.mockImplementation(async(q)=>{let rows:any[]=q.entity==="libraries"?libraries:q.entity==="tags"?tags:stickers;rows=rows.filter(r=>(q.entity!=="stickers"||!q.tagIds?.length||q.tagIds.every((id:string)=>r.tags.some((t:StickerTag)=>t.id===id)))&&(!q.query||`${r.title??r.name} ${r.description??""}`.includes(q.query)));const pages=Math.max(1,Math.ceil(rows.length/12)),page=Math.min(q.page??1,pages);return{items:rows.slice((page-1)*12,page*12),page,pages,total:rows.length};});
});
afterEach(cleanup);
async function openLibrary(){render(<StickerLibraryPanel/>);fireEvent.click(await screen.findByRole("button",{name:/猫猫收藏/}));await screen.findByText("挥手猫");}
it("新建库填写标题和简介，不提供用户发送入口",async()=>{
  render(<StickerLibraryPanel/>);fireEvent.click(screen.getByRole("button",{name:"新建表情包库"}));const d=screen.getByRole("dialog");fireEvent.change(within(d).getByLabelText("表情包库标题"),{target:{value:"日常贴贴"}});fireEvent.change(within(d).getByLabelText("库简介"),{target:{value:"轻松聊天用"}});fireEvent.click(within(d).getByRole("button",{name:"保存"}));await waitFor(()=>expect(api.mutate).toHaveBeenCalledWith({op:"saveLibrary",id:null,revision:null,title:"日常贴贴",description:"轻松聊天用"}));expect(screen.queryByRole("button",{name:/发送表情包/})).toBeNull();
});
it("库搜索和底部分页查询完整集合",async()=>{
  libraries.push(...Array.from({length:12},(_,i)=>({...libraries[0],id:String(i+2),title:`库${i}`})));render(<StickerLibraryPanel/>);await screen.findByText("库0");await waitFor(()=>expect((screen.getByRole("button",{name:"下一页"}) as HTMLButtonElement).disabled).toBe(false));fireEvent.click(screen.getByRole("button",{name:"下一页"}));await waitFor(()=>expect(api.query).toHaveBeenCalledWith(expect.objectContaining({entity:"libraries",page:2})));fireEvent.change(screen.getByRole("searchbox",{name:"搜索表情包库"}),{target:{value:"猫猫收藏"}});await screen.findByText("猫猫收藏");await waitFor(()=>expect(screen.queryByText("库0")).toBeNull());
});
it("批量导入保留GIF数据并逐张报告结果",async()=>{
  await openLibrary();const input=document.querySelector('input[type="file"]')!;fireEvent.change(input,{target:{files:[new File(["gif"],"开心.gif",{type:"image/gif"}),new File(["png"],"猫猫.png",{type:"image/png"})]}});await screen.findByText("已导入 2 个表情包");expect(api.import).toHaveBeenCalledTimes(2);expect(api.import.mock.calls[0]).toEqual(["1",{filename:"开心.gif",dataUrl:expect.stringContaining("data:image/gif;base64,")}]);
});
it("多标签必须同时符合，搜索和筛选一起生效",async()=>{
  await openLibrary();fireEvent.click(screen.getByRole("button",{name:/按标签筛选/}));fireEvent.click(await screen.findByRole("button",{name:"开心"}));fireEvent.click(await screen.findByRole("button",{name:"猫猫"}));await waitFor(()=>expect(api.query).toHaveBeenCalledWith(expect.objectContaining({entity:"stickers",tagIds:["1","2"]})));await waitFor(()=>expect(screen.queryByText("笑脸")).toBeNull());expect(screen.getByText("挥手猫")).toBeTruthy();
});
it("表情包弹窗保存标题介绍和多个标签，失败保留草稿",async()=>{
  await openLibrary();fireEvent.click(screen.getByRole("button",{name:/挥手猫/}));const d=screen.getByRole("dialog");fireEvent.change(within(d).getByLabelText("表情包标题"),{target:{value:"新的标题"}});api.mutate.mockRejectedValueOnce(new Error("资料已更新"));fireEvent.click(within(d).getByRole("button",{name:"保存"}));await within(d).findByText("资料已更新");expect((within(d).getByLabelText("表情包标题") as HTMLInputElement).value).toBe("新的标题");expect(api.mutate).toHaveBeenCalledWith(expect.objectContaining({op:"saveSticker",tagIds:["1","2"]}));fireEvent.click(within(d).getByRole("button",{name:"关闭"}));await waitFor(()=>expect(screen.queryByRole("dialog")).toBeNull());
});
it("标签管理可搜索修改和确认删除，不删除历史图片",async()=>{
  render(<StickerLibraryPanel/>);fireEvent.click(screen.getByRole("button",{name:"标签管理"}));fireEvent.click(await screen.findByRole("button",{name:/开心.*编辑标签/}));const d=screen.getByRole("dialog");fireEvent.click(within(d).getByRole("button",{name:"删除标签"}));expect(within(d).getByText("已经发送的聊天图片会保留。")).toBeTruthy();fireEvent.click(within(d).getByRole("button",{name:"确认删除"}));await waitFor(()=>expect(api.mutate).toHaveBeenCalledWith({op:"delete",entity:"tags",id:"1",revision:0}));
});
it("人设可绑定和解除共享库",async()=>{
  render(<StickerBindings personaId="2"/>);fireEvent.click(await screen.findByRole("button",{name:"绑定"}));await waitFor(()=>expect(api.mutate).toHaveBeenCalledWith({op:"bind",libraryId:"1",personaId:"2",bound:true}));
});
