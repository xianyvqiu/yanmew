import { useEffect, useRef, useState } from "react";
import { errorMessage, sessionApi } from "../api";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import type { Message } from "../types";
import "./workspace.css";

export default function ChatPage({ snapshot, onRefresh }: { snapshot: WorkspaceContext; onRefresh: () => void }) {
  const { context, persona, sessions } = snapshot;
  const [messages, setMessages] = useState<Message[]>([]);
  const [afterId, setAfterId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [title, setTitle] = useState(sessions.find(item => item.id === context.sessionId)?.title ?? "");
  const alive = useRef(true);
  const locked = useRef(false);
  const messageLock = useRef(false);
  useEffect(() => {
    let disposed = false; alive.current = true;
    if (context.sessionId) {
      setLoading(true);
      void sessionApi.messages(context).then(page => {
        if (!disposed) { setMessages(page.items); setAfterId(page.nextAfterId); }
      }).catch(cause => { if (!disposed) setError(errorMessage(cause)); })
        .finally(() => { if (!disposed) setLoading(false); });
    }
    return () => { disposed = true; alive.current = false; };
  }, [context]);
  const act = async (action: () => Promise<unknown>) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try { await action(); if (alive.current) onRefresh(); }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const loadMore = async () => {
    if (!afterId || messageLock.current) return;
    messageLock.current = true; setLoading(true); setError("");
    try {
      const page = await sessionApi.messages(context, afterId);
      if (alive.current) { setMessages(previous => [...previous, ...page.items.filter(item => !previous.some(old => old.id === item.id))]); setAfterId(page.nextAfterId); }
    } catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { messageLock.current = false; if (alive.current) setLoading(false); }
  };
  return <div className="ym-workspace-page">
    <section className="ym-page-card" aria-label="会话管理">
      <div className="ym-page-heading"><h2>我们的对话</h2><button className="ym-button" disabled={busy} onClick={() => { void act(() => sessionApi.create(context)); }}>新建会话</button></div>
      <label className="ym-field">当前会话<select value={context.sessionId ?? ""} disabled={busy || !sessions.length}
        onChange={event => { const id = event.target.value; if (id) void act(() => sessionApi.select(context, id)); }}>
        <option value="" disabled>尚未创建会话</option>
        {sessions.map(session => <option key={session.id} value={session.id}>{session.title}</option>)}
      </select></label>
      {context.sessionId && <form className="ym-inline-form" onSubmit={event => { event.preventDefault(); void act(() => sessionApi.rename(context, context.sessionId!, title)); }}>
        <label className="ym-field">会话名称<input value={title} maxLength={100} disabled={busy} onChange={event => setTitle(event.target.value)} /></label>
        <button className="ym-button" disabled={busy || !title.trim()} type="submit">保存名称</button>
      </form>}
      <p className="ym-page-note">切换会话会取消已经开始的旅行与待回信。</p>
    </section>
    {error && <div role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={onRefresh}>重新加载</button></div>}
    <section className="ym-page-card" aria-label="聊天记录">
      <h2>聊天记录</h2>
      {!context.sessionId ? <p className="ym-page-empty">为 {persona.name} 创建一个会话，开始记录彼此的日常。</p>
        : !messages.length && !loading ? <p className="ym-page-empty">这个会话还没有消息。</p>
        : <ol className="ym-message-list">{messages.map(message => <li key={message.id} data-role={message.role}>
          <div className="ym-message-meta"><strong>{message.role === "user" ? "你" : persona.name}</strong><time>{new Date(message.createdAt).toLocaleString()}</time></div>
          <p>{message.content}</p>
        </li>)}</ol>}
      {loading && <p role="status">正在读取聊天记录…</p>}
      {afterId && <button className="ym-button" disabled={loading} onClick={() => { void loadMore(); }}>加载后续消息</button>}
    </section>
    <p className="ym-page-note">当前可管理会话和查看已保存的聊天记录。发送消息功能尚未开放。</p>
  </div>;
}
