import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useSkillUpdate} from "../hooks/useSkillUpdate";
import SkillUpdateConfirmation from "./SkillUpdateConfirmation";
import type {SkillPreview} from "../companionApi";
const api=vi.hoisted(()=>({skillPreviews:vi.fn(),resolveSkillPreview:vi.fn(),listenSkills:vi.fn()}));
vi.mock("../companionApi",()=>({companionApi:api}));
const context={personaId:"1",sessionId:"1",contextRevision:0};
const preview:SkillPreview={id:"preview",before:{id:"1",title:"旧标题",summary:"旧简介",body:"共享原文",revision:0,createdAt:"a",updatedAt:"a",personas:[{id:"1",name:"小猫"},{id:"2",name:"龙娘"}]},proposed:{title:"新标题",summary:"新简介",body:"拟修改内容"}};
function Harness({scope=context}){const update=useSkillUpdate(scope);return <SkillUpdateConfirmation update={update}/>;}
afterEach(cleanup);beforeEach(()=>{vi.clearAllMocks();api.listenSkills.mockResolvedValue(vi.fn());});
it("展示共享范围和拟修改内容，只有用户确认才提交，冲突不自动重试",async()=>{
 api.skillPreviews.mockResolvedValue([preview]);api.resolveSkillPreview.mockRejectedValueOnce({message:"绑定范围变化"});render(<Harness/>);
 await screen.findByText("拟修改内容");expect(screen.getByText(/将影响：小猫、龙娘/)).toBeTruthy();expect(api.resolveSkillPreview).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"确认修改共享原文"}));await screen.findByRole("alert");expect(api.resolveSkillPreview).toHaveBeenCalledWith(context,"preview",true);expect(api.resolveSkillPreview).toHaveBeenCalledTimes(1);
 api.resolveSkillPreview.mockResolvedValueOnce(undefined);api.skillPreviews.mockResolvedValue([]);fireEvent.click(screen.getByRole("button",{name:"取消"}));await waitFor(()=>expect(screen.queryByText("拟修改内容")).toBeNull());expect(api.resolveSkillPreview).toHaveBeenLastCalledWith(context,"preview",false);
});
it("切换会话后迟到的共享预览不能显示或提交",async()=>{
 let finish!:(v:SkillPreview[])=>void;api.skillPreviews.mockImplementationOnce(()=>new Promise<SkillPreview[]>(resolve=>{finish=resolve;})).mockResolvedValue([]);
 const view=render(<Harness/>);await waitFor(()=>expect(finish).toBeDefined());view.rerender(<Harness scope={{...context,sessionId:"2",contextRevision:1}}/>);
 await act(async()=>{finish([preview]);});expect(screen.queryByRole("button",{name:"确认修改共享原文"})).toBeNull();expect(api.resolveSkillPreview).not.toHaveBeenCalled();
});
