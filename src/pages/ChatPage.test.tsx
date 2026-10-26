import { act, cleanup, fireEvent, render as renderReact, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import type { ChatReplyEvent, ChatRequestSnapshot, Message, MessagePage, Persona, RoundSnapshot, Session, Travel } from "../types";
import ChatPage from "./ChatPage";

function render(ui: React.ReactElement) { return renderReact(ui); }
function openSettings() { fireEvent.click(screen.getByRole("button", { name: "会话设置" })); }
function openCurrentSessionDeletion() {
  const current = screen.getByRole("navigation", { name: "会话列表" }).querySelector('button[aria-current="page"]');
  if (!current) throw new Error("缺少当前会话");
  fireEvent.contextMenu(current);
  fireEvent.click(screen.getByRole("menuitem", { name: "删除会话" }));
}

const api = vi.hoisted(() => ({
  create: vi.fn(),
  select: vi.fn(),
  rename: vi.fn(),
  delete: vi.fn(),
  setUserProfile: vi.fn(), setPinned: vi.fn(),
  messages: vi.fn(),
  current: vi.fn(), lastRound: vi.fn(), continue: vi.fn(), listen: vi.fn(), submit: vi.fn(), cancel: vi.fn(),
  edit: vi.fn(), deleteTail: vi.fn(), regenerate: vi.fn(), listenMessages: vi.fn(),
  backgroundGet: vi.fn(), backgroundSave: vi.fn(),
}));

vi.mock("../api", () => ({
  sessionApi: api,
  chatApi: api,
  backgroundApi: { get: api.backgroundGet, save: api.backgroundSave },
  errorMessage: (cause: unknown) => cause instanceof Error ? cause.message : "操作失败",
}));

const travelState = vi.hoisted(() => ({current:null as Travel | null, loading:false, error:"", refresh:vi.fn()}));
const navigation = vi.hoisted(() => ({open:vi.fn(),openLetter:vi.fn(),openSettingsDetail:vi.fn()}));
vi.mock("../hooks/useTravel", () => ({useTravel:()=>travelState}));
vi.mock("../platform/navigation", () => ({navigation}));

const persona: Persona = {
  id: "persona-1", name: "小喵", prompt: "", initialPrompt: "",
  selfEvolveEnabled: false, lastEvolvedAt: null, color: "#777777",
  chatBgFile: null, chatBgOpacity: 0, chatBgBlur: 0,
  isActive: true, isBuiltin: true, expressions: null,
};

const firstSession: Session = {
  id: "session-1", personaId: persona.id, userProfileId: null, title: "第一段对话",
  memoryRevision: 0, intimacy: 0, createdAt: "2026-09-28T08:00:00Z",
  updatedAt: "2026-09-28T08:00:00Z", isCurrent: true, isPinned: false,
};

const secondSession: Session = { ...firstSession, id: "session-2", title: "第二段对话", isCurrent: false };

function snapshot(sessionId: string | null, contextRevision = 1): WorkspaceContext {
  return {
    context: { personaId: persona.id, sessionId, contextRevision },
    persona,
    userProfiles: [],
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
  navigation.openSettingsDetail.mockReset().mockResolvedValue(undefined);
  travelState.current=null; travelState.loading=false; travelState.error="";
  navigation.open.mockReset().mockResolvedValue(undefined); navigation.openLetter.mockReset().mockResolvedValue(undefined);

  api.backgroundGet.mockReset().mockResolvedValue({ record: { personaId: persona.id, file: null, opacity: 0, blur: 0 }, imageDataUrl: null, imageError: null });
  api.backgroundSave.mockReset().mockImplementation(async (_id, input) => ({ ...input.expected, opacity: input.opacity, blur: input.blur }));
  api.current.mockReset().mockResolvedValue(null);
  api.lastRound.mockReset().mockResolvedValue(null); api.continue.mockReset();
  api.listen.mockReset().mockResolvedValue(() => {});
  api.listenMessages.mockReset().mockResolvedValue(() => {});
  api.edit.mockReset(); api.deleteTail.mockReset(); api.regenerate.mockReset();
  api.submit.mockReset();
  api.cancel.mockReset().mockImplementation(async (request: ChatRequestSnapshot) => ({
    ...request.round, round: { ...request.round.round, status: "interrupted", stopReason: "cancelled" },
  } satisfies RoundSnapshot));
  api.create.mockReset();
  api.select.mockReset();
  api.rename.mockReset();
  api.delete.mockReset();
  api.setUserProfile.mockReset(); api.setPinned.mockReset();
  api.messages.mockReset().mockResolvedValue({ items: [], nextAfterId: null } satisfies MessagePage);
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});

afterEach(() => { cleanup(); });

describe("ChatPage", () => {
  it("AI历史渲染Markdown、用户保留标记，双方都复制各自原文且不修改消息", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const original = Object.getOwnPropertyDescriptor(navigator, "clipboard");
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
    try {
      const user = message("1", "session-1", "**用户原文**");
      const assistant = { ...message("2", "session-1", "**AI原文**\n(^_^) [资料](https://example.com)"), role: "assistant" as const };
      api.messages.mockResolvedValue({ items: [user, assistant], nextAfterId: null } satisfies MessagePage);
      render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
      await screen.findByText("**用户原文**");
      const bubbles = within(screen.getByRole("region", { name: "聊天记录" })).getAllByRole("listitem");
      expect(bubbles).toHaveLength(2);
      expect(within(bubbles[1]).getByText("AI原文").tagName).toBe("STRONG");
      fireEvent.click(within(bubbles[0]).getByRole("button", { name: "复制" }));
      await waitFor(() => expect(writeText).toHaveBeenLastCalledWith(user.content));
      fireEvent.click(within(bubbles[1]).getByRole("button", { name: "复制" }));
      await waitFor(() => expect(writeText).toHaveBeenLastCalledWith(assistant.content));
      expect(api.edit).not.toHaveBeenCalled();
      expect(api.deleteTail).not.toHaveBeenCalled();
    } finally {
      if (original) Object.defineProperty(navigator, "clipboard", original);
      else Reflect.deleteProperty(navigator, "clipboard");
    }
  });
  it("用户设定卡片可打开详情并绑定，绑定失败保留弹窗供重试", async () => {
    const current=snapshot("session-1"), refresh=vi.fn();
    current.userProfiles=[{id:"7",name:"日常",content:"喜欢猫咪",createdAt:"2026-10-15",updatedAt:"2026-10-15"}];
    api.setUserProfile.mockRejectedValueOnce(new Error("保存失败")).mockResolvedValueOnce({...firstSession,userProfileId:"7"});
    render(<ChatPage snapshot={current} onRefresh={refresh} />);
    await waitFor(() => expect(screen.getByRole("button",{name:"绑定用户设定"})).toHaveProperty("disabled",false));
    fireEvent.click(screen.getByRole("button",{name:"绑定用户设定"}));
    fireEvent.click(screen.getByRole("button",{name:"查看用户设定详情：日常"}));
    expect(navigation.openSettingsDetail).toHaveBeenCalledWith("userProfile","7");
    expect(api.setUserProfile).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:"绑定 日常"}));
    await screen.findByText("保存失败");
    fireEvent.click(screen.getByRole("button",{name:"绑定 日常"}));
    await waitFor(() => expect(screen.queryByRole("dialog",{name:"绑定用户设定"})).toBeNull());
    expect(api.setUserProfile).toHaveBeenCalledWith(current.context,"session-1","7"); expect(refresh).toHaveBeenCalledOnce();
  });
  it("回复进行中可以保存背景，不中断回复或重建聊天页", async () => {
    const round = completedRound();
    api.current.mockResolvedValue({ requestId: "1", personaId: persona.id, sessionId: "session-1", contextRevision: 1,
      roundId: "10", seq: 1, partialContent: "正在回复", expression: null, notice: null, budget: null, regenerating: false,
      round: { ...round, round: { ...round.round, status: "pending" }, assistantMessage: null } } satisfies ChatRequestSnapshot);
    const refresh = vi.fn();
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={refresh} />);
    openSettings();
    await screen.findByText("正在回复");
    fireEvent.click(screen.getByRole("button", { name: "聊天外观" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "保存外观" }).closest("fieldset")?.disabled).toBe(false));
    fireEvent.change(screen.getByLabelText("背景透明度：0%"), { target: { value: "60" } });
    fireEvent.click(screen.getByRole("button", { name: "保存外观" }));
    await screen.findByText("聊天外观已保存");
    expect(api.backgroundSave).toHaveBeenCalledWith(persona.id, expect.objectContaining({ opacity: 60 }), 1);
    expect(api.cancel).not.toHaveBeenCalled();
    expect(refresh).not.toHaveBeenCalled();
    expect(screen.getByText("正在回复")).not.toBeNull();
  });
  it("回复中通过绑定面板修改，失败后可重试且不预先清除回复",async()=>{
    const current=snapshot("session-1");current.userProfiles=[{id:"7",name:"背景",content:"内容",createdAt:"2026-10-06",updatedAt:"2026-10-06"}];api.setUserProfile.mockRejectedValueOnce(new Error("保存失败，原绑定保留")).mockResolvedValueOnce({...firstSession,userProfileId:"7"});const refresh=vi.fn();render(<ChatPage snapshot={current} onRefresh={refresh}/>);await waitFor(()=>expect(screen.getByRole("button",{name:"绑定用户设定"})).toHaveProperty("disabled",false));fireEvent.click(screen.getByRole("button",{name:"绑定用户设定"}));fireEvent.click(screen.getByRole("button",{name:"绑定 背景"}));await screen.findByText("保存失败，原绑定保留");expect(api.cancel).not.toHaveBeenCalled();expect(refresh).not.toHaveBeenCalled();fireEvent.click(screen.getByRole("button",{name:"绑定 背景"}));await waitFor(()=>expect(refresh).toHaveBeenCalledOnce());expect(api.setUserProfile).toHaveBeenLastCalledWith(current.context,"session-1","7");
  });

  it("切换会话关闭绑定面板并显示目标会话绑定名称",async()=>{
    const current=snapshot("session-1");current.userProfiles=[{id:"7",name:"背景",content:"内容",createdAt:"2026-10-06",updatedAt:"2026-10-06"}];const view=render(<ChatPage snapshot={current} onRefresh={vi.fn()}/>);await waitFor(()=>expect(screen.getByRole("button",{name:"绑定用户设定"})).toHaveProperty("disabled",false));fireEvent.click(screen.getByRole("button",{name:"绑定用户设定"}));const next={...current,context:{...current.context,sessionId:"session-2",contextRevision:2},sessions:[firstSession,{...secondSession,userProfileId:"7"}]};view.rerender(<ChatPage snapshot={next} onRefresh={vi.fn()}/>);await waitFor(()=>expect(screen.queryByRole("dialog",{name:"绑定用户设定"})).toBeNull());expect(api.setUserProfile).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"第二段对话 · 用户设定：背景"})).toBeTruthy();
  });

  it("新会话可选择全局设定，列表展示绑定名称且设定删除后恢复未绑定", async () => {
    const current = snapshot("session-1");
    current.userProfiles = [{ id: "7", name: "我的背景", content: "设定正文", createdAt: "2026-10-06", updatedAt: "2026-10-06" }];
    current.sessions = [{ ...firstSession, userProfileId: "7" }, secondSession];
    const refresh = vi.fn();
    api.create.mockResolvedValue({ ...firstSession, id: "new-session", userProfileId: "7" });
    const view = render(<ChatPage snapshot={current} onRefresh={refresh} />);
    await waitFor(()=>expect(screen.getByRole("button",{name:"绑定用户设定"})).toHaveProperty("disabled",false));fireEvent.click(screen.getByRole("button",{name:"绑定用户设定"}));
    await waitFor(() => expect(screen.getByRole("button", { name: "发送" })).toBeTruthy());
    expect(screen.getByRole("button", { name: "第一段对话 · 用户设定：我的背景" })).toBeTruthy();
    fireEvent.change(screen.getByLabelText("新会话的用户设定"), { target: { value: "7" } });
    fireEvent.click(within(screen.getByRole("dialog",{name:"绑定用户设定"})).getByRole("button",{name:"关闭"}));
    fireEvent.click(screen.getByRole("button", { name: "新建会话" }));
    await waitFor(() => expect(api.create).toHaveBeenCalledWith(current.context, "7"));
    await waitFor(() => expect(refresh).toHaveBeenCalledOnce());
    view.rerender(<ChatPage snapshot={{ ...current, userProfiles: [], sessions: [firstSession, secondSession] }} onRefresh={refresh} />);
    expect(screen.getByRole("button", { name: "第一段对话 · 用户设定：未绑定" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button",{name:"绑定用户设定"}));
    await waitFor(() => expect(screen.getByLabelText("新会话的用户设定")).toHaveProperty("value", ""));
  });

  function completedRound(): RoundSnapshot {
    return {
      round: { id: "10", sessionId: "session-1", status: "completed", stopReason: null, createdAt: "2026-09-28", completedAt: "2026-09-28" },
      userMessage: { ...message("20", "session-1", "原用户消息"), roundId: "10" },
      assistantMessage: { ...message("21", "session-1", "原完整回复"), roundId: "10", role: "assistant" },
    };
  }

  it("全历史用户与 AI 可编辑，失败保留非空草稿且成功只替换原文", async () => {
    const round = completedRound();
    api.lastRound.mockResolvedValue(round);
    api.edit.mockRejectedValueOnce(new Error("消息已变化，请重试")).mockResolvedValueOnce({ ...round.userMessage, content: "修改后的用户原文" });
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await screen.findByText("原完整回复");
    expect(screen.getAllByRole("button", { name: "编辑" })).toHaveLength(2);
    fireEvent.click(screen.getAllByRole("button", { name: "编辑" })[0]);
    expect(screen.getByText(/短期记忆、长期记忆和旅行游记不会同步修改/)).toBeTruthy();
    const input = screen.getByLabelText("编辑消息正文");
    fireEvent.change(input, { target: { value: "   " } });
    expect(screen.getByRole("button", { name: "保存消息" })).toHaveProperty("disabled", true);
    fireEvent.change(input, { target: { value: "修改后的用户原文" } });
    fireEvent.click(screen.getByRole("button", { name: "保存消息" }));
    await screen.findByText("消息已变化，请重试");
    expect(input).toHaveProperty("value", "修改后的用户原文");
    expect(api.edit).toHaveBeenCalledWith(snapshot("session-1").context, round.userMessage, "修改后的用户原文");
    const edited = { ...round.userMessage, content: "修改后的用户原文" };
    api.lastRound.mockResolvedValue({ ...round, userMessage: edited });
    api.messages.mockResolvedValue({ items: [edited, round.assistantMessage!], nextAfterId: null });
    fireEvent.click(screen.getByRole("button", { name: "保存消息" }));
    await waitFor(() => expect(screen.queryByLabelText("编辑消息正文")).toBeNull());
    expect(screen.getByText("修改后的用户原文")).toBeTruthy();
    expect(screen.getByText("原完整回复")).toBeTruthy();
  });

  it.each(["persona", "session", "revision"])("编辑期间 %s 变化清除旧草稿且不提交", async changed => {
    api.lastRound.mockResolvedValue(completedRound());
    const page = render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await screen.findByText("原完整回复");
    fireEvent.click(screen.getAllByRole("button", { name: "编辑" })[0]);
    fireEvent.change(screen.getByLabelText("编辑消息正文"), { target: { value: "旧会话草稿" } });
    api.lastRound.mockResolvedValue(null);
    const next = snapshot(changed === "session" ? "session-2" : "session-1", changed === "revision" ? 2 : 1);
    if (changed === "persona") next.context.personaId = "persona-2";
    await act(async () => { page.rerender(<ChatPage snapshot={next} onRefresh={vi.fn()} />); });
    expect(screen.queryByLabelText("编辑消息正文")).toBeNull();
    expect(api.edit).not.toHaveBeenCalled();
  });

  it.each(["completed", "interrupted"] as const)("尚有更早分页时 %s 最新回合仍可按状态操作", async status => {
    const complete = completedRound();
    const round = { ...complete, round: { ...complete.round, status, stopReason: status === "interrupted" ? "length_limit" : null } };
    api.lastRound.mockResolvedValue(round);
    api.messages.mockResolvedValue({ items: [round.userMessage, round.assistantMessage!], nextAfterId: "21" });
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await screen.findByText("原完整回复");
    expect(screen.getByRole("button", { name: "加载更早消息" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "删除本轮" })).toBeTruthy();
    if(status==="completed"){expect(screen.getByRole("button",{name:"重说"})).toBeTruthy();expect(screen.queryByRole("button",{name:"继续生成"})).toBeNull();}
    else{expect(screen.getByRole("button",{name:"继续生成"})).toBeTruthy();expect(screen.queryByRole("button",{name:"重说"})).toBeNull();}
  });

  it("初始教程按一条真实消息展示，沿用底部删除且不显示回合操作", async () => {
    const current = snapshot("session-1");
    current.sessions = [{ ...firstSession, title: "初次见面" }];
    const guide = { ...message("1", "session-1", "欢迎来到颜喵，我是电子小猫 (・ω・)\n\n先配置好聊天模型，再和我打个招呼吧！"), role: "assistant" as const };
    api.messages.mockResolvedValue({ items: [guide], nextAfterId: null });
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);
    await screen.findByText(/欢迎来到颜喵，我是电子小猫/);
    expect(screen.queryByRole("button", { name: "重说" })).toBeNull();
    expect(screen.queryByRole("button", { name: "继续生成" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    const modal = within(screen.getByRole("dialog", { name: "删除消息？" }));
    expect(modal.getByText(/只删除底部这一条消息.*不修改已有记忆/)).toBeTruthy();
    fireEvent.click(modal.getByRole("button", { name: "确认删除" }));
    await waitFor(() => expect(api.deleteTail).toHaveBeenCalledWith(current.context, guide));
  });

  it("底部主动消息只允许删除单条，不能越过它删除或重说旧回合", async () => {
    const round = completedRound();
    const proactive = { ...message("22", "session-1", "主动问候"), role: "assistant" as const };
    api.lastRound.mockResolvedValue(round);
    api.messages.mockResolvedValue({ items: [round.userMessage, round.assistantMessage!, proactive], nextAfterId: null });
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await screen.findByText("主动问候");
    expect(screen.queryByRole("button", { name: "删除本轮" })).toBeNull();
    expect(screen.queryByRole("button", { name: "重说" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    const modal = within(screen.getByRole("dialog", { name: "删除消息？" }));
    expect(modal.getByText(/只删除底部这一条消息.*不修改已有记忆/)).toBeTruthy();
    fireEvent.click(modal.getByRole("button", { name: "取消" }));
    expect(api.deleteTail).not.toHaveBeenCalled();
  });

  it("完整回合删除说明记忆回退，失败保留确认，提交中防重复和 Escape", async () => {
    const current = snapshot("session-1");
    const round = completedRound();
    const pending = deferred<void>();
    api.lastRound.mockResolvedValue(round);
    api.deleteTail.mockRejectedValueOnce(new Error("底部消息已变化")).mockReturnValueOnce(pending.promise);
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);
    await screen.findByRole("button", { name: "删除本轮" });
    fireEvent.click(screen.getByRole("button", { name: "删除本轮" }));
    const modal = screen.getByRole("dialog", { name: "删除本轮？" });
    expect(within(modal).getByText(/对应短期记忆.*回退长期记忆/)).toBeTruthy();
    fireEvent.click(within(modal).getByRole("button", { name: "确认删除" }));
    await waitFor(() => expect(within(modal).getByRole("alert").textContent).toBe("底部消息已变化"));
    expect(screen.getByRole("dialog", { name: "删除本轮？" })).toBe(modal);
    fireEvent.click(within(modal).getByRole("button", { name: "确认删除" }));
    const confirm = within(modal).getByRole("button", { name: "删除中…" });
    expect(confirm).toHaveProperty("disabled", true);
    expect(within(modal).getByRole("button", { name: "取消" })).toHaveProperty("disabled", true);
    fireEvent.click(confirm);
    const escape = new Event("cancel", { cancelable: true });
    fireEvent(modal, escape);
    expect(escape.defaultPrevented).toBe(true);
    expect(api.deleteTail).toHaveBeenCalledTimes(2);
    expect(api.deleteTail).toHaveBeenLastCalledWith(current.context, round.assistantMessage);
    api.lastRound.mockResolvedValue(null);
    await act(async () => { pending.resolve(); await pending.promise; });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("生成中禁止编辑但允许确认尾部删除，中断删除不改变已有记忆", async () => {
    const current = snapshot("session-1");
    const complete = completedRound();
    const round = { ...complete, round: { ...complete.round, status: "pending" as const }, assistantMessage: null };
    api.current.mockResolvedValue({ ...current.context, sessionId: "session-1", requestId: "request-1", roundId: "10", seq: 0,
      partialContent: "正在回复", expression: null, round } satisfies ChatRequestSnapshot);
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);
    await screen.findByRole("button", { name: "暂停回复" });
    expect(screen.getByRole("button", { name: "编辑" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("button", { name: "删除" }));
    const modal = within(screen.getByRole("dialog", { name: "删除本轮？" }));
    expect(modal.getByText(/中断回合不生成记忆.*不修改已有记忆/)).toBeTruthy();
    expect(modal.getByText(/删除前会中断正在进行的回复/)).toBeTruthy();
    expect(modal.getByRole("button", { name: "确认删除" })).toHaveProperty("disabled", false);
  });

  it("上下文版本变化关闭旧消息删除确认，不提交旧目标", async () => {
    api.lastRound.mockResolvedValue(completedRound());
    const page = render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await screen.findByRole("button", { name: "删除本轮" });
    fireEvent.click(screen.getByRole("button", { name: "删除本轮" }));
    expect(screen.getByRole("dialog", { name: "删除本轮？" })).toBeTruthy();
    await act(async () => { page.rerender(<ChatPage snapshot={snapshot("session-1", 2)} onRefresh={vi.fn()} />); });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.deleteTail).not.toHaveBeenCalled();
  });

  it("重说流式预览保留原完整回复并提示原子替换与暂停保留", async () => {
    const current = snapshot("session-1");
    const round = completedRound();
    api.lastRound.mockResolvedValue(round);
    api.regenerate.mockResolvedValue({ ...current.context, sessionId: "session-1", requestId: "redo-request", roundId: "10", seq: 0,
      partialContent: "重说的新正文", expression: null, round, regenerating: true } satisfies ChatRequestSnapshot);
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);
    await screen.findByRole("button", { name: "重说" });
    fireEvent.click(screen.getByRole("button", { name: "重说" }));
    await screen.findByText("重说的新正文");
    expect(api.regenerate).toHaveBeenCalledWith(current.context, "10");
    expect(screen.getByText("原完整回复")).toBeTruthy();
    expect(screen.getByText(/正在准备回复和记忆，完成后一起替换；中断保留旧结果/)).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "编辑" }).every(button => (button as HTMLButtonElement).disabled)).toBe(true);
    expect(screen.getByRole("button", { name: "暂停回复" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "重说" })).toBeNull();
  });

  it("会话设置不重复显示绑定与名称表单，自动标题仍同步列表",async()=>{
    const current=snapshot("session-1");const view=render(<ChatPage snapshot={current} onRefresh={vi.fn()}/>);openSettings();expect(screen.queryByLabelText("会话名称")).toBeNull();expect(screen.queryByLabelText("当前会话的用户设定")).toBeNull();expect(screen.queryByLabelText("新会话的用户设定")).toBeNull();view.rerender(<ChatPage snapshot={{...current,sessions:current.sessions.map(item=>item.id==="session-1"?{...item,title:"AI 自动标题"}:item)}} onRefresh={vi.fn()}/>);expect(screen.getByRole("button",{name:/AI 自动标题/})).toBeTruthy();
  });

  it("删除会话确认说明级联与保留范围，取消不调用删除接口", async () => {
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    await waitFor(() => expect(screen.getByRole("textbox", { name: "聊天消息" })).toHaveProperty("disabled", false));
    openCurrentSessionDeletion();
    const modal = within(screen.getByRole("dialog", { name: "删除会话？" }));
    expect(modal.getByText(/第一段对话.*此操作不可恢复/)).toBeTruthy();
    expect(modal.getByText(/聊天回合与消息、短期和长期记忆及来源、记事本、出行与旅行游记、饰品、信件和伴手礼/)).toBeTruthy();
    expect(modal.getByText(/全局自定义日期会保留/)).toBeTruthy();
    expect(modal.getByText(/全局喵币钱包、账单、提醒.*小游戏插件、授权、设置、存档和游戏局仍会保留/)).toBeTruthy();
    expect(modal.getByText(/最近更新的剩余会话.*不会自动创建新会话/)).toBeTruthy();
    fireEvent.click(modal.getByRole("button", { name: "取消" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.delete).not.toHaveBeenCalled();
  });

  it.each(["session-2", null])("删除成功后重读权威上下文（当前会话 %s）", async currentSessionId => {
    const current = snapshot("session-1", 7);
    const pending = deferred<{ currentSessionId: string | null; contextRevision: number }>();
    const onRefresh = vi.fn();
    api.delete.mockReturnValue(pending.promise);
    const page = render(<ChatPage snapshot={current} onRefresh={onRefresh} />);
    await waitFor(() => expect(screen.getByRole("textbox", { name: "聊天消息" })).toHaveProperty("disabled", false));
    openCurrentSessionDeletion();
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
    expect(api.delete).toHaveBeenCalledWith(current.context, "session-1");
    expect(onRefresh).not.toHaveBeenCalled();
    await act(async () => { pending.resolve({ currentSessionId, contextRevision: 8 }); await pending.promise; });
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).toBeNull();
    await act(async () => { page.rerender(<ChatPage snapshot={snapshot(currentSessionId, 8)} onRefresh={onRefresh} />); });
    if (currentSessionId) expect(screen.getByRole("button", { name: "第二段对话 · 用户设定：未绑定" }).getAttribute("aria-current")).toBe("page");
    if (!currentSessionId) {
      expect(screen.queryByRole("button", { name: "删除会话" })).toBeNull();
      expect(screen.getByText(/创建一个会话/)).toBeTruthy();
    }
    expect(api.create).not.toHaveBeenCalled();
  });

  it("删除失败保留确认框和错误，恢复取消操作", async () => {
    api.delete.mockRejectedValue(new Error("会话删除失败，请重试"));
    const onRefresh = vi.fn();
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={onRefresh} />);
    openCurrentSessionDeletion();
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
    await screen.findByText("会话删除失败，请重试");
    const modal = within(screen.getByRole("dialog", { name: "删除会话？" }));
    expect(modal.getByRole("alert").textContent).toBe("会话删除失败，请重试");
    expect(modal.getByRole("button", { name: "确认删除" })).toHaveProperty("disabled", false);
    expect(onRefresh).not.toHaveBeenCalled();
    fireEvent.click(modal.getByRole("button", { name: "取消" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("删除处理中禁止重复确认、取消及 Escape，普通 Escape 关闭确认", async () => {
    const pending = deferred<{ currentSessionId: string | null; contextRevision: number }>();
    api.delete.mockReturnValue(pending.promise);
    render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
    openCurrentSessionDeletion();
    fireEvent(screen.getByRole("dialog"), new Event("cancel", { cancelable: true }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.delete).not.toHaveBeenCalled();
    openCurrentSessionDeletion();
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
    const modal = screen.getByRole("dialog");
    const confirm = within(modal).getByRole("button", { name: "删除中…" });
    expect(confirm).toHaveProperty("disabled", true);
    expect(within(modal).getByRole("button", { name: "取消" })).toHaveProperty("disabled", true);
    fireEvent.click(confirm);
    const escape = new Event("cancel", { cancelable: true });
    fireEvent(modal, escape);
    expect(escape.defaultPrevented).toBe(true);
    expect(screen.getByRole("dialog")).toBe(modal);
    expect(api.delete).toHaveBeenCalledTimes(1);
    await act(async () => { pending.resolve({ currentSessionId: null, contextRevision: 2 }); await pending.promise; });
  });

  it.each(["persona", "session", "revision"])("确认期间 %s 变化使旧删除目标作废", async changed => {
    const current = snapshot("session-1");
    const page = render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);
    await waitFor(() => expect(screen.getByRole("textbox", { name: "聊天消息" })).toHaveProperty("disabled", false));
    openCurrentSessionDeletion();
    const next = snapshot(changed === "session" ? "session-2" : "session-1", changed === "revision" ? 2 : 1);
    if (changed === "persona") next.context.personaId = "persona-2";
    await act(async () => { page.rerender(<ChatPage snapshot={next} onRefresh={vi.fn()} />); });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(api.delete).not.toHaveBeenCalled();
  });

  it("流式删除先发取消，取消回执仍 pending 时直接调用删除", async () => {
    const current = snapshot("session-1");
    const round: RoundSnapshot = {
      round: { id: "10", sessionId: "session-1", status: "pending", stopReason: null, createdAt: "2026-09-28", completedAt: null },
      userMessage: { ...message("20", "session-1", "用户消息"), roundId: "10" }, assistantMessage: null,
    };
    api.current.mockResolvedValue({ ...current.context, sessionId: "session-1", requestId: "request-1", roundId: "10", seq: 0,
      partialContent: "正在回复", expression: null, round } satisfies ChatRequestSnapshot);
    const cancelling = deferred<RoundSnapshot>();
    api.cancel.mockReturnValue(cancelling.promise);
    api.delete.mockResolvedValue({ currentSessionId: null, contextRevision: 2 });
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);
    await screen.findByRole("button", { name: "暂停回复" });
    openCurrentSessionDeletion();
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
    expect(api.cancel).toHaveBeenCalledTimes(1);
    expect(api.delete).not.toHaveBeenCalled();
    await act(async () => { cancelling.resolve(round); await cancelling.promise; });
    expect(api.delete).toHaveBeenCalledWith(current.context, "session-1");
    expect(api.cancel.mock.invocationCallOrder[0]).toBeLessThan(api.delete.mock.invocationCallOrder[0]);
  });

  it("等待聊天取消时上下文改变，不再提交旧会话删除", async () => {
    const current = snapshot("session-1");
    const round: RoundSnapshot = {
      round: { id: "10", sessionId: "session-1", status: "pending", stopReason: null, createdAt: "2026-09-28", completedAt: null },
      userMessage: { ...message("20", "session-1", "用户消息"), roundId: "10" }, assistantMessage: null,
    };
    api.current.mockResolvedValue({ ...current.context, sessionId: "session-1", requestId: "request-1", roundId: "10", seq: 0,
      partialContent: "正在回复", expression: null, round } satisfies ChatRequestSnapshot);
    const cancelling = deferred<RoundSnapshot>();
    const onRefresh = vi.fn();
    api.cancel.mockReturnValue(cancelling.promise);
    const page = render(<ChatPage snapshot={current} onRefresh={onRefresh} />);
    await screen.findByRole("button", { name: "暂停回复" });
    openCurrentSessionDeletion();
    fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
    api.current.mockResolvedValue(null);
    page.rerender(<ChatPage snapshot={snapshot("session-2", 2)} onRefresh={onRefresh} />);
    await act(async () => { cancelling.resolve(round); await cancelling.promise; });
    expect(api.delete).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onRefresh).not.toHaveBeenCalled();
  });

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

    if (screen.queryByRole("dialog", { name: "会话设置" })) fireEvent.click(screen.getByRole("button", { name: "关闭会话设置" }));
    fireEvent.click(screen.getByRole("button", { name: "新建会话" }));
    expect(api.create).toHaveBeenCalledWith(current.context, null);
    expect(onRefresh).not.toHaveBeenCalled();

    await act(async () => { pending.resolve(firstSession); await pending.promise; });
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });

  it("切换会话时传入当前上下文及目标会话", async () => {
    const current = snapshot("session-1", 7);
    const onRefresh = vi.fn();
    api.select.mockResolvedValue({ ...current.context, sessionId: "session-2", contextRevision: 8 });
    render(<ChatPage snapshot={current} onRefresh={onRefresh} />);

    fireEvent.click(screen.getByRole("button", { name: "第二段对话 · 用户设定：未绑定" }));
    expect(api.select).toHaveBeenCalledWith(current.context, "session-2");
    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(1));
  });

  it("先读最近页，加载更早消息后按时间排序且不重复显示重叠项", async () => {
    const current = snapshot("session-1");
    api.messages
      .mockResolvedValueOnce({
        items: [message("2", "session-1", "第二条"), message("3", "session-1", "第三条")],
        nextAfterId: "2",
      } satisfies MessagePage)
      .mockResolvedValueOnce({
        items: [message("1", "session-1", "第一条"), message("2", "session-1", "第二条")],
        nextAfterId: null,
      } satisfies MessagePage);
    render(<ChatPage snapshot={current} onRefresh={vi.fn()} />);

    await screen.findByText("第三条");
    expect(screen.queryByText("第一条")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "加载更早消息" }));
    await screen.findByText("第一条");

    expect(api.messages).toHaveBeenNthCalledWith(1, current.context,null,100,true);
    expect(api.messages).toHaveBeenNthCalledWith(2, current.context, "2",100,true);
    expect(screen.getAllByText("第二条")).toHaveLength(1);
    expect(screen.queryByRole("button", { name: "加载更早消息" })).toBeNull();
    expect(screen.getAllByRole("listitem").filter(item=>item.hasAttribute("data-role")).map(item=>item.textContent)).toEqual([expect.stringContaining("第一条"),expect.stringContaining("第二条"),expect.stringContaining("第三条")]);
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
    if (screen.queryByRole("dialog", { name: "会话设置" })) fireEvent.click(screen.getByRole("button", { name: "关闭会话设置" }));
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

it("出门时禁止普通聊天并按当前出行打开写信", async () => {
  travelState.current={id:"3",sessionId:"session-1",status:"away",startedAt:"2026-10-09T00:00:00Z",plannedReturnAt:"2099-10-09T01:00:00Z",actualReturnAt:null,cancelledAt:null,midwayLetterAt:"2026-10-09T00:30:00Z",destination:"湖边",factsState:"ready",midwayLetterState:"done",returnLetterState:"pending",giftState:"pending",journalState:"pending",giftKind:null};
  render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()} />);
  await screen.findByText("小喵出门啦");
  expect(screen.getByRole("textbox",{name:"聊天消息"})).toHaveProperty("disabled",true);
  fireEvent.click(screen.getByRole("button",{name:"写信"}));
  await waitFor(()=>expect(navigation.openLetter).toHaveBeenCalledWith(snapshot("session-1").context,"3"));
  expect(api.submit).not.toHaveBeenCalled();
});

it("聊天详情使用会话侧栏，管理表单通过弹窗显示",async()=>{
  renderReact(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()}/>);
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"聊天消息"})).toHaveProperty("disabled",false));
  expect(screen.queryByRole("dialog",{name:"会话设置"})).toBeNull();
  expect(screen.queryByRole("button",{name:"保存名称"})).toBeNull();
  expect(screen.queryByRole("button",{name:"删除会话"})).toBeNull();
  expect(screen.queryByRole("combobox",{name:"当前会话"})).toBeNull();
  expect(screen.getByRole("navigation",{name:"会话列表"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"会话设置"}));
  expect(screen.queryByRole("button",{name:"保存名称"})).toBeNull();
  expect(screen.getByRole("button",{name:"删除会话"})).toBeTruthy();
});

