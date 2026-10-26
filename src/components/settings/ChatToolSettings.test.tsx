import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import ChatToolSettings from "./ChatToolSettings";
const api=vi.hoisted(()=>({getSettings:vi.fn(),saveSettings:vi.fn()}));
vi.mock("../../api",()=>({configurationApi:api,errorMessage:(error:Error)=>error.message}));
beforeEach(()=>{vi.resetAllMocks();api.getSettings.mockResolvedValue({tool_call_rounds:"30"});api.saveSettings.mockImplementation(async value=>value);});afterEach(cleanup);
it("读取全局次数并只保存本项，不改主题或记忆参数",async()=>{
 render(<ChatToolSettings/>);const input=await screen.findByLabelText("每次聊天最多工具往返次数");expect(input).toHaveProperty("value","30");
 fireEvent.change(input,{target:{value:"40"}});fireEvent.click(screen.getByRole("button",{name:"保存工具设置"}));
 await waitFor(()=>expect(api.saveSettings).toHaveBeenCalledWith({tool_call_rounds:"40"}));expect(await screen.findByText(/对之后开始的聊天生效/)).toBeTruthy();
});
it.each(["0","-1","1.5","03","2147483648",""])("拒绝无效次数%s，不发送保存",async value=>{
 render(<ChatToolSettings/>);fireEvent.change(await screen.findByLabelText("每次聊天最多工具往返次数"),{target:{value}});
 expect(screen.getByRole("button",{name:"保存工具设置"})).toHaveProperty("disabled",true);expect(api.saveSettings).not.toHaveBeenCalled();
});
it("保存失败保留草稿，用户可修正重试",async()=>{
 api.saveSettings.mockRejectedValueOnce(new Error("写入失败"));render(<ChatToolSettings/>);const input=await screen.findByLabelText("每次聊天最多工具往返次数");
 fireEvent.change(input,{target:{value:"35"}});fireEvent.click(screen.getByRole("button",{name:"保存工具设置"}));
 expect(await screen.findByText("写入失败")).toBeTruthy();expect(input).toHaveProperty("value","35");
});
