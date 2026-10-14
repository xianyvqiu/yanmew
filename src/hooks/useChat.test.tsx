import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useChat } from "./useChat";
import type { ChatContextBudget, ChatReplyEvent, ChatRequestSnapshot, CurrentContext, MessagePage, RoundSnapshot } from "../types";

const api = vi.hoisted(() => ({ listen: vi.fn(), listenMessages: vi.fn().mockResolvedValue(()=>{}),edit:vi.fn(),deleteTail:vi.fn(),regenerate:vi.fn(),current: vi.fn(), lastRound: vi.fn(), continue: vi.fn(), submit: vi.fn(), cancel: vi.fn(), messages: vi.fn() }));
vi.mock("../api", () => ({ chatApi: api, sessionApi: api, errorMessage: (error: Error) => error.message }));
const context: CurrentContext = { personaId: "1", sessionId: "1", contextRevision: 1 };
const round: RoundSnapshot = {
  round: { id: "10", sessionId: "1", status: "pending", createdAt: "2026-09-28", completedAt: null, stopReason: null },
  userMessage: { id: "20", sessionId: "1", roundId: "10", role: "user", content: "你好", expression: null, createdAt: "2026-09-28" },
  assistantMessage: null,
};
const receipt: ChatRequestSnapshot = { ...context, sessionId: "1", requestId: "5", roundId: "10", seq: 0, partialContent: "", expression: null, round };
const saved: RoundSnapshot = { ...round, round: { ...round.round, status: "completed" }, assistantMessage: { ...round.userMessage, id: "21", role: "assistant", content: "你好喵" } };
const limited: RoundSnapshot = { ...saved, round: { ...saved.round, status: "interrupted", stopReason: "length_limit" } };
const budget: ChatContextBudget = { contextWindow: 8192, configuredMaxOutputTokens: 32768, effectiveMaxOutputTokens: 4096,
  estimatedInputTokens: 2000, capacitySource: "provider", estimated: true, reduced: true };
let receive: (event: ChatReplyEvent) => void;
const stop = vi.fn();
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(done => { resolve = done; }); return { promise, resolve }; }
function event(payload: { type: "text"; text: string } | { type: "finished"; finish: "stop" | "length" } | { type: "expression"; expression: string } | { type: "context_notice"; message: string; budget: ChatContextBudget }, seq: number, data: RoundSnapshot | null = null): ChatReplyEvent {
  return { ...context, sessionId: "1", requestId: "5", roundId: "10", seq, saved: data, ...payload };
}
beforeEach(() => {
  vi.clearAllMocks();
  api.edit.mockReset();api.deleteTail.mockReset();api.regenerate.mockReset();
  api.listen.mockImplementation(async callback => { receive = callback; return stop; });
  api.current.mockResolvedValue(null); api.submit.mockResolvedValue(receipt); api.cancel.mockResolvedValue(round);
  api.lastRound.mockResolvedValue(null); api.continue.mockResolvedValue(receipt);
  api.messages.mockResolvedValue({ items: [], nextAfterId: null });
});
afterEach(cleanup);

it("完成后的旧历史查询不能覆盖随后编辑的较早消息", async () => {
  const earlier = { ...round.userMessage, id: "1", roundId: null, content: "较早的旧正文" };
  api.current.mockResolvedValueOnce(receipt);
  api.messages.mockResolvedValue({ items: [earlier, round.userMessage], nextAfterId: null });
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  const pending = deferred<MessagePage>();
  api.messages.mockReturnValueOnce(pending.promise);
  api.lastRound.mockResolvedValue(saved);
  act(() => receive(event({ type: "finished", finish: "stop" }, 1, saved)));
  await waitFor(() => expect(hook.result.current.busy).toBe(false));
  const updated = { ...earlier, content: "刚保存的新正文" };
  api.edit.mockResolvedValue(updated);
  await act(async () => { expect(await hook.result.current.editMessage(earlier, updated.content)).toBe(true); });
  expect(hook.result.current.messages.find(message => message.id === "1")?.content).toBe(updated.content);
  await act(async () => { pending.resolve({ items: [earlier, saved.userMessage, saved.assistantMessage!], nextAfterId: null }); });
  expect(hook.result.current.messages.find(message => message.id === "1")?.content).toBe(updated.content);
});

