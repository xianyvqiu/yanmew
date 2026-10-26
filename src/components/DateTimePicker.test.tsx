import {useState} from "react";
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import DateTimePicker from "./DateTimePicker";
beforeEach(()=>{
  HTMLDialogElement.prototype.showModal=function(){this.setAttribute("open","");};
  HTMLDialogElement.prototype.close=function(){this.removeAttribute("open");};
});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
function Fixture({time=false}:{time?:boolean}) {const[value,setValue]=useState(time?"08:20":"2026-10-21T08:20");return <DateTimePicker aria-label="时间值" type={time?"time":"datetime-local"} value={value} onChange={event=>setValue(event.target.value)}/>;}
it("选择日期和时分后确定才回填，取消保留旧值",()=>{
 render(<Fixture/>);fireEvent.click(screen.getByRole("button",{name:"选择提醒时间"}));
 fireEvent.click(screen.getByRole("button",{name:"2026-10-23"}));fireEvent.change(screen.getByLabelText("小时"),{target:{value:"19"}});fireEvent.change(screen.getByLabelText("分钟"),{target:{value:"35"}});
 expect((screen.getByLabelText("时间值") as HTMLInputElement).value).toBe("2026-10-21T08:20");fireEvent.click(screen.getByRole("button",{name:"确定时间"}));
 expect((screen.getByLabelText("时间值") as HTMLInputElement).value).toBe("2026-10-23T19:35");
 fireEvent.click(screen.getByRole("button",{name:"选择提醒时间"}));fireEvent.change(screen.getByLabelText("小时"),{target:{value:"05"}});fireEvent.click(screen.getByRole("button",{name:"取消"}));
 expect((screen.getByLabelText("时间值") as HTMLInputElement).value).toBe("2026-10-23T19:35");
});
it("循环提醒仅回填时分，禁用时不能打开",()=>{
 const{rerender}=render(<Fixture time/>);fireEvent.click(screen.getByRole("button",{name:"选择循环时间"}));fireEvent.change(screen.getByLabelText("小时"),{target:{value:"22"}});fireEvent.click(screen.getByRole("button",{name:"确定时间"}));expect((screen.getByLabelText("时间值") as HTMLInputElement).value).toBe("22:20");
 rerender(<DateTimePicker type="time" value="09:00" disabled/>);expect((screen.getByRole("button",{name:"选择循环时间"}) as HTMLButtonElement).disabled).toBe(true);
});
