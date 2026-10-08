import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PetStateBubble from "./PetStateBubble";
import type { PetActivity, PetStateNotice } from "../types";

const api = vi.hoisted(() => ({ listen: vi.fn(), stop: vi.fn() }));
vi.mock("../api", () => ({ petApi: { listenStateBubble: api.listen } }));
let receive: (notice: PetStateNotice) => void;
const hungry: PetActivity = {
  personaId: "1", sessionId: "1", contextRevision: 0, revision: 2, activity: "hungry",
  expression: "(・ω・)", intimacy: 1, sleepUntil: null, error: null, canInteract: true, canWander: true,
};
const notice: PetStateNotice = {
  personaId: "1", sessionId: "1", contextRevision: 0, revision: 2, activity: "hungry", text: "好饿…",
};

beforeEach(() => {
  vi.useFakeTimers();
  api.stop.mockClear(); api.listen.mockReset();
  api.listen.mockImplementation(async callback => { receive = callback; return api.stop; });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("PetStateBubble", () => {
  it("实时提示自动收起，重复事件不延长显示时间", async () => {
    await act(async () => { render(<PetStateBubble view={hungry} durationMs={8000} />); });
    act(() => receive(notice));
    expect(screen.getByRole("status").textContent).toContain("好饿…");
    act(() => vi.advanceTimersByTime(7000));
    act(() => receive(notice));
    act(() => vi.advanceTimersByTime(1000));
    expect(screen.queryByRole("status")).toBeNull();
    act(() => receive(notice));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("提示先于活动快照到达时等对应版本，不串到旧状态", async () => {
    const normal = { ...hungry, revision: 1, activity: "normal" as const };
    let result: ReturnType<typeof render>;
    await act(async () => { result = render(<PetStateBubble view={normal} durationMs={8000} />); });
    act(() => receive(notice));
    expect(screen.queryByRole("status")).toBeNull();
    result!.rerender(<PetStateBubble view={hungry} durationMs={8000} />);
    expect(screen.getByRole("status")).toBeTruthy();
    result!.rerender(<PetStateBubble view={{ ...hungry, revision: 3, activity: "eating" }} durationMs={8000} />);
    expect(screen.queryByRole("status")).toBeNull();
    result!.rerender(<PetStateBubble view={{ ...hungry, revision: 4 }} durationMs={8000} />);
    act(() => receive(notice));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("切换会话后清除旧提示并拒绝旧会话迟到事件", async () => {
    let result: ReturnType<typeof render>;
    await act(async () => { result = render(<PetStateBubble view={hungry} durationMs={8000} />); });
    act(() => receive(notice));
    const target = { ...hungry, personaId: "2", sessionId: "2", contextRevision: 1, revision: 5, activity: "bored" as const };
    result!.rerender(<PetStateBubble view={target} durationMs={8000} />);
    expect(screen.queryByRole("status")).toBeNull();
    act(() => receive({ ...notice, revision: 6 }));
    expect(screen.queryByRole("status")).toBeNull();
    act(() => receive({ personaId: "2", sessionId: "2", contextRevision: 1, revision: 5, activity: "bored", text: "好无聊…" }));
    expect(screen.getByRole("status").textContent).toContain("好无聊…");
  });

  it("拖动等禁止展示后清除提示，恢复不补弹", async () => {
    let result: ReturnType<typeof render>;
    await act(async () => { result = render(<PetStateBubble view={hungry} durationMs={8000} />); });
    act(() => receive(notice));
    result!.rerender(<PetStateBubble view={{ ...hungry, canWander: false }} durationMs={8000} />);
    expect(screen.queryByRole("status")).toBeNull();
    result!.rerender(<PetStateBubble view={hungry} durationMs={8000} />);
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("卸载释放监听，异步注册晚到也立即释放", async () => {
    let resolve!: (stop: () => void) => void;
    api.listen.mockImplementation(() => new Promise<() => void>(done => { resolve = done; }));
    const result = render(<PetStateBubble view={hungry} durationMs={8000} />);
    result.unmount();
    await act(async () => { resolve(api.stop); });
    expect(api.stop).toHaveBeenCalledOnce();
    act(() => receive?.(notice));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
