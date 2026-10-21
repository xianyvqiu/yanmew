import { act, cleanup, render, screen } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { usePetWander } from "./usePetWander";
import type { PetActivity, PetWanderLayout } from "../types";
const update = vi.hoisted(() => vi.fn());
vi.mock("../api", () => ({ petApi: { updateWanderLayout: update } }));
const view: PetActivity = { personaId: "1", sessionId: "1", contextRevision: 0, revision: 1, activity: "normal", expression: "(・ω・)", intimacy: 0, sleepUntil: null, error: null, canInteract: true, canWander: true };
function Harness({ current = view, enabled = true, bubble = false, dock = "none" }) {
  const root = useRef<HTMLDivElement>(null), face = useRef<HTMLDivElement>(null);
  const style = usePetWander(root, face, current, enabled);
  return <div ref={root} data-testid="canvas" data-dock={dock} style={style}><div ref={face} data-testid="face" />{bubble && <div className="pet-reminder-slot"><span>气泡</span></div>}</div>;
}
const flush = async () => { await act(async () => { vi.advanceTimersByTime(50); await Promise.resolve(); }); };
const latest = () => update.mock.calls[update.mock.calls.length - 1][0] as PetWanderLayout;
beforeEach(() => {
  vi.useFakeTimers(); update.mockReset().mockResolvedValue(undefined);
  vi.stubGlobal("requestAnimationFrame", (fn: FrameRequestCallback) => setTimeout(() => fn(0), 16));
  vi.stubGlobal("cancelAnimationFrame", (id: number) => clearTimeout(id));
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const face = this.dataset.testid === "face", bubble = this.className === "pet-reminder-slot";
    const x = face ? 100 : bubble ? 12 : 0, y = face ? 120 : bubble ? 44 : 0, width = face ? 120 : bubble ? 296 : 320, height = face ? 80 : bubble ? 104 : 280;
    return { x, y, left: x, top: y, right: x + width, bottom: y + height, width, height, toJSON: () => ({}) } as DOMRect;
  });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.useRealTimers(); });
it("向原生层分别报告身体与气泡边界，不再在画布内平移", async () => {
  render(<Harness bubble />); await flush();
  expect(latest().anchor).toEqual({ x: 100, y: 120, width: 120, height: 80 });
  expect(latest().body.width).toBe(296); expect(latest().offset).toEqual({ x: 0, y: 0 });
  expect(latest().paused).toBe(true);
  expect(screen.getByTestId("canvas").style.getPropertyValue("--ym-wander-x")).toBe("0px");
});
it("关闭或上下文变化会报告最新暂停资格，旧会话不会继续使用", async () => {
  const r = render(<Harness />); await flush(); expect(latest().paused).toBe(false);
  r.rerender(<Harness enabled={false} current={{ ...view, sessionId: "2", contextRevision: 3 }} />); await flush();
  expect(latest()).toMatchObject({ paused: true, sessionId: "2", contextRevision: 3 });
});
it("底部布局按身体高度放操作区，稳定布局不产生持续上报循环", async () => {
  const r = render(<Harness dock="rest_lie" />); await flush(); await flush();
  expect(screen.getByTestId("canvas").style.getPropertyValue("--dock-measured-body-height")).toBe("80px");
  const calls = update.mock.calls.length; await flush(); expect(update.mock.calls.length).toBe(calls);
  r.unmount(); expect(vi.getTimerCount()).toBe(0);
});
it("系统减少动态效果时暂停", async () => {
  vi.stubGlobal("matchMedia", () => ({ matches: true, addEventListener() {}, removeEventListener() {} }));
  render(<Harness />); await flush(); expect(latest().paused).toBe(true);
});

it("无会话的人设也更新停靠按钮避让尺寸，但不提交游走请求",async()=>{
  render(<Harness dock="rest_lie" current={{...view,sessionId:null}}/>);await flush();
  expect(screen.getByTestId("canvas").style.getPropertyValue("--dock-measured-body-height")).toBe("80px");
  expect(update).not.toHaveBeenCalled();
});
