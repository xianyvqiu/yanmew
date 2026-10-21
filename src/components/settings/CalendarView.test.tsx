import { act,cleanup,fireEvent,render,screen,waitFor } from "@testing-library/react";
import { afterEach,beforeEach,expect,it,vi } from "vitest";
import type { CalendarMonth } from "../../types";
import CalendarView from "./CalendarView";
const month=vi.hoisted(()=>vi.fn());
vi.mock("../../api",()=>({festivalsApi:{month,listen:async()=>()=>{}},errorMessage:(e:Error)=>e.message}));
const calendar=(year:number,m:number):CalendarMonth=>({year,month:m,days:Array.from({length:new Date(year,m,0).getDate()},(_,i)=>({date:`${year}-${String(m).padStart(2,"0")}-${String(i+1).padStart(2,"0")}`,lunar:"初一",solarTerm:null,events:[]}))});
const animate=vi.fn((_frames:Keyframe[],_options:KeyframeAnimationOptions)=>({cancel:vi.fn()}));
beforeEach(()=>{
 month.mockReset();month.mockImplementation(async(y:number,m:number)=>calendar(y,m));animate.mockClear();
 vi.stubGlobal("matchMedia",()=>({matches:false}));
 Object.defineProperty(HTMLElement.prototype,"animate",{configurable:true,value:animate});
});
afterEach(()=>{cleanup();vi.unstubAllGlobals();delete (HTMLElement.prototype as unknown as {animate?:unknown}).animate;});
const open=async()=>{const rendered=render(<CalendarView revision="1" onAdd={()=>{}} onEdit={()=>{}}/>);await waitFor(()=>expect(rendered.container.querySelectorAll(".ym-calendar-day").length).toBeGreaterThan(0));return rendered;};
it("下一月从右侧滑入、上一月从左侧滑入，同月刷新不重复动画",async()=>{
 const view=await open();expect(animate).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"下个月"}));await waitFor(()=>expect(animate).toHaveBeenCalledTimes(1));
 expect(animate.mock.calls[0]).toEqual([[{opacity:.4,transform:"translateX(36px)"},{opacity:1,transform:"translateX(0)"}],{duration:240,easing:"cubic-bezier(.22,1,.36,1)"}]);
 fireEvent.click(screen.getByRole("button",{name:"上个月"}));await waitFor(()=>expect(animate).toHaveBeenCalledTimes(2));
 expect(animate.mock.calls[1][0][0].transform).toBe("translateX(-36px)");
 view.rerender(<CalendarView revision="2" onAdd={()=>{}} onEdit={()=>{}}/>);await waitFor(()=>expect(month).toHaveBeenCalledTimes(4));expect(animate).toHaveBeenCalledTimes(2);
});
it("翻月读取失败保留旧日历且不能误选旧日期，重试后恢复",async()=>{
 const view=await open();const oldDate=view.container.querySelector<HTMLButtonElement>(".ym-calendar-day")!.getAttribute("aria-label")!.trim();
 let reject:(e:Error)=>void=()=>{};month.mockImplementationOnce(()=>new Promise((_,r)=>{reject=r;}));
 fireEvent.click(screen.getByRole("button",{name:"下个月"}));expect(screen.getByRole("button",{name:oldDate}).hasAttribute("disabled")).toBe(true);
 await act(async()=>reject(new Error("读取失败")));expect(screen.getByRole("alert").textContent).toContain("读取失败");
 expect(screen.getByRole("button",{name:oldDate})).toBeTruthy();expect(animate).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"重试"}));await waitFor(()=>expect(animate).toHaveBeenCalledTimes(1));
 expect(view.container.querySelector<HTMLButtonElement>(".ym-calendar-day")!.disabled).toBe(false);
});
it("快速翻月丢弃过期响应，月份选择与回到今天也按方向切换",async()=>{
 const view=await open();const now=new Date();let finish:(c:CalendarMonth)=>void=()=>{};
 month.mockImplementationOnce(()=>new Promise(r=>{finish=r;}));fireEvent.click(screen.getByRole("button",{name:"下个月"}));
 fireEvent.change(screen.getByLabelText("查看月份"),{target:{value:"2030-01"}});
 await waitFor(()=>expect(view.container.querySelector(".ym-calendar-days")?.getAttribute("data-month")).toBe("2030-1"));
 const late=new Date(now.getFullYear(),now.getMonth()+1,1);await act(async()=>finish(calendar(late.getFullYear(),late.getMonth()+1)));
 expect(view.container.querySelector(".ym-calendar-days")?.getAttribute("data-month")).toBe("2030-1");
 fireEvent.click(screen.getByRole("button",{name:"回到今天"}));await waitFor(()=>expect(view.container.querySelector(".ym-calendar-days")?.getAttribute("data-month")).toBe(`${now.getFullYear()}-${now.getMonth()+1}`));
 expect(animate.mock.calls[animate.mock.calls.length-1]?.[0][0].transform).toBe("translateX(-36px)");
});
it("减少动态效果时仍切换月份，且保留日期选择功能",async()=>{
 vi.stubGlobal("matchMedia",()=>({matches:true}));const view=await open();
 fireEvent.click(screen.getByRole("button",{name:"下个月"}));await waitFor(()=>expect(month).toHaveBeenCalledTimes(2));
 await waitFor(()=>expect(view.container.querySelector<HTMLButtonElement>(".ym-calendar-day")!.disabled).toBe(false));
 expect(animate).not.toHaveBeenCalled();fireEvent.click(view.container.querySelector<HTMLButtonElement>(".ym-calendar-day")!);
 expect(screen.getByLabelText("所选日期")).toBeTruthy();
});