it("终态后的旧尾轮查询不能覆盖新编辑的尾轮正文", async () => {
  api.current.mockResolvedValueOnce(receipt);
  api.messages.mockResolvedValue({ items: [saved.userMessage, saved.assistantMessage!], nextAfterId: null });
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  const pending = deferred<RoundSnapshot>();
  api.lastRound.mockReturnValueOnce(pending.promise);
  act(() => receive(event({ type: "finished", finish: "stop" }, 1, saved)));
  await waitFor(() => expect(api.lastRound).toHaveBeenCalledTimes(1));
  const changed = { ...saved, assistantMessage: { ...saved.assistantMessage!, content: "尾轮新正文" } };
  api.edit.mockResolvedValue(changed.assistantMessage);
  api.lastRound.mockResolvedValue(changed);
  await act(async () => { expect(await hook.result.current.editMessage(saved.assistantMessage!, changed.assistantMessage.content)).toBe(true); });
  await act(async () => { pending.resolve(saved); });
  expect(hook.result.current.messages.find(message => message.id === "21")?.content).toBe("尾轮新正文");
  expect(hook.result.current.lastRound).toEqual(changed);
});

it("删除尾轮后迟到的旧历史查询不会重新显示已删除消息", async () => {
  api.current.mockResolvedValueOnce(receipt);
  api.messages.mockResolvedValue({ items: [saved.userMessage, saved.assistantMessage!], nextAfterId: null });
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  const pending = deferred<MessagePage>();
  api.messages.mockReturnValueOnce(pending.promise);
  act(() => receive(event({ type: "finished", finish: "stop" }, 1, saved)));
  api.deleteTail.mockImplementation(async () => {
    api.messages.mockResolvedValue({ items: [], nextAfterId: null }); api.lastRound.mockResolvedValue(null);
  });
  await act(async () => { expect(await hook.result.current.deleteTail(saved.assistantMessage!)).toBe(true); });
  expect(hook.result.current.messages).toEqual([]);
  await act(async () => { pending.resolve({ items: [saved.userMessage, saved.assistantMessage!], nextAfterId: null }); });
  expect(hook.result.current.messages).toEqual([]);
  expect(hook.result.current.lastRound).toBeNull();
});

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
  api.messages.mockResolvedValue({items:[saved.userMessage,saved.assistantMessage!],nextAfterId:null});
  api.lastRound.mockResolvedValue(saved);
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

it("上下文通知校验请求和序号，显示预算且不结束流式请求", async () => {
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  act(() => receive(event({ type: "context_notice", message: "未匹配通知", budget }, 1)));
  expect(hook.result.current.notice).toBe("");
  await act(async () => { await hook.result.current.send("你好"); });
  act(() => {
    receive({ ...event({ type: "context_notice", message: "旧请求通知", budget }, 1), requestId: "old" });
    receive({ ...event({ type: "context_notice", message: "错误回合通知", budget }, 1), roundId: "other" });
    receive(event({ type: "context_notice", message: "已减少较旧内容，历史仍保留。", budget }, 1));
    receive(event({ type: "context_notice", message: "重复通知", budget }, 1));
    receive(event({ type: "text", text: "继续回答" }, 2));
  });
  expect(hook.result.current.active?.requestId).toBe("5");
  expect(hook.result.current.active?.seq).toBe(2);
  expect(hook.result.current.active?.partialContent).toBe("继续回答");
  expect(hook.result.current.notice).toBe("已减少较旧内容，历史仍保留。");
  expect(hook.result.current.budget).toEqual(budget);
  expect(hook.result.current.busy).toBe(true);
  expect(api.cancel).not.toHaveBeenCalled();
  api.lastRound.mockResolvedValue(saved);
  await act(async () => receive(event({ type: "finished", finish: "stop" }, 3, saved)));
  expect(hook.result.current.notice).toContain("历史仍保留");
});

