import { useState } from "react";
import { act, cleanup, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import TravelPanel from "./TravelPanel";
import LetterEditor from "./LetterEditor";
import { useTravel } from "../hooks/useTravel";
import type { CurrentContext, Letter, LetterTarget, Travel } from "../types";

const api = vi.hoisted(() => ({
  travel: { current:vi.fn(),list:vi.fn(),status:vi.fn(),retry:vi.fn(),listen:vi.fn() },
  letters: { list:vi.fn(),get:vi.fn(),flags:vi.fn(),delete:vi.fn(),status:vi.fn(),retry:vi.fn(),send:vi.fn(),listen:vi.fn() },
  pet: {listen:vi.fn()},
  journals: {list:vi.fn(),get:vi.fn(),status:vi.fn(),retry:vi.fn(),update:vi.fn(),delete:vi.fn()},
  navigation: {openLetter:vi.fn(),open:vi.fn()},
}));
vi.mock("../api", () => ({travelApi:api.travel,lettersApi:api.letters,petApi:api.pet,travelJournalsApi:api.journals,errorMessage:(e: {message?:string})=>e.message ?? "读取失败"}));
vi.mock("../platform/navigation", () => ({navigation:api.navigation}));
const context: CurrentContext = {personaId:"1",sessionId:"1",contextRevision:2};
const trip: Travel = {id:"3",sessionId:"1",status:"away",startedAt:"2026-10-09T00:00:00Z",plannedReturnAt:"2099-10-09T01:00:00Z",actualReturnAt:null,cancelledAt:null,midwayLetterAt:"2026-10-09T00:30:00Z",destination:"湖边",factsState:"ready",midwayLetterState:"done",returnLetterState:"pending",giftState:"pending",journalState:"pending",giftKind:null};
const target: LetterTarget = {...context,travelId:"3",replyToId:null};
let stored: Letter;
let exists = true;
beforeEach(() => {
  vi.resetAllMocks(); exists=true;
  stored={id:"7",sessionId:"1",travelId:"3",sender:"ai",content:"湖边很好 (・ω・)",replyToId:null,hasBeenReplied:false,replyDueAt:null,replyState:"none",letterKind:"midway",isFavorite:false,isRead:false,createdAt:"2026-10-09T00:30:00Z"};
  api.travel.current.mockResolvedValue(trip); api.travel.list.mockResolvedValue([trip]);
  api.travel.status.mockResolvedValue({active:null,failures:[]});
  api.journals.list.mockResolvedValue([{id:"8",sessionId:"1",travelId:"3",title:"旅行标题",createdAt:"a",updatedAt:"a"}]);
  api.journals.get.mockResolvedValue({id:"8",sessionId:"1",travelId:"3",title:"旅行标题",content:"旅行正文",createdAt:"a",updatedAt:"a"});
  api.journals.status.mockResolvedValue({active:null,pendingCount:0,failures:[],notice:null});
  api.letters.status.mockResolvedValue({active:null,failures:[],notice:null});
  api.letters.list.mockImplementation(async()=>exists?[{...stored}]:[]);
  api.letters.get.mockImplementation(async()=> { if(!exists) throw {code:"NOT_FOUND",message:"信件已删除"};return {...stored}; });
  api.letters.flags.mockImplementation(async (_context,_id,flags)=> {stored={...stored,isRead:flags.read ?? stored.isRead,isFavorite:flags.favorite ?? stored.isFavorite};return {...stored};});
  api.letters.delete.mockImplementation(async()=> {exists=false;});
  api.letters.send.mockResolvedValue({});
  for(const listen of [api.travel.listen,api.letters.listen,api.pet.listen]) listen.mockResolvedValue(()=>{});
  api.navigation.openLetter.mockResolvedValue(undefined); api.navigation.open.mockResolvedValue(undefined);
  HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};
  HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
});
afterEach(cleanup);

