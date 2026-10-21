import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import MotionRegion from "./MotionRegion";
afterEach(()=>{cleanup();vi.unstubAllGlobals();vi.restoreAllMocks();delete (HTMLElement.prototype as {animate?:unknown}).animate;});
it("切换动效标识保留已有表单DOM和输入，取消前一次动画",()=>{
    const cancel=vi.fn(),animate=vi.fn(()=>({cancel}));
    HTMLElement.prototype.animate=animate as unknown as typeof HTMLElement.prototype.animate;
    vi.stubGlobal("matchMedia",()=>({matches:false}));
    const view=render(<MotionRegion motionKey="first"><input aria-label="草稿" defaultValue="初始"/></MotionRegion>);
    const input=screen.getByLabelText("草稿");fireEvent.change(input,{target:{value:"还没保存的正文"}});
    view.rerender(<MotionRegion motionKey="second"><input aria-label="草稿" defaultValue="初始"/></MotionRegion>);
    expect(screen.getByLabelText("草稿")).toBe(input);expect((input as HTMLInputElement).value).toBe("还没保存的正文");
    expect(animate).toHaveBeenCalledTimes(2);expect(cancel).toHaveBeenCalledTimes(1);
});
it("系统减少动态效果时不启动Web Animations",()=>{
    const animate=vi.fn();HTMLElement.prototype.animate=animate;
    vi.stubGlobal("matchMedia",()=>({matches:true}));
    render(<MotionRegion motionKey="first">内容</MotionRegion>);expect(animate).not.toHaveBeenCalled();expect(screen.getByText("内容")).toBeTruthy();
});
