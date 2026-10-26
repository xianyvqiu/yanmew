import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import PetConversationBubbles from "./PetConversationBubbles";
import type { PetActivity, ProactiveMessageNotice } from "../types";

const api = vi.hoisted(() => ({ listen: vi.fn(), stop: vi.fn(), open: vi.fn() }));
vi.mock("../api", () => ({ petApi: { listenProactiveMessage: api.listen },
  configurationApi: { listenPersonaUpdates: async () => () => {} } }));
vi.mock("../platform/navigation", () => ({ navigation: { open: api.open } }));
vi.mock("./PetStateBubble", () => ({ default: () => <div data-testid="ordinary-bubble" /> }));
let receive: (notice: ProactiveMessageNotice) => void;
const view: PetActivity = {
  personaId: "1", sessionId: "2", contextRevision: 3, revision: 4, activity: "bored",
  expression: "(・ω・)", intimacy: 1, sleepUntil: null, error: null, canInteract: true, canWander: true,
};
const notice: ProactiveMessageNotice = {
  personaId: "1", sessionId: "2", contextRevision: 3,
  message: { id: "9", sessionId: "2", roundId: null, role: "assistant", content: "来聊聊天吧", expression: null, createdAt: "2026-10-06" },
};
beforeEach(() => {
  vi.useFakeTimers({toFake:["setTimeout","clearTimeout","setInterval","clearInterval","performance"]}); vi.clearAllMocks();
  api.open.mockResolvedValue(undefined);
  api.listen.mockImplementation(async callback => { receive = callback; return api.stop; });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("PetConversationBubbles", () => {
  it("展示已保存正文并按默认三分钟收起，超过20秒仍保留且重复事件不延时", async () => {
    await act(async () => { render(<PetConversationBubbles view={view} />); });
    expect(screen.getByTestId("ordinary-bubble")).toBeTruthy();
    act(() => receive(notice));
    expect(screen.getByRole("status").hasAttribute("data-pet-interactive")).toBe(true);
    expect(screen.getByText("来聊聊天吧")).toBeTruthy();
    expect(screen.queryByTestId("ordinary-bubble")).toBeNull();
    act(() => vi.advanceTimersByTime(179000));
    expect(screen.getByRole("status")).toBeTruthy();
    act(() => receive(notice));
    act(() => vi.advanceTimersByTime(1000));
    expect(screen.queryByRole("status")).toBeNull();
    act(() => receive(notice));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("鼠标悬停保留，离开主动消息后重新等待三分钟",async()=>{
    await act(async()=>{render(<PetConversationBubbles view={view}/>);});act(()=>receive(notice));
    fireEvent.pointerEnter(screen.getByRole("status"));act(()=>vi.advanceTimersByTime(300000));expect(screen.getByRole("status")).toBeTruthy();
    fireEvent.pointerLeave(screen.getByRole("status"));act(()=>vi.advanceTimersByTime(179000));expect(screen.getByRole("status")).toBeTruthy();
    act(()=>vi.advanceTimersByTime(1000));expect(screen.queryByRole("status")).toBeNull();
  });
  it("复用宠窗根区域和配置时间，草稿与忙碌结束后重新计时",async()=>{
    const root={current:document.createElement("div")};document.body.append(root.current);
    const options={root,minutes:1,hidden:false,busy:false,hasDraft:true};
    let r!:ReturnType<typeof render>;await act(async()=>{r=render(<PetConversationBubbles view={view} autoCollapse={options}/>);});act(()=>receive(notice));
    act(()=>vi.advanceTimersByTime(120000));expect(screen.getByRole("status")).toBeTruthy();
    r.rerender(<PetConversationBubbles view={view} autoCollapse={{...options,hasDraft:false,busy:true}}/>);
    act(()=>vi.advanceTimersByTime(120000));expect(screen.getByRole("status")).toBeTruthy();
    r.rerender(<PetConversationBubbles view={view} autoCollapse={{...options,hasDraft:false}}/>);
    fireEvent.pointerEnter(root.current);act(()=>vi.advanceTimersByTime(120000));expect(screen.getByRole("status")).toBeTruthy();
    fireEvent.pointerLeave(root.current);act(()=>vi.advanceTimersByTime(59000));expect(screen.getByRole("status")).toBeTruthy();
    act(()=>vi.advanceTimersByTime(1000));expect(screen.queryByRole("status")).toBeNull();r.unmount();root.current.remove();
  });
  it("设置为零时不会自动收起主动消息",async()=>{
    const options={root:{current:document.createElement("div")},minutes:0,hidden:false,busy:false,hasDraft:false};
    await act(async()=>{render(<PetConversationBubbles view={view} autoCollapse={options}/>);});act(()=>receive(notice));
    act(()=>vi.advanceTimersByTime(1800000));expect(screen.getByText("来聊聊天吧")).toBeTruthy();
  });
  it.each(["personaId", "sessionId", "contextRevision"] as const)("拒绝不匹配的 %s", async field => {
    await act(async () => { render(<PetConversationBubbles view={view} />); });
    act(() => receive({ ...notice, [field]: field === "contextRevision" ? 4 : "other" }));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("上下文变化撤下旧提示并拒绝旧事件", async () => {
    let result!: ReturnType<typeof render>;
    await act(async () => { result = render(<PetConversationBubbles view={view} />); });
    act(() => receive(notice));
    result.rerender(<PetConversationBubbles view={{ ...view, contextRevision: 4 }} />);
    expect(screen.queryByRole("status")).toBeNull();
    act(() => receive({ ...notice, message: { ...notice.message, id: "10" } }));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("禁止展示时撤下且恢复不补弹，暂停时收到的消息同样丢弃", async () => {
    let result!: ReturnType<typeof render>;
    await act(async () => { result = render(<PetConversationBubbles view={view} />); });
    act(() => receive(notice));
    result.rerender(<PetConversationBubbles view={{ ...view, canWander: false }} />);
    expect(screen.queryByRole("status")).toBeNull();
    const next = { ...notice, message: { ...notice.message, id: "10" } };
    act(() => receive(next));
    result.rerender(<PetConversationBubbles view={view} />);
    act(() => receive(next));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("查看聊天使用既有导航，失败提示并允许重试", async () => {
    api.open.mockRejectedValueOnce(new Error("unavailable"));
    await act(async () => { render(<PetConversationBubbles view={view} />); });
    act(() => receive(notice));
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "查看聊天" })); });
    expect(api.open).toHaveBeenCalledWith("chat");
    expect(screen.getByRole("alert").textContent).toContain("暂时无法打开聊天");
    expect(screen.getByText("来聊聊天吧")).toBeTruthy();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "查看聊天" })); });
    expect(api.open).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("遮挡卸载释放监听，不在重新挂载时恢复旧消息", async () => {
    let result!: ReturnType<typeof render>;
    const seenMessageIds = new Set<string>();
    await act(async () => { result = render(<PetConversationBubbles view={view} seenMessageIds={seenMessageIds} />); });
    act(() => receive(notice)); result.unmount();
    expect(api.stop).toHaveBeenCalledOnce();
    const oldReceive = receive;
    await act(async () => { render(<PetConversationBubbles view={view} seenMessageIds={seenMessageIds} />); });
    act(() => oldReceive(notice));
    act(() => receive(notice));
    expect(screen.queryByRole("status")).toBeNull();
  });

  it("监听注册迟到时仍释放资源", async () => {
    let resolve!: (stop: () => void) => void;
    api.listen.mockImplementation(() => new Promise<() => void>(done => { resolve = done; }));
    const result = render(<PetConversationBubbles view={view} />);
    result.unmount();
    await act(async () => { resolve(api.stop); });
    expect(api.stop).toHaveBeenCalledOnce();
  });
});
