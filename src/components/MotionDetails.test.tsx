import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import MotionDetails from "./MotionDetails";
afterEach(()=>{cleanup();vi.useRealTimers();vi.unstubAllGlobals();});
it("收起保留退出动画，内部禁用，快速重开不被旧计时器关闭",()=>{
 vi.useFakeTimers();render(<MotionDetails><summary>查看记录</summary><button>详情操作</button></MotionDetails>);const summary=screen.getByText("查看记录"),details=summary.closest("details")!;fireEvent.click(summary);expect(details.open).toBe(true);const body=screen.getByText("详情操作").parentElement!;expect(body.inert).toBe(false);fireEvent.click(summary);expect(details.open).toBe(true);expect(body.inert).toBe(true);expect(body.getAttribute("aria-hidden")).toBe("true");act(()=>vi.advanceTimersByTime(100));fireEvent.click(summary);act(()=>vi.advanceTimersByTime(300));expect(details.open).toBe(true);expect(body.inert).toBe(false);fireEvent.click(summary);act(()=>vi.advanceTimersByTime(230));expect(details.open).toBe(false);
});
it("减少动态效果时直接完成收起",()=>{
 vi.useFakeTimers();vi.stubGlobal("matchMedia",()=>({matches:true}));render(<MotionDetails><summary>查看记录</summary><p>正文</p></MotionDetails>);const summary=screen.getByText("查看记录");fireEvent.click(summary);fireEvent.click(summary);act(()=>vi.advanceTimersByTime(1));expect(summary.closest("details")!.open).toBe(false);
});
