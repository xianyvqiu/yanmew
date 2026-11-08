import {cleanup,fireEvent,render,screen,waitFor,within} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import PersonaCompanionSettings from "./PersonaCompanionSettings";
import type {Skill} from "../../companionApi";
const api=vi.hoisted(()=>({foods:vi.fn(),preferences:vi.fn(),skills:vi.fn(),listenSkills:vi.fn(),bind:vi.fn(),savePreferences:vi.fn(),openSkill:vi.fn()}));
vi.mock("../../companionApi",()=>({companionApi:api,openSkill:api.openSkill}));
vi.mock("../../api",()=>({errorMessage:(e:Error)=>e.message}));
let skills:Skill[];
beforeEach(()=>{
  vi.resetAllMocks();api.foods.mockResolvedValue([]);api.preferences.mockResolvedValue({defaultFoodId:"token",favoriteFoodId:null});api.listenSkills.mockResolvedValue(()=>{});
  skills=Array.from({length:13},(_,i)=>({id:String(i+1),title:`资料${i+1}`,summary:"摘要",body:`正文片段${i+1}`,revision:0,createdAt:"",updatedAt:`2026-10-${String(i+1).padStart(2,"0")}`,personas:i===0?[{id:"p",name:"伙伴"}]:[]}));
  api.skills.mockImplementation(async()=>skills);api.bind.mockResolvedValue(undefined);
});
afterEach(cleanup);
it("Skill独立折叠，已绑定项在分页前优先且两种时间排序均保持优先",async()=>{
  render(<PersonaCompanionSettings personaId="p"/>);
  const summary=screen.getByText("Skill",{selector:"summary span"});expect(summary.closest("summary")?.getAttribute("aria-expanded")).toBe("false");fireEvent.click(summary);
  await screen.findByText("正文片段1");expect(screen.getAllByRole("article")[0].textContent).toContain("资料1");
  expect(screen.queryByRole("heading",{name:"资料2"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"下一页"}));expect(screen.getByRole("heading",{name:"资料2"})).toBeTruthy();
  fireEvent.change(screen.getByLabelText("按时间排序"),{target:{value:"oldest"}});expect(screen.getAllByRole("article")[0].textContent).toContain("资料1");
  expect(screen.getByRole("heading",{name:"食物喜好"}).closest("section")?.querySelector("details")).toBeNull();
});
it("正文点击不跳转，详情定位指定Skill，绑定操作仍调用原接口",async()=>{
  render(<PersonaCompanionSettings personaId="p"/>);fireEvent.click(screen.getByText("Skill",{selector:"summary span"}));
  const body=await screen.findByText("正文片段1");fireEvent.click(body);expect(api.openSkill).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"查看 Skill 详情：资料1"}));expect(api.openSkill).toHaveBeenCalledWith("1");
  fireEvent.click(within(body.closest("article")!).getByRole("button",{name:"已绑定 · 解除"}));await waitFor(()=>expect(api.bind).toHaveBeenCalledWith("1","p",false));
});
