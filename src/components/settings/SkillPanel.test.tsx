import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import SkillPanel from "./SkillPanel";
const api=vi.hoisted(()=>({skills:vi.fn(),listenSkills:vi.fn(),listPersonas:vi.fn(),parseSkillMarkdown:vi.fn(),cancelSkillMarkdownImport:vi.fn(),saveSkill:vi.fn()}));
vi.mock("../../companionApi",()=>({companionApi:api}));
vi.mock("../../api",()=>({configurationApi:api,errorMessage:String}));
afterEach(cleanup);
beforeEach(()=>{vi.resetAllMocks();api.skills.mockResolvedValue([]);api.listPersonas.mockResolvedValue([]);api.listenSkills.mockResolvedValue(vi.fn());api.cancelSkillMarkdownImport.mockResolvedValue(undefined);api.saveSkill.mockResolvedValue(undefined);});
it("按全部、未绑定和人设筛选后再搜索分页，切换分类回到第一页",async()=>{
 api.listenSkills.mockResolvedValue(vi.fn());api.listPersonas.mockResolvedValue([{id:"1",name:"小猫"},{id:"2",name:"龙娘"}]);
 const items=Array.from({length:13},(_,n)=>({id:String(n+1),title:`小猫规则${n}`,summary:"简介",body:"正文",updatedAt:"2026-10-20",personas:[{id:"1",name:"小猫"}]}));
 items.push({id:"14",title:"龙娘规则",summary:"简介",body:"正文",updatedAt:"2026-10-20",personas:[{id:"2",name:"龙娘"}]},{id:"15",title:"未绑定规则",summary:"简介",body:"正文",updatedAt:"2026-10-20",personas:[]});api.skills.mockResolvedValue(items);
 render(<SkillPanel/>);const filter=await screen.findByRole("combobox",{name:"按人设筛选Skill"});
 fireEvent.change(filter,{target:{value:"1"}});fireEvent.click(screen.getByRole("button",{name:"下一页"}));expect(screen.getByText("2 / 2 页 · 13 条")).toBeTruthy();
 fireEvent.change(filter,{target:{value:"2"}});expect(screen.getByText("1 / 1 页 · 1 条")).toBeTruthy();expect(screen.getByRole("button",{name:/龙娘规则/})).toBeTruthy();expect(screen.queryByRole("button",{name:/小猫规则/})).toBeNull();
 fireEvent.change(filter,{target:{value:"unbound"}});expect(screen.getByRole("button",{name:/未绑定规则/})).toBeTruthy();
 fireEvent.change(screen.getByRole("searchbox"),{target:{value:"龙娘"}});expect(screen.queryByRole("button",{name:/龙娘规则/})).toBeNull();
 fireEvent.change(filter,{target:{value:"all"}});expect(screen.getByRole("button",{name:/龙娘规则/})).toBeTruthy();
});
it("上传保留文件名和完整正文，AI只填简介，用户保存前不写仓库",async()=>{
 const body="---\nname: 不作为标题\n---\n# 其他一级标题\n\n保留  原文。\n";
 api.parseSkillMarkdown.mockResolvedValue({title:"不采用的改名",summary:"表达与处理规则",body:"不采用的改写"});
 render(<SkillPanel/>);
 for(const title of ["SKILL_BASE","AGENTS","SKILL_BASE.md","AGENTS.md"])expect(screen.getByText(title,{selector:"code"})).toBeTruthy();
 fireEvent.change(screen.getByLabelText("上传 Markdown 文件"),{target:{files:[new File([body],"AGENTS.md",{type:"text/markdown"})]}});
 await screen.findByDisplayValue("表达与处理规则");expect(api.parseSkillMarkdown).toHaveBeenCalledWith("AGENTS.md",body);
 expect((screen.getByLabelText("标题") as HTMLInputElement).value).toBe("AGENTS.md");expect((screen.getByLabelText("Skill 正文") as HTMLTextAreaElement).value).toBe(body);
 expect(api.saveSkill).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"保存"}));
 await waitFor(()=>expect(api.saveSkill).toHaveBeenCalledWith({id:null,revision:null,title:"AGENTS.md",summary:"表达与处理规则",body}));
});
it("AI解析失败仍保留可编辑原文和文件标题，不自动保存",async()=>{
 api.parseSkillMarkdown.mockRejectedValue(new Error("模型连接失败"));render(<SkillPanel/>);
 fireEvent.change(screen.getByLabelText("上传 Markdown 文件"),{target:{files:[new File(["# 原文\n规则"],"SKILL_BASE.md")]}});
 await screen.findByRole("alert");expect((screen.getByLabelText("标题") as HTMLInputElement).value).toBe("SKILL_BASE.md");expect((screen.getByLabelText("Skill 正文") as HTMLTextAreaElement).value).toBe("# 原文\n规则");
 expect((screen.getByRole("button",{name:"保存"}) as HTMLButtonElement).disabled).toBe(false);expect(api.saveSkill).not.toHaveBeenCalled();
});
it("停止解析后迟到结果不能覆盖用户修改的草稿",async()=>{
 let finish!:(value:{title:string;summary:string;body:string})=>void;api.parseSkillMarkdown.mockReturnValue(new Promise(resolve=>{finish=resolve;}));render(<SkillPanel/>);
 fireEvent.change(screen.getByLabelText("上传 Markdown 文件"),{target:{files:[new File(["完整正文"],"旅行.md")]}});
 await waitFor(()=>expect(api.parseSkillMarkdown).toHaveBeenCalled());fireEvent.click(screen.getByRole("button",{name:"停止解析"}));
 fireEvent.change(screen.getByLabelText("简介"),{target:{value:"我填写的简介"}});
 await act(async()=>{finish({title:"旅行.md",summary:"迟到简介",body:"完整正文"});});
 expect((screen.getByLabelText("简介") as HTMLTextAreaElement).value).toBe("我填写的简介");expect(api.cancelSkillMarkdownImport).toHaveBeenCalledTimes(1);expect(api.saveSkill).not.toHaveBeenCalled();
});
it("非Markdown文件不发送到模型",async()=>{
 render(<SkillPanel/>);fireEvent.change(screen.getByLabelText("上传 Markdown 文件"),{target:{files:[new File(["内容"],"test.txt")]}});
 expect(await screen.findByRole("alert")).toBeTruthy();expect(api.parseSkillMarkdown).not.toHaveBeenCalled();expect(screen.queryByLabelText("Skill 正文")).toBeNull();
});
