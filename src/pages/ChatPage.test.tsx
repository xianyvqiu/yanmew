import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import type { Message, MessagePage, Persona, Session } from "../types";
import ChatPage from "./ChatPage";

const api = vi.hoisted(() => ({
  create: vi.fn(),
  select: vi.fn(),
  rename: vi.fn(),
  messages: vi.fn(),
}));

vi.mock("../api", () => ({
  sessionApi: api,
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
  api.create.mockReset();
  api.select.mockReset();
  api.rename.mockReset();
  api.messages.mockReset().mockResolvedValue({ items: [], nextAfterId: null } satisfies MessagePage);
});

afterEach(() => { cleanup(); });

describe("ChatPage", () => {
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
