import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import type { ChatReplyEvent, ChatRequestSnapshot, Message, MessagePage, Persona, RoundSnapshot, Session } from "../types";
import ChatPage from "./ChatPage";

const api = vi.hoisted(() => ({
  create: vi.fn(),
  select: vi.fn(),
  rename: vi.fn(),
  messages: vi.fn(),
  current: vi.fn(), lastRound: vi.fn(), continue: vi.fn(), listen: vi.fn(), submit: vi.fn(), cancel: vi.fn(),
}));

vi.mock("../api", () => ({
  sessionApi: api,
  chatApi: api,
  errorMessage: (cause: unknown) => cause instanceof Error ? cause.message : "操作失败",
}));

const persona: Persona = {
  id: "persona-1", name: "小喵", prompt: "", initialPrompt: "",
  selfEvolveEnabled: false, lastEvolvedAt: null, color: "#777777",
  chatBgFile: null, chatBgOpacity: 0, chatBgBlur: 0,
  isActive: true, isBuiltin: true, expressions: null,
};

const firstSession: Session = {
  id: "session-1", personaId: persona.id, userProfileId: null, title: "第一段对话",
  memoryRevision: 0, intimacy: 0, createdAt: "2026-09-28T08:00:00Z",
  updatedAt: "2026-09-28T08:00:00Z", isCurrent: true,
};

const secondSession: Session = { ...firstSession, id: "session-2", title: "第二段对话", isCurrent: false };

function snapshot(sessionId: string | null, contextRevision = 1): WorkspaceContext {
  return {
    context: { personaId: persona.id, sessionId, contextRevision },
    persona,
    sessions: sessionId ? [firstSession, secondSession] : [],
  };
}

