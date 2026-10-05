import { useEffect, useRef, useState } from "react";
import { errorMessage, sessionApi } from "../api";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import { useChat } from "../hooks/useChat";
import ChatComposer from "../components/ChatComposer";
import { navigation } from "../platform/navigation";
import "./workspace.css";

export default function ChatPage({ snapshot, onRefresh }: { snapshot: WorkspaceContext; onRefresh: () => void }) {
  const { context, persona, sessions } = snapshot;
  const chat = useChat(context);
  const { messages, loading, afterId } = chat;
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [title, setTitle] = useState(sessions.find(item => item.id === context.sessionId)?.title ?? "");
  const alive = useRef(true);
  const locked = useRef(false);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);
  useEffect(() => { setTitle(sessions.find(item => item.id === context.sessionId)?.title ?? ""); }, [sessions, context.sessionId]);
  const act = async (action: () => Promise<unknown>, switching = false) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try { if (switching && chat.busy) await chat.cancel(); await action(); if (alive.current) onRefresh(); }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const send = async () => {
    const input = draft;
    if (await chat.send(input)) {
      if (alive.current) setDraft(current => current === input ? "" : current);
    }
  };
  return <div className="ym-workspace-page ym-chat-page">
    <section className="ym-page-card" aria-label="会话管理">
      <div className="ym-page-heading"><h2>我们的对话</h2><button className="ym-button" disabled={busy} onClick={() => { void act(() => sessionApi.create(context), true); }}>新建会话</button></div>
      <label className="ym-field">当前会话<select value={context.sessionId ?? ""} disabled={busy || !sessions.length}
        onChange={event => { const id = event.target.value; if (id) void act(() => sessionApi.select(context, id), true); }}>
        <option value="" disabled>尚未创建会话</option>
        {sessions.map(session => <option key={session.id} value={session.id}>{session.title}</option>)}
      </select></label>
      {context.sessionId && <form className="ym-inline-form" onSubmit={event => { event.preventDefault(); void act(() => sessionApi.rename(context, context.sessionId!, title)); }}>
        <label className="ym-field">会话名称<input value={title} maxLength={100} disabled={busy} onChange={event => setTitle(event.target.value)} /></label>
        <button className="ym-button" disabled={busy || !title.trim()} type="submit">保存名称</button>
      </form>}
      <p className="ym-page-note">切换会话会取消已经开始的旅行与待回信。</p>
      <button className="ym-button" disabled={!context.sessionId || busy} onClick={() => { void act(() => navigation.open("memory")); }}>查看记事本</button>
    </section>
    {(error || chat.error) && <div role="alert" className="ym-page-error">{error || chat.error}<button className="ym-button" onClick={() => { setError(""); void chat.reload(); }}>重新加载</button></div>}
    <section className="ym-page-card ym-chat-history" aria-label="聊天记录">
      <h2>聊天记录</h2>
      {!context.sessionId ? <p className="ym-page-empty">为 {persona.name} 创建一个会话，开始记录彼此的日常。</p>
        : !messages.length && !loading ? <p className="ym-page-empty">这个会话还没有消息。</p>
        : <ol className="ym-message-list">{messages.filter(message => message.id !== chat.active?.round.assistantMessage?.id).map(message => <li key={message.id} data-role={message.role}>
          <div className="ym-message-meta"><strong>{message.role === "user" ? "你" : persona.name}</strong><time>{new Date(message.createdAt).toLocaleString()}</time></div>
          <p>{message.content}</p>
          {message.role === "assistant" && message.roundId === chat.lastRound?.round.id
            && chat.lastRound.round.status === "interrupted" && chat.lastRound.round.stopReason === "length_limit" && !chat.active && <div className="ym-inline-form">
              <span className="ym-page-note">已达到本次输出上限</span>
              <button className="ym-button" disabled={busy || chat.busy || !chat.ready || loading} onClick={() => { void chat.continueReply(); }}>继续生成</button>
            </div>}
        </li>)}</ol>}
      {chat.active && <div className="ym-stream-message" aria-label="正在生成的回复">
        <strong>{persona.name} {chat.active.expression}</strong>
        <p>{chat.active.partialContent || "正在想怎么回答你…"}</p>
        <span className="ym-page-note">正在回复，尚未保存</span>
      </div>}
      {chat.notice && <p role="status" className="ym-page-note">{chat.notice}</p>}
      {chat.budget && <p className="ym-page-note">本次最大输出：配置 {chat.budget.configuredMaxOutputTokens} tokens，实际 {chat.budget.effectiveMaxOutputTokens} tokens；
        上下文容量：{chat.budget.contextWindow == null ? "未知" : `${chat.budget.contextWindow} tokens`}；
        输入{chat.budget.estimated ? "估算" : "计数"}：{chat.budget.estimatedInputTokens} tokens；
        能力来源：{chat.budget.capacitySource === "manual" ? "手动设置" : chat.budget.capacitySource === "provider" ? "提供商" : chat.budget.capacitySource === "builtin" ? "已核对模型资料" : "未知"}。
        {chat.budget.effectiveMaxOutputTokens < chat.budget.configuredMaxOutputTokens && "本次输出因模型能力或剩余上下文容量而降低，已保存设置不变。"}</p>}
      {loading && <p role="status">正在读取聊天记录…</p>}
      {afterId && <button className="ym-button" disabled={loading} onClick={() => { void chat.loadMore(); }}>加载后续消息</button>}
    </section>
    <ChatComposer value={draft} onChange={setDraft} onSend={() => { void send(); }}
      onCancel={() => { void chat.cancel(); }} disabled={!context.sessionId || !chat.ready || busy}
      busy={chat.busy} cancelling={chat.cancelling} />
  </div>;
}