async function openMail() {
  render(<TravelPanel context={context} personaName="小喵" />);
  fireEvent.click(screen.getByRole("button",{name:"邮箱"}));
  fireEvent.click(await screen.findByRole("button",{name:/小喵的来信/}));
  await screen.findByText("本次旅行尚未结束，可以回复一次。");
}

it("游记在出行页独立标签中查看，标签切换保留编辑草稿", async () => {
  render(<TravelPanel context={context} personaName="小喵" />);
  expect(api.journals.list).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name:"旅行游记"}));
  fireEvent.click(await screen.findByRole("button", {name:/旅行标题/}));
  fireEvent.click(await screen.findByRole("button", {name:"编辑游记"}));
  fireEvent.change(screen.getByLabelText("游记正文"), {target:{value:"未保存的旅行草稿"}});
  fireEvent.click(screen.getByRole("button", {name:"邮箱"}));
  fireEvent.click(screen.getByRole("button", {name:"旅行游记"}));
  expect((screen.getByLabelText("游记正文") as HTMLTextAreaElement).value).toBe("未保存的旅行草稿");
});
it("详情成功加载后标已读，收藏和回信绑定原出行与上下文",async()=> {
  await openMail();
  expect(api.letters.flags).toHaveBeenCalledWith(context,"7",{read:true});
  fireEvent.click(screen.getByRole("button",{name:/小喵的来信/}));
  expect(screen.getByText("本次旅行尚未结束，可以回复一次。")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"收藏"}));
  await waitFor(()=>expect(api.letters.flags).toHaveBeenCalledWith(context,"7",{favorite:true}));
  await screen.findByRole("button",{name:"取消收藏"});
  fireEvent.click(screen.getByRole("button",{name:"回信"}));
  await waitFor(()=>expect(api.navigation.openLetter).toHaveBeenCalledWith(context,"3","7"));
});
it("删除需要确认，完成后关闭详情并刷新列表",async()=> {
  await openMail();
  fireEvent.click(screen.getByRole("button",{name:"删除信件"}));
  expect(api.letters.delete).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"确认删除"}));
  await waitFor(()=>expect(api.letters.delete).toHaveBeenCalledWith(context,"7"));
  await screen.findByText("还没有信件。出门期间可以寄一封信。");
  expect(screen.queryByText("本次旅行尚未结束，可以回复一次。")).toBeNull();
});
it("返回后的邮箱仍可查看，回信禁用，取消历史不显示奖励",async()=> {
  api.travel.current.mockResolvedValue(null);
  api.travel.list.mockResolvedValue([{...trip,status:"cancelled",cancelledAt:"2026-10-09T00:35:00Z",giftKind:null}]);
  render(<TravelPanel context={context} personaName="小喵" />);
  await screen.findByText("旅行已取消，不生成回来信、伴手礼或游记。");
  fireEvent.click(screen.getByRole("button",{name:"邮箱"}));
  fireEvent.click(await screen.findByRole("button",{name:/小喵的来信/}));
  await screen.findByText("只能在这封信所属旅行尚未结束时回复。");
  expect(screen.getByRole("button",{name:"回信"})).toHaveProperty("disabled",true);
});
it("行程失败与信件失败显示并按当前会话提前重试",async()=> {
  api.travel.status.mockResolvedValue({active:null,failures:[{travelId:"3",error:{code:"AI_ERROR",message:"行程失败"},retryAfterSeconds:20}]});
  api.letters.status.mockResolvedValue({active:null,failures:[{task:{travelId:"3",letterId:"8",kind:"reply"},error:{code:"AI_ERROR",message:"回信失败"},retryAfterSeconds:30}],notice:"本次减少了旧内容"});
  render(<TravelPanel context={context} personaName="小喵" />);
  await screen.findByText("本次减少了旧内容");
  fireEvent.click(screen.getByRole("button",{name:"重试行程生成"}));
  await waitFor(()=>expect(api.travel.retry).toHaveBeenCalledWith(context));
  await waitFor(()=>expect(screen.getByRole("button",{name:"重试待生成信件"})).toHaveProperty("disabled",false));
  fireEvent.click(screen.getByRole("button",{name:"重试待生成信件"}));
  await waitFor(()=>expect(api.letters.retry).toHaveBeenCalledWith(context));
});
it("分页读取较旧信件后，聚焦和新信件通知刷新仍保留展开内容",async()=> {
  const first=Array.from({length:30},(_,i)=>({...stored,id:String(100-i)}));
  const rows=[...first,{...stored,id:"1",content:"更早的信"}];
  api.letters.list.mockImplementation(async(_context,before)=>rows.filter(row=>!before || BigInt(row.id)<BigInt(before)).slice(0,30));
  let notify!: (change:{sessionId:string})=>void;
  api.letters.listen.mockImplementation(async callback=>{notify=callback;return ()=>{};});
  render(<TravelPanel context={context} personaName="小喵" />);
  fireEvent.click(screen.getByRole("button",{name:"邮箱"}));
  fireEvent.click(await screen.findByRole("button",{name:"更多信件"}));
  await screen.findByText("更早的信");
  expect(api.letters.list).toHaveBeenCalledWith(context,"71");
  expect(screen.getAllByRole("button",{name:/小喵的来信/})).toHaveLength(31);
  await act(async()=>{window.dispatchEvent(new Event("focus"));});
  expect(screen.getByText("更早的信")).toBeTruthy();
  rows.unshift({...stored,id:"101",content:"刚收到的新信"});
  await act(async()=>{notify({sessionId:"1"});});
  await screen.findByText("刚收到的新信");
  expect(screen.getByText("更早的信")).toBeTruthy();
  expect(screen.getAllByRole("button",{name:/小喵的来信/})).toHaveLength(32);
});
it("出行历史和邮箱分别保留分页深度，刷新不丢失较早出行",async()=> {
  const rows=[...Array.from({length:30},(_,i)=>({...trip,id:String(100-i),status:"returned" as const})),{...trip,id:"1",status:"returned" as const,destination:"较早的山间旅行"}];
  api.travel.list.mockImplementation(async(_context,before)=>rows.filter(row=>!before || BigInt(row.id)<BigInt(before)).slice(0,30));
  render(<TravelPanel context={context} personaName="小喵" />);
  fireEvent.click(await screen.findByRole("button",{name:"更多出行记录"}));
  await screen.findByText("较早的山间旅行");
  fireEvent.click(screen.getByRole("button",{name:"邮箱"}));
  await act(async()=>{window.dispatchEvent(new Event("focus"));});
  fireEvent.click(screen.getByRole("button",{name:"出行历史"}));
  expect(screen.getByText("较早的山间旅行")).toBeTruthy();
  expect(api.travel.list).toHaveBeenCalledWith(context,"71");
  expect(api.letters.list.mock.calls.every(call=>call[1]===undefined)).toBe(true);
});
it("分页请求迟于较新刷新返回时不再追加旧页面",async()=> {
  const first=Array.from({length:30},(_,i)=>({...stored,id:String(100-i)}));
  let finish!: (rows:Letter[])=>void;
  const pending=new Promise<Letter[]>(resolve=>{finish=resolve;});
  api.letters.list.mockImplementation(async(_context,before)=>before?pending:first);
  render(<TravelPanel context={context} personaName="小喵" />);
  fireEvent.click(screen.getByRole("button",{name:"邮箱"}));
  fireEvent.click(await screen.findByRole("button",{name:"更多信件"}));
  await waitFor(()=>expect(api.letters.list).toHaveBeenCalledWith(context,"71"));
  api.letters.list.mockResolvedValue(first);
  await act(async()=>{window.dispatchEvent(new Event("focus"));});
  await act(async()=>{finish([{...stored,id:"1",content:"失效的旧分页结果"}]);});
  expect(screen.queryByText("失效的旧分页结果")).toBeNull();
  expect(screen.getAllByRole("button",{name:/小喵的来信/})).toHaveLength(30);
  expect(screen.getByRole("button",{name:"更多信件"})).toHaveProperty("disabled",false);
});
function Editor({scope=context,editorTarget=target}: {scope?:CurrentContext;editorTarget?:LetterTarget}) {
  const [value,setValue]=useState("一路平安");
  return <LetterEditor context={scope} target={editorTarget} value={value} onChange={setValue} onCancel={async()=>{}} />;
}
it("写信成功后清空草稿，只提交正文和固定出行目标",async()=> {
  render(<Editor />);
  await waitFor(()=>expect(screen.getByRole("button",{name:"寄出"})).toHaveProperty("disabled",false));
  fireEvent.click(screen.getByRole("button",{name:"寄出"}));
  await waitFor(()=>expect(api.letters.send).toHaveBeenCalledWith(context,{travelId:"3",replyToId:null,content:"一路平安"}));
  await waitFor(()=>expect(screen.getByLabelText("信件正文")).toHaveProperty("value",""));
});
it("发送失败保留草稿，旅行结束后禁发而不删除正文",async()=> {
  api.letters.send.mockRejectedValue({code:"AI_ERROR",message:"发送失败"});
  render(<Editor />);
  await waitFor(()=>expect(screen.getByRole("button",{name:"寄出"})).toHaveProperty("disabled",false));
  fireEvent.click(screen.getByRole("button",{name:"寄出"}));
  await screen.findByText("发送失败");
  expect(screen.getByLabelText("信件正文")).toHaveProperty("value","一路平安");
  api.travel.current.mockResolvedValue(null);
  await act(async()=> {window.dispatchEvent(new Event("focus"));});
  await screen.findByText("这次旅行已结束或取消，草稿仍保留，但不能再发送。");
  expect(screen.getByRole("button",{name:"寄出"})).toHaveProperty("disabled",true);
});
it("会话切换不把旧草稿转投新会话，已回复原信禁用寄出",async()=> {
  stored.hasBeenReplied=true;
  const editorTarget={...target,replyToId:"7"};
  const {rerender}=render(<Editor editorTarget={editorTarget} />);
  await screen.findByText("原信已回复，删除回复也不会恢复资格。");
  expect(screen.getByRole("button",{name:"寄出"})).toHaveProperty("disabled",true);
  rerender(<Editor editorTarget={editorTarget} scope={{...context,sessionId:"2",contextRevision:3}} />);
  await screen.findByText("当前人设或会话已变化，旧草稿已保留供复制，不能转投新会话。");
  expect(screen.getByLabelText("信件正文")).toHaveProperty("value","一路平安");
  expect(api.letters.send).not.toHaveBeenCalled();
});
it("旧会话查询迟到结果不会覆盖新会话",async()=> {
  let resolveOld!: (value:Travel)=>void;
  const old=new Promise<Travel>(resolve=>{resolveOld=resolve;});
  api.travel.current.mockImplementation(async(scope)=>scope.sessionId==="1"?old:null);
  const {result,rerender}=renderHook(scope=>useTravel(scope),{initialProps:context});
  await waitFor(()=>expect(api.travel.current).toHaveBeenCalledWith(context));
  const next={...context,sessionId:"2",contextRevision:3};
  rerender(next);
  await waitFor(()=>expect(result.current.loading).toBe(false));
  await act(async()=>resolveOld(trip));
  expect(result.current.current).toBeNull();
});
it("忽略其他会话事件，聚焦仍主动补查当前出行",async()=> {
  const {result}=renderHook(()=>useTravel(context));
  await waitFor(()=>expect(result.current.loading).toBe(false));
  const count=api.travel.current.mock.calls.length;
  await act(async()=>api.travel.listen.mock.calls[0][0]({sessionId:"2"}));
  expect(api.travel.current.mock.calls.length).toBe(count);
  await act(async()=>window.dispatchEvent(new Event("focus")));
  expect(api.travel.current.mock.calls.length).toBe(count+1);
});