function message(id: string, sessionId: string, content: string): Message {
  return {
    id, sessionId, roundId: null, role: "user", content,
    expression: null, createdAt: "2026-09-28T08:01:00Z",
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}

beforeEach(() => {
  api.current.mockReset().mockResolvedValue(null);
  api.lastRound.mockReset().mockResolvedValue(null); api.continue.mockReset();
  api.listen.mockReset().mockResolvedValue(() => {});
  api.submit.mockReset(); api.cancel.mockReset();
  api.create.mockReset();
  api.select.mockReset();
  api.rename.mockReset();
  api.messages.mockReset().mockResolvedValue({ items: [], nextAfterId: null } satisfies MessagePage);
});

afterEach(() => { cleanup(); });

describe("ChatPage", () => {
  it("长度中断恢复继续按钮，续写只显示一份原正文且保留未发送草稿", async () => {
    const current = snapshot("session-1");
    const limited: RoundSnapshot = {
      round: { id: "10", sessionId: "session-1", status: "interrupted", stopReason: "length_limit", createdAt: "2026-09-28", completedAt: null },
      userMessage: { ...message("20", "session-1", "原用户消息"), roundId: "10" },
      assistantMessage: { ...message("21", "session-1", "原回复前缀"), roundId: "10", role: "assistant" },
    };
    const receipt: ChatRequestSnapshot = { ...current.context, sessionId: "session-1", requestId: "new-request", roundId: "10", seq: 0,
      partialContent: "原回复前缀", expression: null, round: { ...limited, round: { ...limited.round, status: "pending", stopReason: null } },
      notice: "本次上下文过长，已减少部分较旧内容；历史消息和记忆仍完整保留。",
      budget: { contextWindow: null, configuredMaxOutputTokens: 32768, effectiveMaxOutputTokens: 4096, estimatedInputTokens: 2000, capacitySource: "unknown", estimated: true, reduced: true } };
    let receive!: (event: ChatReplyEvent) => void;
    api.listen.mockImplementation(async callback => { receive = callback; return () => {}; });
    api.lastRound.mockResolvedValue(limited);
    api.continue.mockResolvedValue(receipt);
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);
    await screen.findByRole("button", { name: "继续生成" });
    expect(screen.getByText("已达到本次输出上限")).toBeTruthy();
    expect(screen.getByText("已达到本次输出上限，已收到的内容已保存。")).toBeTruthy();
    const input = screen.getByLabelText("聊天消息");
    fireEvent.change(input, { target: { value: "还没发送的草稿" } });
    fireEvent.click(screen.getByRole("button", { name: "继续生成" }));
    await screen.findByRole("button", { name: "暂停回复" });
    expect(api.continue).toHaveBeenCalledWith(current.context, "10");
    expect(screen.getAllByText("原回复前缀")).toHaveLength(1);
    expect(screen.getAllByText("原用户消息")).toHaveLength(1);
    expect(input).toHaveProperty("value", "还没发送的草稿");
    expect(screen.getByText(/配置 32768 tokens，实际 4096 tokens/)).toBeTruthy();
    expect(screen.getByText(/上下文容量：未知/)).toBeTruthy();
    expect(screen.getByText(/本次输出因模型能力或剩余上下文容量而降低/)).toBeTruthy();
    act(() => receive({ ...current.context, sessionId: "session-1", requestId: "new-request", roundId: "10", seq: 1, saved: null, type: "text", text: "，后续正文" }));
    expect(screen.getByText("原回复前缀，后续正文")).toBeTruthy();
    expect(screen.queryByText("原回复前缀")).toBeNull();
    const completed: RoundSnapshot = { ...limited, round: { ...limited.round, status: "completed", stopReason: null },
      assistantMessage: { ...limited.assistantMessage!, content: "原回复前缀，后续正文" } };
    api.lastRound.mockResolvedValue(completed);
    await act(async () => receive({ ...current.context, sessionId: "session-1", requestId: "new-request", roundId: "10", seq: 2, saved: completed, type: "finished", finish: "stop" }));
    expect(screen.queryByRole("button", { name: "继续生成" })).toBeNull();
    expect(screen.getAllByText("原回复前缀，后续正文")).toHaveLength(1);
    expect(api.submit).not.toHaveBeenCalled();
  });

  it.each([
    { status: "completed" as const, stopReason: null },
    { status: "interrupted" as const, stopReason: "cancelled" },
  ])("$status / $stopReason 回合不显示继续按钮", async ({ status, stopReason }) => {
    api.lastRound.mockResolvedValue({
      round: { id: "10", sessionId: "session-1", status, stopReason, createdAt: "2026-09-28", completedAt: null },
      userMessage: { ...message("20", "session-1", "用户消息"), roundId: "10" },
      assistantMessage: { ...message("21", "session-1", "已存回复"), roundId: "10", role: "assistant" },
    } satisfies RoundSnapshot);
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await screen.findByText("已存回复");
    expect(screen.queryByRole("button", { name: "继续生成" })).toBeNull();
  });

  it.each([
    ["repetition", "持续重复"],
    ["invalid_encoding", "异常编码"],
    ["timeout", "回复超时"],
    ["tool_limit", "工具调用达到上限或持续无进展"],
  ])("恢复 %s 中断显示原因且不显示继续按钮", async (stopReason, notice) => {
    api.lastRound.mockResolvedValue({
      round: { id: "10", sessionId: "session-1", status: "interrupted", stopReason, createdAt: "2026-09-28", completedAt: null },
      userMessage: { ...message("20", "session-1", "原用户消息"), roundId: "10" },
      assistantMessage: { ...message("21", "session-1", "已存部分回复"), roundId: "10", role: "assistant" },
    });
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await screen.findByText("已存部分回复");
    expect(screen.getByText(new RegExp(notice))).toBeTruthy();
    expect(screen.queryByRole("button", { name: "继续生成" })).toBeNull();
    expect(api.continue).not.toHaveBeenCalled();
  });

  it("发送失败保留草稿并显示原因，不创建本地假消息", async () => {
    api.submit.mockRejectedValue(new Error("请先配置聊天提供商"));
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    const input = screen.getByRole("textbox", { name: "聊天消息" }) as HTMLTextAreaElement;
    await waitFor(() => expect(input.disabled).toBe(false));
    fireEvent.change(input, { target: { value: "我的草稿" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));
    await screen.findByText("请先配置聊天提供商");
    expect(input.value).toBe("我的草稿");
    expect(screen.getByText("这个会话还没有消息。")).toBeTruthy();
  });

  it("新建会话保存完成后才通知刷新", async () => {
    const current = snapshot(null);
    const pending = deferred<Session>();
    const onRefresh = vi.fn();
    api.create.mockReturnValue(pending.promise);
    render(<ChatPage snapshot={current} onRefresh={onRefresh} />);

    fireEvent.click(screen.getByRole("button", { name: "新建会话" }));
    expect(api.create).toHaveBeenCalledWith(current.context);
    expect(onRefresh).not.toHaveBeenCalled();

    await act(async () => { pending.resolve(firstSession); await pending.promise; });
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("切换会话时传入当前上下文及目标会话", async () => {
    const current = snapshot("session-1", 7);
    const onRefresh = vi.fn();
    api.select.mockResolvedValue({ ...current.context, sessionId: "session-2", contextRevision: 8 });
    render(<ChatPage snapshot={current} onRefresh={onRefresh} />);

    fireEvent.change(screen.getByLabelText("当前会话"), { target: { value: "session-2" } });
    expect(api.select).toHaveBeenCalledWith(current.context, "session-2");
    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(1));
  });

  it("加载后续消息时追加新记录且不重复显示重叠项", async () => {
    const current = snapshot("session-1");
    api.messages
      .mockResolvedValueOnce({
        items: [message("message-1", "session-1", "第一条"), message("message-2", "session-1", "第二条")],
        nextAfterId: "message-2",
      } satisfies MessagePage)
      .mockResolvedValueOnce({
        items: [message("message-2", "session-1", "第二条"), message("message-3", "session-1", "第三条")],
        nextAfterId: null,
      } satisfies MessagePage);
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);

    await screen.findByText("第一条");
    fireEvent.click(screen.getByRole("button", { name: "加载后续消息" }));
    await screen.findByText("第三条");

    expect(api.messages).toHaveBeenNthCalledWith(1, current.context);
    expect(api.messages).toHaveBeenNthCalledWith(2, current.context, "message-2");
    expect(screen.getAllByText("第二条")).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "加载后续消息" })).toBeNull();
  });

  it("卸载后迟到的消息和新建结果不影响后续页面，也不触发旧刷新", async () => {
    const oldSnapshot = snapshot("session-1", 1);
    const newSnapshot = snapshot("session-2", 2);
    const oldMessages = deferred<MessagePage>();
    const oldCreate = deferred<Session>();
    const oldRefresh = vi.fn();
    api.messages.mockImplementation((context: WorkspaceContext["context"]) =>
      context.sessionId === "session-1" ? oldMessages.promise : Promise.resolve({
        items: [message("message-new", "session-2", "新会话消息")], nextAfterId: null,
      } satisfies MessagePage));
    api.create.mockReturnValue(oldCreate.promise);

    const oldPage = render(<ChatPage snapshot={oldSnapshot} onRefresh={oldRefresh} />);
    fireEvent.click(screen.getByRole("button", { name: "新建会话" }));
    oldPage.unmount();
    render(<ChatPage snapshot={newSnapshot} onRefresh={vi.fn()} />);
    await screen.findByText("新会话消息");

    await act(async () => {
      oldMessages.resolve({ items: [message("message-old", "session-1", "迟到的旧消息")], nextAfterId: null });
      oldCreate.resolve(firstSession);
      await Promise.all([oldMessages.promise, oldCreate.promise]);
    });
    expect(screen.queryByText("迟到的旧消息")).toBeNull();
    expect(oldRefresh).not.toHaveBeenCalled();
  });
});