it("漏掉预算事件时从活动快照恢复，并丢弃快照前的重复通知", async () => {
  api.current.mockResolvedValueOnce({ ...receipt, seq: 1, partialContent: "你好", notice: "恢复的容量提示", budget });
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.active?.seq).toBe(1));
  act(() => receive(event({ type: "context_notice", message: "过期提示", budget: { ...budget, effectiveMaxOutputTokens: 1 } }, 1)));
  expect(hook.result.current.notice).toBe("恢复的容量提示");
  expect(hook.result.current.budget?.effectiveMaxOutputTokens).toBe(4096);
  api.current.mockResolvedValue({ ...receipt, seq: 3, partialContent: "你好，恢复", notice: "缩减后继续", budget: { ...budget, effectiveMaxOutputTokens: 2048 } });
  await act(async () => receive(event({ type: "context_notice", message: "序号缺口后的提示", budget }, 3)));
  await waitFor(() => expect(hook.result.current.notice).toBe("缩减后继续"));
  expect(hook.result.current.active?.partialContent).toBe("你好，恢复");
  expect(hook.result.current.budget?.effectiveMaxOutputTokens).toBe(2048);
});

it("长度中断尾轮恢复后可继续，同轮保留前缀并拒绝旧请求与重复增量", async () => {
  api.lastRound.mockResolvedValue(limited);
  const continuing = { ...receipt, requestId: "6", partialContent: "你好喵", round: { ...limited, round: { ...limited.round, status: "pending" as const, stopReason: null } } };
  const pending = deferred<ChatRequestSnapshot>(); api.continue.mockReturnValue(pending.promise);
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.lastRound?.round.stopReason).toBe("length_limit"));
  expect(hook.result.current.messages.map(message => message.content)).toEqual(["你好", "你好喵"]);
  let continuation!: Promise<boolean>;
  act(() => { continuation = hook.result.current.continueReply(); });
  expect(api.continue).toHaveBeenCalledWith(context, "10");
  await act(async () => { expect(await hook.result.current.continueReply()).toBe(false); });
  act(() => {
    receive({ ...event({ type: "text", text: "，续写" }, 1), requestId: "6" });
    receive({ ...event({ type: "text", text: "，续写" }, 1), requestId: "6" });
    receive(event({ type: "finished", finish: "length" }, 4, limited));
  });
  await act(async () => { pending.resolve(continuing); await continuation; });
  expect(hook.result.current.active?.partialContent).toBe("你好喵，续写");
  expect(hook.result.current.messages).toHaveLength(2);
  expect(api.submit).not.toHaveBeenCalled();
  const completed = { ...saved, assistantMessage: { ...saved.assistantMessage!, content: "你好喵，续写" } };
  api.lastRound.mockResolvedValue(completed);
  await act(async () => receive({ ...event({ type: "finished", finish: "stop" }, 2, completed), requestId: "6" }));
  await waitFor(() => expect(hook.result.current.lastRound?.round.status).toBe("completed"));
  expect(hook.result.current.messages.map(message => message.content)).toEqual(["你好", "你好喵，续写"]);
  expect(hook.result.current.busy).toBe(false);
  await act(async () => { expect(await hook.result.current.continueReply()).toBe(false); });
  expect(api.continue).toHaveBeenCalledTimes(1);
});

it("继续失败保留已存前缀，普通暂停回合不允许继续", async () => {
  api.lastRound.mockResolvedValue(limited); api.continue.mockRejectedValue(new Error("提供商不可用"));
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  await act(async () => { expect(await hook.result.current.continueReply()).toBe(false); });
  expect(hook.result.current.messages[1].content).toBe("你好喵");
  expect(hook.result.current.lastRound?.round.stopReason).toBe("length_limit");
  expect(hook.result.current.error).toBe("提供商不可用");
  api.lastRound.mockResolvedValue({ ...limited, round: { ...limited.round, stopReason: "cancelled" } });
  await act(async () => { await hook.result.current.reload(); });
  await act(async () => { expect(await hook.result.current.continueReply()).toBe(false); });
  expect(api.continue).toHaveBeenCalledTimes(1);
});

