import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import SkillPanel from "./SkillPanel";
const api=vi.hoisted(()=>({skills:vi.fn(),listenSkills:vi.fn(),listPersonas:vi.fn()}));
vi.mock("../../companionApi",()=>({companionApi:api}));
vi.mock("../../api",()=>({configurationApi:api,errorMessage:String}));
afterEach(cleanup);
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
