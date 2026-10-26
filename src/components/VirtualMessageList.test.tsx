import {cleanup,render} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useRef} from "react";
import VirtualMessageList from "./VirtualMessageList";
const rows=Array.from({length:5000},(_,i)=>({id:String(i+1),role:i%2?"user":"assistant"}));
function Harness({count=5000,pinnedId}:{count?:number;pinnedId?:string}){
 const viewport=useRef<HTMLElement>(null);
 return <section ref={viewport}><VirtualMessageList items={rows.slice(0,count)} viewport={viewport} pinnedId={pinnedId} renderItem={item=><li>{item.id}<input aria-label={`编辑${item.id}`}/></li>}/></section>;
}
beforeEach(()=>{vi.stubGlobal("requestAnimationFrame",vi.fn(()=>1));vi.stubGlobal("cancelAnimationFrame",vi.fn());});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("5000条消息只挂载最近的初始窗口，其他消息使用占位",()=>{
 const {container}=render(<Harness/>);
 expect(container.querySelectorAll("[data-message-id]")).toHaveLength(20);
 expect(container.querySelector('[data-message-id="5000"]')).not.toBeNull();
 expect(container.querySelector('[data-message-id="1"]')).toBeNull();
 expect(container.querySelector('[aria-hidden="true"]')).not.toBeNull();
});
it("离屏编辑项保持挂载，结束编辑后可以回收",()=>{
 const {container,rerender}=render(<Harness pinnedId="1"/>);
 expect(container.querySelector('[data-message-id="1"]')).not.toBeNull();
 expect(container.querySelectorAll("[data-message-id]")).toHaveLength(21);
 rerender(<Harness/>);expect(container.querySelector('[data-message-id="1"]')).toBeNull();
});
it("短会话完整呈现，用户短气泡保持按内容收窄",()=>{
 const {container}=render(<Harness count={12}/>);
 expect(container.querySelectorAll("[data-message-id]")).toHaveLength(12);
 expect((container.querySelector('[data-message-id="2"]') as HTMLElement).style.width).toBe("fit-content");
});
