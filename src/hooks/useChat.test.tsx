import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useChat } from "./useChat";
import type { ChatReplyEvent, ChatRequestSnapshot, CurrentContext, RoundSnapshot } from "../types";

const api = vi.hoisted(() => ({ listen: vi.fn(), current: vi.fn(), submit: vi.fn(), cancel: vi.fn(), messages: vi.fn() }));
vi.mock("../api", () => ({ chatApi: api, sessionApi: api, errorMessage: (error: Error) => error.message }));
const context: CurrentContext = { personaId: "1", sessionId: "1", contextRevision: 1 };
const round: RoundSnapshot = {
  round: { id: "10", sessionId: "1", status: "pending", createdAt: "2026-09-28", completedAt: null },
  userMessage: { id: "20", sessionId: "1", roundId: "10", role: "user", content: "你好", expression: null, createdAt: "2026-09-28" },
  assistantMessage: null,
};
const receipt: ChatRequestSnapshot = { ...context, sessionId: "1", requestId: "5", roundId: "10", seq: 0, partialContent: "", expression: null, round };
const saved: RoundSnapshot = { ...round, round: { ...round.round, status: "completed" }, assistantMessage: { ...round.userMessage, id: "21", role: "assistant", content: "你好喵" } };
let receive: (event: ChatReplyEvent) => void;
const stop = vi.fn();
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
function event(payload: { type: "text"; text: string } | { type: "finished"; finish: "stop" } | { type: "expression"; expression: string }, seq: number, data: RoundSnapshot | null = null): ChatReplyEvent {
  return { ...context, sessionId: "1", requestId: "5", roundId: "10", seq, saved: data, ...payload };
}
beforeEach(() => {
  vi.clearAllMocks();
  api.listen.mockImplementation(async callback => { receive = callback; return stop; });
  api.current.mockResolvedValue(null); api.submit.mockResolvedValue(receipt); api.cancel.mockResolvedValue(round);
  api.messages.mockResolvedValue({ items: [], nextAfterId: null });
});
afterEach(cleanup);

it("回执前的事件完整重放，重复事件与旧上下文不污染正文", async () => {
  const pending = deferred<ChatRequestSnapshot>(); api.submit.mockReturnValue(pending.promise);
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  let send!: Promise<boolean>;
  act(() => { send = hook.result.current.send("你好"); });
  await act(async () => { expect(await hook.result.current.send("重复")).toBe(false); });
  act(() => {
    receive(event({ type: "expression", expression: "(^_^)" }, 1));
    receive(event({ type: "text", text: "你好" }, 2));
    receive(event({ type: "text", text: "你好" }, 2));
    receive({ ...event({ type: "text", text: "旧内容" }, 3), contextRevision: 0 });
  });
  await act(async () => { pending.resolve(receipt); await send; });
  expect(hook.result.current.active?.partialContent).toBe("你好");
  expect(hook.result.current.messages).toHaveLength(1);
  act(() => receive(event({ type: "finished", finish: "stop" }, 3, saved)));
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  expect(hook.result.current.messages.map(message => message.content)).toEqual(["你好", "你好喵"]);
  expect(api.submit).toHaveBeenCalledTimes(1);
});

it("提交失败保留可重试状态，取消等待保存结果", async () => {
  api.submit.mockRejectedValueOnce(new Error("请配置提供商"));
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  await act(async () => { expect(await hook.result.current.send("草稿")).toBe(false); });
  expect(hook.result.current.error).toBe("请配置提供商");
  await act(async () => { await hook.result.current.send("你好"); await hook.result.current.cancel(); });
  expect(hook.result.current.cancelling).toBe(true);
  expect(hook.result.current.messages).toHaveLength(1);
  const interrupted: RoundSnapshot = { ...saved, round: { ...saved.round, status: "interrupted" } };
  await act(async () => receive({ ...event({ type: "finished", finish: "stop" }, 1, interrupted), type: "cancelled" }));
  expect(hook.result.current.busy).toBe(false);
  expect(hook.result.current.notice).toContain("已中断");
});

it("卸载早于回执时精确取消迟到请求并释放监听", async () => {
  const pending = deferred<ChatRequestSnapshot>(); api.submit.mockReturnValue(pending.promise);
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  let send!: Promise<boolean>; act(() => { send = hook.result.current.send("你好"); });
  hook.unmount();
  await act(async () => { pending.resolve(receipt); await send; });
  expect(api.cancel).toHaveBeenCalledWith(receipt);
  expect(stop).toHaveBeenCalledTimes(1);
});

it("聚焦恢复活动快照，终态漏事件后从数据库恢复", async () => {
  api.current.mockResolvedValueOnce({ ...receipt, seq: 2, partialContent: "你好" });
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.active?.partialContent).toBe("你好"));
  api.cancel.mockResolvedValue(saved);
  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(hook.result.current.active).toBeNull());
  expect(hook.result.current.messages[hook.result.current.messages.length - 1]?.content).toBe("你好喵");
  expect(hook.result.current.busy).toBe(false);
});

it("同一上下文对象刷新不取消请求，换会话清理旧请求并隔离迟到事件", async () => {
  api.current.mockResolvedValueOnce(receipt);
  const hook = renderHook(({ value }) => useChat(value), { initialProps: { value: context } });
  await waitFor(() => expect(hook.result.current.active?.requestId).toBe("5"));
  hook.rerender({ value: { ...context } });
  expect(api.cancel).not.toHaveBeenCalled();
  const oldReceive = receive;
  hook.rerender({ value: { ...context, sessionId: "2", contextRevision: 2 } });
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  act(() => oldReceive(event({ type: "text", text: "旧会话" }, 1)));
  expect(hook.result.current.active).toBeNull();
  expect(hook.result.current.messages).toEqual([]);
  expect(api.cancel).toHaveBeenCalledWith(receipt);
});
