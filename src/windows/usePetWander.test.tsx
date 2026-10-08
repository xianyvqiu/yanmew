import { act, render, screen } from "@testing-library/react";
import { useRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { usePetWander } from "./usePetWander";
import type { PetActivity, PetWanderLayout, PetWanderTarget } from "../types";

const update = vi.fn(); const stop = vi.fn();
let receive: (target: PetWanderTarget) => void;
vi.mock("../api", () => ({ petApi: {
  updateWanderLayout: (layout: PetWanderLayout) => update(layout),
  listenWander: async (callback: typeof receive) => { receive = callback; return stop; },
} }));
const view: PetActivity = { personaId: "1", sessionId: "1", contextRevision: 0, revision: 1, activity: "normal",
  expression: "(・ω・)", intimacy: 0, sleepUntil: null, error: null, canInteract: true, canWander: true };
function Harness({ current = view, enabled = true, bubble = false }: {current?: PetActivity; enabled?: boolean; bubble?: boolean}) {
  const root = useRef<HTMLDivElement>(null); const face = useRef<HTMLDivElement>(null);
  const style = usePetWander(root, face, current, enabled);
  return <div ref={root} data-testid="canvas" style={style}>
    <div ref={face} data-testid="face" />
    {bubble && <div className="pet-reminder-slot"><span>气泡</span></div>}
  </div>;
}
const flush = async (ms = 20) => { await act(async () => { await Promise.resolve(); vi.advanceTimersByTime(ms); }); };
const latest = () => update.mock.calls[update.mock.calls.length - 1][0] as PetWanderLayout;
const target = (offset = { x: 12, y: 0 }): PetWanderTarget => ({ ...latest(), offset });
beforeEach(() => {
  vi.useFakeTimers(); update.mockReset().mockResolvedValue(undefined); stop.mockReset();
  vi.stubGlobal("requestAnimationFrame", (fn: FrameRequestCallback) => setTimeout(() => fn(0), 16));
  vi.stubGlobal("cancelAnimationFrame", (id: number) => clearTimeout(id));
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
  vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener() {}, removeEventListener() {} }));
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const canvas = this.closest<HTMLElement>("[data-testid=canvas]")!;
    const dx = parseFloat(canvas.style.getPropertyValue("--ym-wander-x")) || 0;
    const dy = parseFloat(canvas.style.getPropertyValue("--ym-wander-y")) || 0;
    const face = this.dataset.testid === "face"; const bubble = this.className === "pet-reminder-slot";
    const x = face ? 100 + dx : bubble ? 12 + dx : 0;
    const y = face ? 120 + dy : bubble ? 44 + dy : 0;
    const width = face ? 120 : bubble ? 296 : 320;
    const height = face ? 80 : bubble ? 104 : 240;
    return { x, y, left: x, top: y, right: x + width, bottom: y + height, width, height, toJSON: () => ({}) } as DOMRect;
  });
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); vi.useRealTimers(); });

describe("临时画布游走", () => {
  it("只改变本地偏移并在动作结束后报告最终位置", async () => {
    render(<Harness />); await flush();
    const initial = latest(); expect(initial.body.x).toBe(100);
    act(() => receive(target())); await flush(600);
    expect(screen.getByTestId("canvas").style.getPropertyValue("--ym-wander-x")).toBe("12px");
    await flush(); expect(latest().offset.x).toBe(12); expect(latest().body.x).toBe(100);
    expect(update.mock.calls.length).toBeLessThanOrEqual(3);
  });
  it("拒绝旧会话、旧布局、非法或越界目标", async () => {
    render(<Harness />); await flush();
    const t = target();
    act(() => {
      receive({ ...t, sessionId: "2" }); receive({ ...t, contextRevision: 1 });
      receive({ ...t, layoutRevision: t.layoutRevision - 1 }); receive({ ...t, offset: { x: 500, y: 0 } });
      receive({ ...t, offset: { x: NaN, y: 0 } });
    });
    await flush(600); expect(screen.getByTestId("canvas").style.getPropertyValue("--ym-wander-x")).toBe("0px");
  });
  it("按住或隐藏时立即停止未完成动画", async () => {
    const r = render(<Harness />); await flush();act(() => receive(target()));await flush(200);
    const held = screen.getByTestId("canvas").style.getPropertyValue("--ym-wander-x");
    r.rerender(<Harness enabled={false} />);await flush(600);
    expect(screen.getByTestId("canvas").style.getPropertyValue("--ym-wander-x")).toBe(held);
    expect(latest().paused).toBe(true);
  });
  it("气泡出现时合并边界并修正超出的偏移", async () => {
    const r = render(<Harness />);await flush();act(() => receive(target()));await flush(620);
    r.rerender(<Harness bubble />);await flush();
    expect(latest().body.width).toBe(296);expect(latest().offset.x).toBe(8);
    expect(screen.getByTestId("canvas").style.getPropertyValue("--ym-wander-x")).toBe("8px");
  });
  it("系统减少动态效果时不执行游走", async () => {
    vi.stubGlobal("matchMedia", () => ({ matches: true, addEventListener() {}, removeEventListener() {} }));
    render(<Harness />);await flush();expect(latest().paused).toBe(true);
    act(() => receive(target()));await flush(600);
    expect(screen.getByTestId("canvas").style.getPropertyValue("--ym-wander-x")).toBe("0px");
  });
  it("卸载释放监听且迟到事件不会开始动画", async () => {
    const r = render(<Harness />);await flush();const t = target();r.unmount();
    expect(stop).toHaveBeenCalledOnce();act(() => receive(t));await flush(600);
    expect(vi.getTimerCount()).toBe(0);
  });
});
