import {act,cleanup,renderHook} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {usePetReturnNotice} from "./usePetReturnNotice";
import type {PetActivity} from "../types";
const api=vi.hoisted(()=>({list:vi.fn()}));
vi.mock("../api",()=>({travelApi:api}));
const away={personaId:"1",sessionId:"1",contextRevision:0,revision:2,activity:"away"}as PetActivity;
const home={...away,activity:"normal",revision:3}as PetActivity;
beforeEach(()=>{vi.useFakeTimers();api.list.mockResolvedValue([{id:"1",status:"returned"}]);});
afterEach(()=>{cleanup();vi.useRealTimers();vi.resetAllMocks();});
it("正常返回显示五秒，普通状态刷新不重复或延长",async()=>{
 const {result,rerender}=renderHook(view=>usePetReturnNotice(view,true),{initialProps:away});
 rerender(home);await act(async()=>{});expect(result.current).toBe(true);
 act(()=>vi.advanceTimersByTime(4000));rerender({...home,revision:4});expect(result.current).toBe(true);
 act(()=>vi.advanceTimersByTime(999));expect(result.current).toBe(true);act(()=>vi.advanceTimersByTime(1));expect(result.current).toBe(false);expect(api.list).toHaveBeenCalledTimes(1);
});
it("首次打开在家和切换上下文不误报回来",async()=>{
 const {result,rerender}=renderHook(view=>usePetReturnNotice(view,true),{initialProps:home});
 await act(async()=>{});expect(result.current).toBe(false);expect(api.list).not.toHaveBeenCalled();
 rerender(away);rerender({...home,sessionId:"2",contextRevision:1});await act(async()=>{});expect(api.list).not.toHaveBeenCalled();
});
it("取消旅行和核对失败不显示返回",async()=>{
 api.list.mockResolvedValue([{id:"1",status:"cancelled"}]);const {result,rerender}=renderHook(view=>usePetReturnNotice(view,true),{initialProps:away});
 rerender(home);await act(async()=>{});expect(result.current).toBe(false);
 rerender(away);api.list.mockRejectedValue(new Error("无法读取"));rerender(home);await act(async()=>{});expect(result.current).toBe(false);
});
it("核对结果迟到不能在其他会话或隐藏后弹出",async()=>{
 let resolve!:(items:unknown[])=>void;api.list.mockImplementation(()=>new Promise(r=>{resolve=r;}));
 const {result,rerender}=renderHook(({view,enabled})=>usePetReturnNotice(view,enabled),{initialProps:{view:away,enabled:true}});
 rerender({view:home,enabled:true});rerender({view:{...home,sessionId:"2",contextRevision:1},enabled:false});
 await act(async()=>resolve([{status:"returned"}]));expect(result.current).toBe(false);
});
