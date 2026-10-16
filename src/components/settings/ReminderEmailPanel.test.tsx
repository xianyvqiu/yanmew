import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import ReminderEmailPanel, { recommendedEmail } from "./ReminderEmailPanel";
import type { ReminderEmailView } from "../../types";
const api = vi.hoisted(() => ({ get:vi.fn(),sendCode:vi.fn(),verify:vi.fn(),save:vi.fn(),enable:vi.fn(),retry:vi.fn(),open:vi.fn() }));
vi.mock("../../api", () => ({ reminderEmailApi:api,webSearchApi:{open:api.open},errorMessage:(e:{message:string})=>e.message }));
function view(overrides:Partial<ReminderEmailView>={}):ReminderEmailView {
  return {connection:recommendedEmail(""),revision:0,enabled:false,verified:false,hasSecret:false,deliveries:[],...overrides};
}
beforeEach(() => {vi.resetAllMocks();api.get.mockResolvedValue(view());api.open.mockResolvedValue(undefined);});
afterEach(cleanup);
it("常见邮箱自动推荐；未知和企业域名不猜SMTP地址",()=>{
  expect(recommendedEmail("u@QQ.COM").host).toBe("smtp.qq.com");
  expect(recommendedEmail("u@163.com").username).toBe("u@163.com");
  expect(recommendedEmail("u@qq.com").to).toBe("");
  expect(recommendedEmail("u@example.com").host).toBe("");
  expect(recommendedEmail("u@qq.example.com").host).toBe("");
});
it("默认关闭且未验证不能启用；分别填发信和收信邮箱，验证码验证后开启",async()=>{
  const connection={...recommendedEmail("u@qq.com"),to:"receiver@163.com"};
  const configured=view({connection,revision:1,hasSecret:true});
  api.sendCode.mockResolvedValue(configured);api.verify.mockResolvedValue({...configured,enabled:true,verified:true});
  render(<ReminderEmailPanel/>);
  const email=await screen.findByRole("textbox",{name:"发信邮箱"});
  expect((screen.getByRole("switch",{name:"邮件提醒"}) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(email,{target:{value:"u@qq.com"}});
  expect((screen.getByLabelText("收信邮箱") as HTMLInputElement).value).toBe("");
  fireEvent.change(screen.getByLabelText("收信邮箱"),{target:{value:"receiver@163.com"}});
  fireEvent.change(screen.getByLabelText("发信邮箱授权码"),{target:{value:"authorization"}});
  expect(screen.getByText("如何获取授权码 · QQ邮箱")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"发送验证码"}));
  await screen.findByLabelText("6位验证码");
  expect(api.sendCode).toHaveBeenCalledWith({connection,revision:0,secret:{action:"set",value:"authorization"}});
  expect(screen.getByText(/验证码已发送到 receiver@163.com/)).toBeTruthy();
  fireEvent.change(screen.getByLabelText("6位验证码"),{target:{value:"123456"}});
  fireEvent.click(screen.getByRole("button",{name:"验证并开启"}));
  await screen.findByText("邮箱已验证，邮件提醒已开启。");
  expect(api.verify).toHaveBeenCalledWith("123456",1);
  expect(screen.getByRole("switch",{name:"邮件提醒"}).getAttribute("aria-checked")).toBe("true");
});
it("高级自定义字段在修改邮箱时保留，恢复推荐配置才覆盖",async()=>{
  render(<ReminderEmailPanel/>);
  const email=await screen.findByLabelText("发信邮箱");
  fireEvent.change(email,{target:{value:"u@qq.com"}});
  fireEvent.change(screen.getByLabelText("收信邮箱"),{target:{value:"receiver@126.com"}});
  fireEvent.click(screen.getByText(/高级设置/, { selector:"summary" }));
  fireEvent.change(screen.getByLabelText("发信服务器（SMTP）"),{target:{value:"custom.example.com"}});
  fireEvent.change(email,{target:{value:"other@163.com"}});
  expect((screen.getByLabelText("发信服务器（SMTP）") as HTMLInputElement).value).toBe("custom.example.com");
  expect((screen.getByLabelText("SMTP登录账号") as HTMLInputElement).value).toBe("other@163.com");
  expect((screen.getByLabelText("收信邮箱") as HTMLInputElement).value).toBe("receiver@126.com");
  fireEvent.click(screen.getByRole("button",{name:"恢复推荐配置"}));
  expect((screen.getByLabelText("发信服务器（SMTP）") as HTMLInputElement).value).toBe("smtp.163.com");
  expect((screen.getByLabelText("收信邮箱") as HTMLInputElement).value).toBe("receiver@126.com");
});
it("错误展示且不伪装绑定成功；修改地址后清除旧验证码输入",async()=>{
  api.sendCode.mockResolvedValue(view({connection:{...recommendedEmail("u@126.com"),to:"receiver@qq.com"},revision:1,hasSecret:true}));
  api.verify.mockRejectedValue({message:"验证码不正确，还可尝试4次"});
  render(<ReminderEmailPanel/>);
  fireEvent.change(await screen.findByLabelText("发信邮箱"),{target:{value:"u@126.com"}});
  fireEvent.change(screen.getByLabelText("收信邮箱"),{target:{value:"receiver@qq.com"}});
  fireEvent.change(screen.getByLabelText("发信邮箱授权码"),{target:{value:"test"}});
  fireEvent.click(screen.getByRole("button",{name:"发送验证码"}));
  fireEvent.change(await screen.findByLabelText("6位验证码"),{target:{value:"123456"}});
  fireEvent.click(screen.getByRole("button",{name:"验证并开启"}));
  await screen.findByRole("alert");
  expect(screen.getByText("验证码不正确，还可尝试4次")).toBeTruthy();
  expect(screen.getByRole("switch",{name:"邮件提醒"}).getAttribute("aria-checked")).toBe("false");
  fireEvent.change(screen.getByLabelText("发信邮箱"),{target:{value:"next@126.com"}});
  expect(screen.queryByLabelText("6位验证码")).toBeNull();
});
it("送达结果不确定需确认重发；取消确认不调用后端",async()=>{
  api.get.mockResolvedValue(view({connection:{...recommendedEmail("u@qq.com"),to:"u@qq.com"},verified:true,enabled:true,revision:1,hasSecret:true,deliveries:[{id:"1",recipient:"u@qq.com",subject:"[颜喵] 提醒：事项",state:"uncertain",attempts:1,nextRetryAt:null,error:"发送结果不确定",createdAt:"2026-10-15T00:00:00Z",sentAt:null}]}));
  const confirm=vi.spyOn(window,"confirm").mockReturnValue(false);
  render(<ReminderEmailPanel/>);
  await screen.findByText("已验证 · u@qq.com");
  fireEvent.click(screen.getByText(/最近发送记录/));
  fireEvent.click(screen.getByRole("button",{name:"确认后重新发送"}));
  expect(api.retry).not.toHaveBeenCalled();
  confirm.mockReturnValue(true);api.retry.mockResolvedValue(view({verified:true,enabled:true}));
  fireEvent.click(screen.getByRole("button",{name:"确认后重新发送"}));
  await waitFor(()=>expect(api.retry).toHaveBeenCalledWith("1",1,true));confirm.mockRestore();
});