it("恢复进行中的续写保持前缀，暂停后迟到的旧终态不能覆盖后续继续正文", async () => {
  const continuing = { ...receipt, requestId: "6", seq: 2, partialContent: "你好喵，续写", round: { ...limited, round: { ...limited.round, status: "pending" as const, stopReason: null } } };
  api.current.mockResolvedValueOnce(continuing);
  api.cancel.mockResolvedValueOnce(limited);
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.active?.partialContent).toBe("你好喵，续写"));
  await act(async () => { await hook.result.current.cancel(); });
  api.continue.mockResolvedValue({ ...continuing, requestId: "7", seq: 0 });
  await act(async () => { await hook.result.current.continueReply(); });
  const completed = { ...saved, assistantMessage: { ...saved.assistantMessage!, content: "你好喵，续写完成" } };
  api.lastRound.mockResolvedValue(completed);
  await act(async () => receive({ ...event({ type: "finished", finish: "stop" }, 1, completed), requestId: "7" }));
  await act(async () => receive({ ...event({ type: "finished", finish: "length" }, 3, limited), requestId: "6" }));
  expect(hook.result.current.messages[1].content).toBe("你好喵，续写完成");
});

it.each([
  ["repetition", "持续重复"],
  ["invalid_encoding", "异常编码"],
  ["timeout", "回复超时"],
  ["tool_limit", "工具调用达到上限或持续无进展"],
])("恢复 %s 中断回合显示原因，保留正文且禁止继续", async (stopReason, notice) => {
  const interrupted = { ...saved, round: { ...saved.round, status: "interrupted" as const, stopReason } };
  api.lastRound.mockResolvedValue(interrupted);
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  expect(hook.result.current.notice).toContain(notice);
  expect(hook.result.current.notice).toContain("已收到的内容已保存");
  expect(hook.result.current.messages.map(message => message.content)).toEqual(["你好", "你好喵"]);
  await act(async () => { expect(await hook.result.current.continueReply()).toBe(false); });
  expect(api.continue).not.toHaveBeenCalled();
  api.lastRound.mockResolvedValue(saved);
  await act(async () => { await hook.result.current.reload(); });
  expect(hook.result.current.notice).toBe("");
});

it.each([
  ["CHAT_REPETITION", "repetition", "检测到持续重复，回复已停止"],
  ["CHAT_INVALID_ENCODING", "invalid_encoding", "检测到异常编码，回复已停止"],
  ["CHAT_NO_PROGRESS", "timeout", "回复持续无有效进展，已停止"],
  ["CHAT_FIRST_RESPONSE_TIMEOUT", "timeout", "等待首个有效响应超时，已停止"],
  ["CHAT_TOTAL_TIMEOUT", "timeout", "回复总时长超时，已停止"],
  ["CHAT_TOOL_NO_PROGRESS", "tool_limit", "工具调用持续无进展，已停止"],
])("%s 失败事件显示后端中文消息并保留已存部分正文", async (code, stopReason, message) => {
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.ready).toBe(true));
  await act(async () => { await hook.result.current.send("你好"); });
  const interrupted = { ...saved, round: { ...saved.round, status: "interrupted" as const, stopReason } };
  api.lastRound.mockResolvedValue(interrupted);
  await act(async () => receive({ ...context, sessionId: "1", requestId: "5", roundId: "10", seq: 1,
    saved: interrupted, type: "failed", code, message }));
  expect(hook.result.current.error).toBe(message);
  expect(hook.result.current.notice).toContain("已收到的内容已保存");
  expect(hook.result.current.messages.map(item => item.content)).toEqual(["你好", "你好喵"]);
  expect(hook.result.current.busy).toBe(false);
  await act(async () => { expect(await hook.result.current.continueReply()).toBe(false); });
  expect(api.continue).not.toHaveBeenCalled();
});