it("侧栏可收起展开，底部入口打开原设置与记事本窗口",async()=>{
  render(<ChatPage snapshot={snapshot("session-1")} onRefresh={vi.fn()}/>);
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"聊天消息"})).toHaveProperty("disabled",false));
  fireEvent.click(screen.getByRole("button",{name:"收起会话侧栏"}));
  expect(screen.queryByRole("navigation",{name:"会话列表"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"展开会话侧栏"}));
  expect(screen.getByRole("navigation",{name:"会话列表"})).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"设置"}));
  await waitFor(()=>expect(navigation.open).toHaveBeenCalledWith("settings"));
  await waitFor(()=>expect(screen.getByRole("button",{name:"查看记事本"})).toHaveProperty("disabled",false));
  fireEvent.click(screen.getByRole("button",{name:"查看记事本"}));
  await waitFor(()=>expect(navigation.open).toHaveBeenCalledWith("memory"));
});

it("右键重命名和置顶使用点中的非当前会话，不切换或中断回复",async()=>{
  const current=snapshot("session-1"), refresh=vi.fn();
  api.rename.mockRejectedValueOnce(new Error("名称保存失败")).mockResolvedValueOnce({...secondSession,title:"新的名称"});
  api.setPinned.mockResolvedValue({...secondSession,isPinned:true});
  const page=render(<ChatPage snapshot={current} onRefresh={refresh}/>);
  const other=screen.getByRole("button",{name:"第二段对话 · 用户设定：未绑定"});
  fireEvent.contextMenu(other);
  fireEvent.click(screen.getByRole("menuitem",{name:"重命名"}));
  fireEvent.change(screen.getByLabelText("新会话名称"),{target:{value:"新的名称"}});
  fireEvent.click(screen.getByRole("button",{name:"保存名称"}));
  await screen.findByText("名称保存失败");
  expect(screen.getByLabelText("新会话名称")).toHaveProperty("value","新的名称");
  fireEvent.click(screen.getByRole("button",{name:"保存名称"}));
  await waitFor(()=>expect(screen.queryByRole("dialog",{name:"重命名会话"})).toBeNull());
  expect(api.rename).toHaveBeenLastCalledWith(current.context,"session-2","新的名称");
  fireEvent.contextMenu(other);
  fireEvent.click(screen.getByRole("menuitem",{name:"置顶"}));
  await waitFor(()=>expect(api.setPinned).toHaveBeenCalledWith(current.context,"session-2",true));
  await waitFor(()=>expect(refresh).toHaveBeenCalledTimes(2));
  page.rerender(<ChatPage snapshot={{...current,sessions:[{...secondSession,isPinned:true},firstSession]}} onRefresh={refresh}/>);
  expect(within(screen.getByRole("region",{name:"已置顶"})).getByRole("button",{name:"第二段对话 · 用户设定：未绑定"})).toBeTruthy();
  fireEvent.contextMenu(screen.getByRole("button",{name:"第二段对话 · 用户设定：未绑定"}));
  fireEvent.click(screen.getByRole("menuitem",{name:"取消置顶"}));
  await waitFor(()=>expect(api.setPinned).toHaveBeenLastCalledWith(current.context,"session-2",false));
  expect(api.select).not.toHaveBeenCalled(); expect(api.cancel).not.toHaveBeenCalled();
});

