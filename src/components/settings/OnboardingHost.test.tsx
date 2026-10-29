import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
const events=vi.hoisted(()=>({emitTo:vi.fn().mockResolvedValue(undefined),listen:vi.fn().mockResolvedValue(()=>{})}));
vi.mock("@tauri-apps/api/event",()=>events);
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:()=>({label:"settings",onMoved:async()=>()=>{},onResized:async()=>()=>{}})}));
vi.mock("../../api",()=>({errorMessage:(e:Error)=>e.message}));
import Host,{guideBridge,OnboardingLayer} from "./OnboardingHost";
const state={token:1,step:0,viewing:true,target:"settings",page:"提供商",title:"认识入口",text:"说明",total:24,busy:false,error:""};
beforeEach(()=>{events.listen.mockResolvedValue(()=>{});events.emitTo.mockResolvedValue(undefined);vi.spyOn(guideBridge,"state").mockResolvedValue(state);vi.spyOn(guideBridge,"show").mockResolvedValue();vi.spyOn(guideBridge,"action").mockResolvedValue();vi.spyOn(HTMLElement.prototype,"getBoundingClientRect").mockImplementation(function(this:HTMLElement){const h=Number(this.dataset.height||120),top=Number(this.dataset.top??40);return {x:40,y:top,left:40,top,right:240,bottom:top+h,width:200,height:h,toJSON:()=>({})};});});afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();events.emitTo.mockClear();});
it("内容异步更换后重新定位高亮，正文没有被设为inert",async()=>{const view=render(<><main className="ym-window-shell__content"><div data-onboarding-content><section className="ym-page-card" data-height="120" key="a">内容</section></div></main><Host/></>);await waitFor(()=>expect(guideBridge.show).toHaveBeenLastCalledWith(state,expect.objectContaining({height:120})));expect(screen.getByRole("main").inert).not.toBe(true);view.rerender(<><main className="ym-window-shell__content"><div data-onboarding-content><section className="ym-page-card" data-height="70" key="b">新内容</section></div></main><Host/></>);await waitFor(()=>expect(guideBridge.show).toHaveBeenLastCalledWith(state,expect.objectContaining({height:70})));});
it("介绍配置正文时禁止提交配置，同时允许恢复控制和滚轮",async()=>{const save=vi.fn(),skip=vi.fn();render(<><main><div data-onboarding-content><section className="ym-page-card"><button onClick={save}>保存模型</button></section></div><button data-onboarding-control onClick={skip}>故障恢复</button></main><Host/></>);await waitFor(()=>expect(guideBridge.show).toHaveBeenCalled());fireEvent.click(screen.getByText("保存模型"));expect(save).not.toHaveBeenCalled();fireEvent.click(screen.getByText("故障恢复"));expect(skip).toHaveBeenCalledTimes(1);expect(fireEvent.wheel(screen.getByText("保存模型"),{deltaY:100})).toBe(true);});
it("显示原生层失败会通知控制窗口，不静默困住用户",async()=>{vi.mocked(guideBridge.show).mockRejectedValue(new Error("区域失败"));render(<><div data-onboarding-content><section className="ym-page-card">正文</section></div><Host/></>);await waitFor(()=>expect(events.emitTo).toHaveBeenCalledWith("settings","onboarding-error",{token:1,step:0,error:"区域失败"}));});

it("连续滚动逐帧更新，原生请求未完成时只提交最新位置",async()=>{
 const frames=new Map<number,FrameRequestCallback>();let id=0;
 vi.stubGlobal("requestAnimationFrame",(fn:FrameRequestCallback)=>{frames.set(++id,fn);return id;});vi.stubGlobal("cancelAnimationFrame",(key:number)=>frames.delete(key));
 const tick=async()=>{await act(async()=>{const queued=[...frames.values()];frames.clear();queued.forEach(fn=>fn(0));});};
 let complete:()=>void=()=>{};vi.mocked(guideBridge.show).mockImplementationOnce(()=>new Promise<void>(resolve=>{complete=resolve;}));
 await act(async()=>{render(<><main className="ym-window-shell__content" data-top="0" data-height="500"><div data-onboarding-content><section className="ym-page-card" data-top="100">内容</section></div></main><Host/></>);});
 await tick();expect(guideBridge.show).toHaveBeenCalledTimes(1);
 const main=screen.getByRole("main"),card=screen.getByText("内容");
 for(const top of [80,60,40]){card.dataset.top=String(top);fireEvent.scroll(main);await tick();}
 expect(guideBridge.show).toHaveBeenCalledTimes(1);
 await act(async()=>{complete();});await tick();expect(guideBridge.show).toHaveBeenLastCalledWith(state,expect.objectContaining({top:40}));expect(guideBridge.show).toHaveBeenCalledTimes(2);
 for(const top of [35,30,25]){card.dataset.top=String(top);fireEvent.scroll(main);await tick();expect(guideBridge.show).toHaveBeenLastCalledWith(state,expect.objectContaining({top}));}
 const count=vi.mocked(guideBridge.show).mock.calls.length;fireEvent.scroll(main);await tick();expect(guideBridge.show).toHaveBeenCalledTimes(count);
});
it("同一步骤的几何更新不重建弹窗、不重播入场或重置阅读位置",async()=>{
 let changed=()=>{};events.listen.mockImplementation(async(name:string,fn:()=>void)=>{if(name==="onboarding-view-changed")changed=fn;return()=>{};});
 const first={state,area:{left:50,top:150,width:200,height:300},bounds:{left:0,top:0,width:900,height:700},panel:{left:500,top:150,width:320,height:300}};
 vi.spyOn(guideBridge,"view").mockResolvedValue(first);
 render(<OnboardingLayer/>);const panel=await screen.findByRole("dialog");panel.scrollTop=48;
 vi.mocked(guideBridge.view).mockResolvedValue({...first,area:{...first.area,top:80}});await act(async()=>{changed();});
 expect(screen.getByRole("dialog")).toBe(panel);expect(panel.scrollTop).toBe(48);
});
