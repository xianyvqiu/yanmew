import { useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { usePetChat } from "../windows/usePetChat";
import { PetChatBubble, PetChatInput } from "./PetChat";
import type { ChatReplyEvent, ChatRequestSnapshot, PetActivity, RoundSnapshot } from "../types";

const api = vi.hoisted(() => ({ listen:vi.fn(),listenMessages:vi.fn(),current:vi.fn(),lastRound:vi.fn(),messages:vi.fn(),submit:vi.fn(),cancel:vi.fn() }));
vi.mock("../api",()=>({ chatApi:api,sessionApi:api,webSearchApi:{open:vi.fn()},errorMessage:(error:Error)=>error.message }));
const pet:PetActivity={personaId:"1",sessionId:"2",contextRevision:3,revision:1,activity:"normal",expression:"(・ω・)",intimacy:0,sleepUntil:null,error:null,canInteract:true,canWander:true};
const round:RoundSnapshot={round:{id:"10",sessionId:"2",status:"pending",createdAt:"2026-10-15",completedAt:null,stopReason:null},userMessage:{id:"20",sessionId:"2",roundId:"10",role:"user",content:"你好",expression:null,createdAt:"2026-10-15"},assistantMessage:null};
const receipt:ChatRequestSnapshot={personaId:"1",sessionId:"2",contextRevision:3,requestId:"30",roundId:"10",seq:0,partialContent:"",expression:null,round};
let receive:(event:ChatReplyEvent)=>void;
function Harness({ view=pet,onDetails=()=>{} }:{view?:PetActivity;onDetails?:()=>void}) {
  const chat=usePetChat(view);const [open,setOpen]=useState(true);
  return <><button onClick={()=>setOpen(value=>!value)}>切换输入</button><PetChatBubble chat={chat} onDetails={onDetails}/>{open&&<PetChatInput chat={chat} hasSession={Boolean(view.sessionId)} away={view.activity==="away"} pending={false} onDetails={onDetails} onLetter={()=>{}}/>}</>;
}
beforeEach(()=>{
  vi.resetAllMocks();api.listen.mockImplementation(async(callback)=>{receive=callback;return()=>{};});api.listenMessages.mockResolvedValue(()=>{});
  api.current.mockResolvedValue(null);api.lastRound.mockResolvedValue(null);api.messages.mockResolvedValue({items:[],nextAfterId:null});api.submit.mockResolvedValue(receipt);api.cancel.mockResolvedValue(round);
});
afterEach(cleanup);
it("宠窗发送复用当前会话，头顶流式更新；收起输入不取消，完成后保留正文",async()=>{
  const details=vi.fn();render(<Harness onDetails={details}/>);
  await waitFor(()=>expect((screen.getByRole("textbox",{name:"聊天消息"}) as HTMLTextAreaElement).disabled).toBe(false));
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"你好"}});fireEvent.click(screen.getByRole("button",{name:"发送"}));
  await screen.findByRole("region",{name:"桌宠聊天回复"});
  expect(api.submit).toHaveBeenCalledWith({personaId:"1",sessionId:"2",contextRevision:3},"你好");
  act(()=>receive({...receipt,type:"text",seq:1,text:"你好",saved:null}));expect(screen.getByText("你好")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"切换输入"}));
  act(()=>receive({...receipt,type:"text",seq:2,text:"，喵",saved:null}));expect(screen.getByText("你好，喵")).toBeTruthy();expect(api.cancel).not.toHaveBeenCalled();
  const saved:RoundSnapshot={...round,round:{...round.round,status:"completed"},assistantMessage:{...round.userMessage,id:"21",role:"assistant",content:"你好，喵"}};
  api.lastRound.mockResolvedValue(saved);
  await act(async()=>receive({...receipt,type:"finished",seq:3,finish:"stop",saved}));
  expect(screen.getByText("你好，喵")).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"聊天详情"}));expect(details).toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"收起聊天回复"}));expect(screen.queryByRole("region",{name:"桌宠聊天回复"})).toBeNull();
});
it("暂停沿用当前请求，切换会话清理快捷草稿及旧回复",async()=>{
  const page=render(<Harness/>);
  await waitFor(()=>expect((screen.getByRole("textbox") as HTMLTextAreaElement).disabled).toBe(false));
  fireEvent.change(screen.getByRole("textbox"),{target:{value:"你好"}});fireEvent.click(screen.getByRole("button",{name:"发送"}));
  await screen.findByRole("button",{name:"暂停回复"});fireEvent.click(screen.getByRole("button",{name:"暂停回复"}));await waitFor(()=>expect(api.cancel).toHaveBeenCalledWith(receipt));
  page.rerender(<Harness view={{...pet,sessionId:"4",contextRevision:4}}/>);
  await waitFor(()=>expect(screen.queryByRole("region",{name:"桌宠聊天回复"})).toBeNull());
  expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("");
});
it("出行不出现发送输入，未选会话提供聊天详情入口",async()=>{
  const page=render(<Harness view={{...pet,activity:"away"}}/>);
  expect(screen.queryByRole("textbox")).toBeNull();expect(screen.getByRole("button",{name:"写信"})).toBeTruthy();
  page.rerender(<Harness view={{...pet,sessionId:null,contextRevision:4}}/>);
  expect(screen.queryByRole("textbox")).toBeNull();expect(screen.getByRole("button",{name:"聊天详情"})).toBeTruthy();expect(api.submit).not.toHaveBeenCalled();
});