it("右键删除其他会话不取消当前正在生成的回复",async()=>{
  const current=snapshot("session-1"), refresh=vi.fn();
  const round:RoundSnapshot={round:{id:"10",sessionId:"session-1",status:"pending",stopReason:null,createdAt:"2026-10-15",completedAt:null},userMessage:{...message("20","session-1","问题"),roundId:"10"},assistantMessage:null};
  api.current.mockResolvedValue({...current.context,sessionId:"session-1",requestId:"request-1",roundId:"10",seq:0,partialContent:"正在回答",expression:null,round} satisfies ChatRequestSnapshot);
  api.delete.mockResolvedValue({currentSessionId:"session-1",contextRevision:1});
  render(<ChatPage snapshot={current} onRefresh={refresh}/>);
  await screen.findByRole("button",{name:"暂停回复"});
  fireEvent.contextMenu(screen.getByRole("button",{name:"第二段对话 · 用户设定：未绑定"}));
  fireEvent.click(screen.getByRole("menuitem",{name:"删除会话"}));
  expect(screen.getByText(/当前聊天和记忆任务保持不变/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"确认删除"}));
  await waitFor(()=>expect(api.delete).toHaveBeenCalledWith(current.context,"session-2"));
  await waitFor(()=>expect(refresh).toHaveBeenCalledOnce());
  expect(api.cancel).not.toHaveBeenCalled(); expect(api.select).not.toHaveBeenCalled();
});