it("后台刷新不把旧草稿伪装成新修订，提交仍携带开始编辑时的版本",async()=>{
  render(<ReminderEmailPanel/>);
  fireEvent.change(await screen.findByLabelText("发信邮箱"),{target:{value:"draft@qq.com"}});
  fireEvent.change(screen.getByLabelText("收信邮箱"),{target:{value:"receiver@qq.com"}});
  fireEvent.change(screen.getByLabelText("发信邮箱授权码"),{target:{value:"test"}});
  api.get.mockResolvedValue(view({revision:5,connection:recommendedEmail("external@163.com")}));
  fireEvent(window,new Event("focus"));
  await waitFor(()=>expect(api.get).toHaveBeenCalledTimes(2));
  expect((screen.getByLabelText("发信邮箱") as HTMLInputElement).value).toBe("draft@qq.com");
  api.sendCode.mockRejectedValue({message:"邮件配置已被修改，请重新读取后再操作"});
  fireEvent.click(screen.getByRole("button",{name:"发送验证码"}));
  await screen.findByRole("alert");
  expect(api.sendCode.mock.calls[0][0].revision).toBe(0);
  fireEvent.click(screen.getByRole("button",{name:"放弃修改并重新读取"}));
  await screen.findByText("已重新读取邮件配置。");
  expect((screen.getByLabelText("发信邮箱") as HTMLInputElement).value).toBe("external@163.com");
});

it("已验证配置通过横向开关关闭与重开，使用后端返回的新修订号",async()=>{
  const configured=view({connection:{...recommendedEmail("sender@qq.com"),to:"receiver@163.com"},verified:true,enabled:true,revision:4,hasSecret:true});
  api.get.mockResolvedValue(configured);
  api.enable.mockResolvedValueOnce({...configured,enabled:false,revision:5}).mockResolvedValueOnce({...configured,enabled:true,revision:6});
  render(<ReminderEmailPanel/>);
  const toggle=await screen.findByRole("switch",{name:"邮件提醒"});
  fireEvent.click(toggle);
  await screen.findByText("邮件提醒已关闭，未开始发送的任务已取消。");
  expect(api.enable).toHaveBeenNthCalledWith(1,false,4);
  expect(toggle.getAttribute("aria-checked")).toBe("false");
  fireEvent.click(toggle);
  await screen.findByText("邮件提醒已开启，只发送此后触发的提醒。");
  expect(api.enable).toHaveBeenNthCalledWith(2,true,5);
  expect(toggle.getAttribute("aria-checked")).toBe("true");
});
