import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import ReminderBubble from "./ReminderBubble";
import type { ReminderOccurrence, ReminderPresentation } from "../types";
const {polish,listen} = vi.hoisted(() => ({polish:vi.fn(),listen:vi.fn(async (_callback:()=>void)=>()=>{})}));
vi.mock("../api", () => ({ reminderApi: { polish,listen } }));
const occurrence: ReminderOccurrence = { id:"1",reminderId:"2",name:"喝水",content:"一杯水",scheduledAt:"2026-10-14T01:00:00Z",status:"pending",deliveryState:"queued",createdAt:"2026-10-14T01:00:00Z",completedAt:null };
const props = { occurrence, busy:false,error:"", onDismiss:()=>{},onOpenReminders:()=>{} };
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it("等待AI时立即显示默认词，成功后保留原详情与时间", async () => {
  let resolve!: (value: ReminderPresentation) => void;
  polish.mockReturnValue(new Promise<ReminderPresentation>(finish => { resolve = finish; }));
  render(<ReminderBubble {...props} context={{personaId:"1",contextRevision:0}} />);
  expect(screen.getByText("喝水事件提醒：一杯水")).toBeTruthy();
  await act(async () => { resolve({occurrenceId:"1",personaId:"1",contextRevision:0,text:"该喝水啦。"}); });
  expect(screen.getByText("该喝水啦。")).toBeTruthy(); expect(screen.getByText("提醒详情：一杯水")).toBeTruthy();
  expect(document.querySelector("time")?.getAttribute("dateTime")).toBe(occurrence.scheduledAt);
});
it("润色失败只保留默认词，不阻止知道了操作", async () => {
  polish.mockRejectedValue(new Error("服务不可用"));
  await act(async () => { render(<ReminderBubble {...props} context={{personaId:"1",contextRevision:0}} />); });
  expect(screen.getByText("喝水事件提醒：一杯水")).toBeTruthy();
  expect(screen.queryByRole("alert")).toBeNull(); expect((screen.getByRole("button",{name:"知道了"}) as HTMLButtonElement).disabled).toBe(false);
});
it("旧人设或旧提醒的迟到结果不显示", async () => {
  let finish!: (value: ReminderPresentation) => void;
  polish.mockReturnValueOnce(new Promise<ReminderPresentation>(resolve => { finish = resolve; })).mockResolvedValue(null);
  const view = render(<ReminderBubble {...props} context={{personaId:"1",contextRevision:0}} />);
  await act(async () => { view.rerender(<ReminderBubble {...props} occurrence={{...occurrence,content:"新详情"}} context={{personaId:"2",contextRevision:1}} />); });
  await act(async () => { finish({occurrenceId:"1",personaId:"1",contextRevision:0,text:"旧润色"}); });
  expect(screen.queryByText("旧润色")).toBeNull(); expect(screen.getByText("喝水事件提醒：新详情")).toBeTruthy();
});

it("后台保存事件到来后补读同一句话，不重置弹窗",async()=>{
 let changed!:()=>void;listen.mockImplementationOnce(async callback=>{changed=callback;return()=>{};});
 polish.mockResolvedValueOnce(null).mockResolvedValueOnce({occurrenceId:"1",personaId:"1",contextRevision:0,text:"先喝一杯水呀。"});
 await act(async()=>{render(<ReminderBubble {...props} context={{personaId:"1",contextRevision:0}}/>);});
 expect(screen.getByText("喝水事件提醒：一杯水")).toBeTruthy();await act(async()=>{changed();});expect(screen.getByText("先喝一杯水呀。")).toBeTruthy();expect(polish).toHaveBeenCalledTimes(2);
});
