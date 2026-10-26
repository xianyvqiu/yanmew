import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useChatScroll} from "./useChatScroll";
let height=1000,top=0,resize:ResizeObserverCallback,frame:FrameRequestCallback;
const disconnect=vi.fn(),cancel=vi.fn();
function Harness({scope="one",revision=1,ready=true,first="50"}:{scope?:string;revision?:number;ready?:boolean;first?:string}){
 const scroll=useChatScroll(scope,ready,revision,undefined,first);
 return <section ref={scroll.viewport} aria-label="记录"><div ref={scroll.content}>内容</div></section>;
}
beforeEach(()=>{
 height=1000;top=0;disconnect.mockClear();cancel.mockClear();
 vi.spyOn(HTMLElement.prototype,"scrollHeight","get").mockImplementation(()=>height);
 vi.spyOn(HTMLElement.prototype,"clientHeight","get").mockReturnValue(300);
 vi.spyOn(HTMLElement.prototype,"scrollTop","get").mockImplementation(()=>top);
 vi.spyOn(HTMLElement.prototype,"scrollTop","set").mockImplementation(value=>{top=Math.max(0,Math.min(height-300,value));});
 vi.stubGlobal("ResizeObserver",class{constructor(callback:ResizeObserverCallback){resize=callback;}observe(){}disconnect(){disconnect();}});
 vi.stubGlobal("requestAnimationFrame",vi.fn((callback:FrameRequestCallback)=>{frame=callback;return 1;}));vi.stubGlobal("cancelAnimationFrame",cancel);
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it("首次加载完成和切换会话都显示最新位置",()=>{
 const {rerender}=render(<Harness ready={false}/>);expect(top).toBe(0);rerender(<Harness/>);expect(top).toBe(700);
 top=100;fireEvent.scroll(screen.getByLabelText("记录"));rerender(<Harness scope="two"/>);expect(top).toBe(700);
});
it("用户在远处查看历史时保持位置，接近底部后跟随新增与流式内容",()=>{
 const {rerender}=render(<Harness/>);const box=screen.getByLabelText("记录");top=100;fireEvent.scroll(box);
 height=1200;rerender(<Harness revision={2}/>);expect(top).toBe(100);
 top=820;fireEvent.scroll(box);height=1400;rerender(<Harness revision={3}/>);expect(top).toBe(1100);
});
it("图片或排版延迟增高时也跟随，用户离底部远时不抢位置",()=>{
 render(<Harness/>);height=1300;act(()=>{resize([],{} as ResizeObserver);frame(0);});expect(top).toBe(1000);
 top=300;fireEvent.scroll(screen.getByLabelText("记录"));height=1600;act(()=>{resize([],{} as ResizeObserver);frame(0);});expect(top).toBe(300);
});
it("加载更早消息后保持原本在看的内容锚点",()=>{
 const {rerender}=render(<Harness/>);top=100;fireEvent.scroll(screen.getByLabelText("记录"));height=1400;
 rerender(<Harness first="1" revision={2}/>);expect(top).toBe(500);
});
it("滚轮按像素增量移动，隔离外层滚动，并支持按行的滚轮设备",()=>{
 render(<Harness/>);const box=screen.getByLabelText("记录");top=100;fireEvent.scroll(box);
 const wheel=new WheelEvent("wheel",{deltaY:24,bubbles:true,cancelable:true});act(()=>box.dispatchEvent(wheel));expect(top).toBe(124);expect(wheel.defaultPrevented).toBe(true);
 fireEvent.wheel(box,{deltaY:-2,deltaMode:1});expect(top).toBe(84);
});
it("关闭时释放尺寸观察及待执行滚动",()=>{
 const {unmount}=render(<Harness/>);act(()=>resize([],{} as ResizeObserver));unmount();expect(disconnect).toHaveBeenCalled();expect(cancel).toHaveBeenCalledWith(1);
});