it("删除后权威重读清理旧列表且拒绝迟到终态",async()=>{
  api.messages.mockResolvedValue({items:[saved.userMessage,saved.assistantMessage!],nextAfterId:null});
  api.lastRound.mockResolvedValue(saved);
  api.current.mockResolvedValue({...receipt,round:saved,partialContent:"预览",regenerating:true});
  const hook=renderHook(()=>useChat(context));
  await waitFor(()=>expect(hook.result.current.ready).toBe(true));
  api.deleteTail.mockImplementation(async()=>{api.messages.mockResolvedValue({items:[],nextAfterId:null});api.lastRound.mockResolvedValue(null);api.current.mockResolvedValue(null);});
  await act(async()=>{expect(await hook.result.current.deleteTail(saved.assistantMessage!)).toBe(true);});
  act(()=>receive(event({type:"finished",finish:"stop"},3,saved)));
  expect(hook.result.current.messages).toEqual([]);
  expect(hook.result.current.busy).toBe(false);
});
it("取消重说等待终态，保留完整原回合和旧正文",async()=>{
  api.messages.mockResolvedValue({items:[saved.userMessage,saved.assistantMessage!],nextAfterId:null});api.lastRound.mockResolvedValue(saved);
  api.regenerate.mockResolvedValue({...receipt,round:saved,regenerating:true});api.cancel.mockResolvedValue(saved);
  const hook=renderHook(()=>useChat(context));
  await waitFor(()=>expect(hook.result.current.ready).toBe(true));
  await act(async()=>{expect(await hook.result.current.regenerateReply()).toBe(true);});
  act(()=>receive(event({type:"text",text:"新预览"},1)));
  await act(async()=>{await hook.result.current.cancel();});
  expect(hook.result.current.busy).toBe(true);
  expect(hook.result.current.cancelling).toBe(true);
  expect(hook.result.current.messages.find(message=>message.id==="21")?.content).toBe("你好喵");
  await act(async()=>receive({...event({type:"finished",finish:"stop"},2,saved),type:"cancelled"}));
  expect(hook.result.current.busy).toBe(false);
  expect(hook.result.current.notice).toContain("原回复和记忆保留");
});
it("编辑失败不改原消息，成功保留原ID并刷新尾轮",async()=>{
  api.messages.mockResolvedValue({items:[saved.userMessage,saved.assistantMessage!],nextAfterId:null});api.lastRound.mockResolvedValue(saved);
  const hook=renderHook(()=>useChat(context));await waitFor(()=>expect(hook.result.current.ready).toBe(true));
  api.edit.mockRejectedValueOnce(new Error("消息已改变"));
  await act(async()=>{expect(await hook.result.current.editMessage(saved.userMessage,"编辑后的原文")).toBe(false);});
  expect(hook.result.current.messages[0].content).toBe("你好");
  const changed={...saved,userMessage:{...saved.userMessage,content:"编辑后的原文"}};
  api.edit.mockResolvedValue(changed.userMessage);api.lastRound.mockResolvedValue(changed);
  await act(async()=>{expect(await hook.result.current.editMessage(saved.userMessage,"编辑后的原文")).toBe(true);});
  expect(hook.result.current.messages[0]).toEqual(changed.userMessage);
});
it("搜索状态跟随序号并能从活动快照恢复，不误判回复结束", async () => {
  api.current.mockResolvedValue({ ...receipt, seq: 2, webActivity: "searching" });
  const hook = renderHook(() => useChat(context));
  await waitFor(() => expect(hook.result.current.active?.webActivity).toBe("searching"));
  act(() => receive({ ...context, sessionId: "1", requestId: "5", roundId: "10", seq: 3, saved: null, type: "web_activity", phase: "reading" }));
  expect(hook.result.current.active?.webActivity).toBe("reading");
  act(() => receive({ ...context, sessionId: "1", requestId: "5", roundId: "10", seq: 2, saved: null, type: "web_activity", phase: "searching" }));
  expect(hook.result.current.active?.webActivity).toBe("reading");
  act(() => receive({ ...context, sessionId: "1", requestId: "5", roundId: "10", seq: 4, saved: null, type: "web_activity", phase: null }));
  expect(hook.result.current.active?.requestId).toBe("5"); expect(hook.result.current.active?.webActivity).toBeNull();
  act(() => receive(event({ type: "text", text: "已经找到资料" }, 5)));
  expect(hook.result.current.active?.partialContent).toBe("已经找到资料");
});
