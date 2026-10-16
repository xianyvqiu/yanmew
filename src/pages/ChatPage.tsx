import { useEffect, useRef, useState } from "react";
import { errorMessage, sessionApi } from "../api";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import { useTravel } from "../hooks/useTravel";
import { useChat } from "../hooks/useChat";
import type { Message,ChatImageInput,Session } from "../types";
import ChatImage from "../components/ChatImage";
import ChatComposer from "../components/ChatComposer";
import CopyMessageButton from "../components/CopyMessageButton";
import ChatBackgroundSettings from "../components/ChatBackgroundSettings";
import UserProfilePicker from "../components/UserProfilePicker";
import ChatSessionSidebar from "../components/ChatSessionSidebar";
import { navigation } from "../platform/navigation";
import "./workspace.css";

type SessionDeleteConfirmation = { context: WorkspaceContext["context"]; sessionId: string; title: string };
type MessageTarget = { context: WorkspaceContext["context"]; message: Message };
type MessageDeleteConfirmation = MessageTarget & { kind: "complete" | "unfinished" | "proactive" };

function sameContext(left: WorkspaceContext["context"], right: WorkspaceContext["context"]) {
  return left.personaId === right.personaId && left.sessionId === right.sessionId
    && left.contextRevision === right.contextRevision;
}

export default function ChatPage({ snapshot, onRefresh, sidebarCollapsed: controlledCollapsed, onSidebarCollapsed }: {
  snapshot: WorkspaceContext; onRefresh: () => void; sidebarCollapsed?: boolean; onSidebarCollapsed?: (value: boolean) => void;
}) {
  const { context, persona, sessions, userProfiles } = snapshot;
  const chat = useChat(context);
  const history = useRef<HTMLElement>(null);
  const followHistory = useRef(true);
  useEffect(() => { if (followHistory.current && history.current) history.current.scrollTop = history.current.scrollHeight; }, [chat.active?.partialContent, chat.messages.length]);
  const travel = useTravel(context);
  const away = Boolean(travel.current);
  const { messages, loading, afterId } = chat;
  const [draft, setDraft] = useState("");
  const [images,setImages]=useState<ChatImageInput[]>([]);
  useEffect(()=>setImages([]),[context.personaId,context.sessionId,context.contextRevision]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [newProfile, setNewProfile] = useState("");
  const [sessionSettingsOpen, setSessionSettingsOpen] = useState(false);
  const [localCollapsed, setLocalCollapsed] = useState(() => window.innerWidth < 680);
  const sidebarCollapsed = controlledCollapsed ?? localCollapsed;
  const collapseSidebar = (value: boolean) => { setLocalCollapsed(value); onSidebarCollapsed?.(value); };
  const settingsDialog = useRef<HTMLDialogElement>(null);
  const renameDialog = useRef<HTMLDialogElement>(null);
  const [renaming, setRenaming] = useState<SessionDeleteConfirmation | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [renameError, setRenameError] = useState("");
  const [profilePickerOpen, setProfilePickerOpen] = useState(false);
  const [profilePickerError, setProfilePickerError] = useState("");
  const currentSession = sessions.find(item => item.id === context.sessionId);
  const [binding, setBinding] = useState(currentSession?.userProfileId ?? "");
  const bindingOwner = useRef(`${context.personaId}:${context.sessionId}`);
  const bindingDirty = useRef(false);
  const [deleting, setDeleting] = useState<SessionDeleteConfirmation | null>(null);
  const [deleteError, setDeleteError] = useState("");
  const [editing, setEditing] = useState<MessageTarget | null>(null);
  const [editDraft, setEditDraft] = useState("");
  const [messageDeleting, setMessageDeleting] = useState<MessageDeleteConfirmation | null>(null);
  const [messageError, setMessageError] = useState("");
  const [title, setTitle] = useState(sessions.find(item => item.id === context.sessionId)?.title ?? "");
  const alive = useRef(true);
  const locked = useRef(false);
  const titleOwner = useRef(context.sessionId);
  const titleDirty = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const messageDialog = useRef<HTMLDialogElement>(null);
  const currentContext = useRef(context);
  const localMessageTarget = useRef(false);
  currentContext.current = context;
  localMessageTarget.current = Boolean(editing || messageDeleting);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);
  useEffect(() => {
    if (newProfile && !userProfiles.some(profile => profile.id === newProfile)) setNewProfile("");
  }, [userProfiles, newProfile]);
  useEffect(() => {
    const owner = `${context.personaId}:${context.sessionId}`;
    if (bindingOwner.current !== owner) { bindingOwner.current = owner; bindingDirty.current = false; }
    if (!bindingDirty.current || (binding && !userProfiles.some(profile => profile.id === binding))) {
      bindingDirty.current = false; setBinding(currentSession?.userProfileId ?? "");
    }
  }, [context.personaId, context.sessionId, currentSession?.userProfileId, userProfiles, binding]);
  useEffect(() => {
    if (titleOwner.current !== context.sessionId) { titleOwner.current = context.sessionId; titleDirty.current = false; }
    if (!titleDirty.current) setTitle(sessions.find(item => item.id === context.sessionId)?.title ?? "");
  }, [sessions, context.sessionId]);
  useEffect(() => {
    if (deleting && !sameContext(deleting.context, context)) { setDeleting(null); setDeleteError(""); }
    if (renaming && !sameContext(renaming.context, context)) { setRenaming(null); setRenameError(""); }
  }, [deleting, renaming, context.personaId, context.sessionId, context.contextRevision]);
  useEffect(() => {
    if (editing && !sameContext(editing.context, context)) { setEditing(null); setEditDraft(""); setMessageError(""); }
    if (messageDeleting && !sameContext(messageDeleting.context, context)) { setMessageDeleting(null); setMessageError(""); }
  }, [editing, messageDeleting, context.personaId, context.sessionId, context.contextRevision]);
  useEffect(() => {
    if (localMessageTarget.current) setMessageError(chat.error);
  }, [chat.error]);
  useEffect(() => {
    if (deleting && !dialog.current?.open) dialog.current?.showModal();
    else if (!deleting) dialog.current?.close();
  }, [deleting]);
  useEffect(() => {
    if (messageDeleting && !messageDialog.current?.open) messageDialog.current?.showModal();
    else if (!messageDeleting) messageDialog.current?.close();
  }, [messageDeleting]);
  useEffect(() => {
    if (sessionSettingsOpen && !settingsDialog.current?.open) settingsDialog.current?.showModal();
    else if (!sessionSettingsOpen) settingsDialog.current?.close();
  }, [sessionSettingsOpen]);
  useEffect(() => {
    if (renaming && !renameDialog.current?.open) renameDialog.current?.showModal();
    else if (!renaming) renameDialog.current?.close();
  }, [renaming]);
  const dialogOpen = Boolean(deleting || messageDeleting || profilePickerOpen || renaming);
  useEffect(() => { setProfilePickerOpen(false); setProfilePickerError(""); }, [context.personaId, context.sessionId, context.contextRevision]);
  const bindProfile = async (profileId: string | null) => {
    if (locked.current || !context.sessionId) return;
    locked.current = true; setBusy(true); setProfilePickerError("");
    const target = { ...context };
    try {
      const saved = await sessionApi.setUserProfile(target, target.sessionId!, profileId);
      if (alive.current && sameContext(target, currentContext.current)) {
        bindingDirty.current = false; setBinding(saved.userProfileId ?? ""); setProfilePickerOpen(false); onRefresh();
      }
    } catch (cause) { if (alive.current && sameContext(target, currentContext.current)) setProfilePickerError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const tail = !afterId && chat.ready && !loading ? messages[messages.length - 1] : undefined;
  const tailRound = tail?.roundId && tail.roundId === chat.lastRound?.round.id ? chat.lastRound : null;
  const deletableTail = tail && (tail.roundId === null || (tailRound && (tail.role === "assistant" || !tailRound.assistantMessage))) ? tail : undefined;
  const finalReply = tail?.role === "assistant" && tail.id === tailRound?.assistantMessage?.id ? tailRound : null;
  const openTailDelete = (message: Message) => {
    if (locked.current || dialogOpen || editing || message.id !== deletableTail?.id) return;
    setMessageError("");
    setMessageDeleting({ context: { ...context }, message: { ...message },
      kind: message.roundId === null ? "proactive" : tailRound?.round.status === "completed" ? "complete" : "unfinished" });
  };
  const act = async (action: () => Promise<unknown>, switching = false) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    try { if (switching && chat.busy) await chat.cancel(); await action(); if (alive.current) onRefresh(); }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const send = async () => {
    if (locked.current || editing || dialogOpen || away || travel.loading || travel.error) return;
    const input = draft;
    const target = { ...context };
    const sentImages=images;
    await messageAction(() => sentImages.length?chat.send(input,sentImages):chat.send(input), () => {
      if(sameContext(target,currentContext.current))setImages(current=>current===sentImages?[]:current);
      if (sameContext(target, currentContext.current)) setDraft(current => current === input ? "" : current);
    });
  };
  const messageAction = async (action: () => Promise<boolean>, onSuccess?: () => void, localError = false) => {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError("");
    if (localError) setMessageError("");
    const target = { ...context };
    try {
      const success = await action();
      if (success && alive.current && sameContext(target, currentContext.current)) onSuccess?.();
    } catch (cause) {
      if (alive.current && sameContext(target, currentContext.current)) {
        if (localError) setMessageError(errorMessage(cause));
        else setError(errorMessage(cause));
      }
    } finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const saveEdit = async () => {
    const target = editing;
    if (!target || chat.busy || !editDraft.trim() || !sameContext(target.context, currentContext.current)) return;
    await messageAction(() => chat.editMessage(target.message, editDraft), () => { setEditing(null); setEditDraft(""); setMessageError(""); }, true);
  };
  const deleteMessage = async () => {
    const target = messageDeleting;
    if (!target || !sameContext(target.context, currentContext.current)) return;
    await messageAction(() => chat.deleteTail(target.message), () => { setMessageDeleting(null); setMessageError(""); }, true);
  };
  const renameSession = async () => {
    const target = context;
    await sessionApi.rename(target, target.sessionId!, title);
    if (alive.current && sameContext(target, currentContext.current)) {
      titleDirty.current = false;
      setTitle(title.trim());
    }
  };
  const deleteSession = async () => {
    const target = deleting;
    if (!target || locked.current) return;
    if (!sameContext(target.context, currentContext.current)) { setDeleting(null); setDeleteError(""); return; }
    locked.current = true; setBusy(true); setDeleteError("");
    try {
      if (chat.busy && target.sessionId === target.context.sessionId) await chat.cancel();
      if (!alive.current || !sameContext(target.context, currentContext.current)) return;
      await sessionApi.delete(target.context, target.sessionId);
      if (alive.current) { setDeleting(null); onRefresh(); }
    } catch (cause) {
      if (alive.current && sameContext(target.context, currentContext.current)) setDeleteError(errorMessage(cause));
    } finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const openSessionDelete = (session: Session) => {
    setSessionSettingsOpen(false); setDeleteError(""); setDeleting({ context: { ...context }, sessionId: session.id, title: session.title });
  };
  const renameSelected = async () => {
    const target = renaming;
    if (!target || locked.current || !renameDraft.trim() || !sameContext(target.context, currentContext.current)) return;
    locked.current = true; setBusy(true); setRenameError("");
    try {
      await sessionApi.rename(target.context, target.sessionId, renameDraft);
      if (alive.current && sameContext(target.context, currentContext.current)) { setRenaming(null); onRefresh(); }
    } catch (cause) { if (alive.current && sameContext(target.context, currentContext.current)) setRenameError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  return <div className="ym-chat-layout" data-sidebar-collapsed={sidebarCollapsed}>
    <ChatSessionSidebar sessions={sessions} profiles={userProfiles} currentId={context.sessionId} contextKey={`${context.personaId}:${context.sessionId}:${context.contextRevision}`}
      collapsed={sidebarCollapsed} onCollapse={collapseSidebar} disabled={busy || dialogOpen || Boolean(editing)}
      bindingDisabled={!context.sessionId || !chat.ready || busy || dialogOpen || Boolean(editing)} memoryDisabled={!context.sessionId || busy || dialogOpen}
      onCreate={() => { void act(() => sessionApi.create(context, newProfile || null), true); }}
      onSelect={id => { if (window.innerWidth < 680) collapseSidebar(true); void act(() => sessionApi.select(context, id), true); }}
      onRename={session => { setRenameError(""); setRenameDraft(session.title); setRenaming({ context: { ...context }, sessionId: session.id, title: session.title }); }}
      onDelete={openSessionDelete} onPin={session => { void act(() => sessionApi.setPinned(context, session.id, !session.isPinned)); }}
      onBind={() => { setProfilePickerError(""); setProfilePickerOpen(true); }} onSessionSettings={() => setSessionSettingsOpen(true)}
      onMemory={() => { void act(() => navigation.open("memory")); }} onSettings={() => { void act(() => navigation.open("settings")); }} />
    <div className="ym-workspace-page ym-chat-page">
    <dialog ref={settingsDialog} id="chat-session-settings" className="ym-session-dialog ym-session-settings-dialog" aria-labelledby="chat-session-settings-title"
      onCancel={event => { event.preventDefault(); if (!locked.current) setSessionSettingsOpen(false); }}>
      <div className="ym-page-heading"><h2 id="chat-session-settings-title">会话设置</h2><button type="button" className="ym-button" disabled={busy} onClick={() => setSessionSettingsOpen(false)}>关闭会话设置</button></div>
      <div className="ym-session-dialog-body ym-chat-session-settings">
      <ChatBackgroundSettings persona={persona} contextRevision={context.contextRevision} label="聊天外观"
        disabled={busy || dialogOpen || editing !== null} onSaved={() => { /* 配置通知会原位刷新，不重建正在回复的聊天页。 */ }} />
      <label className="ym-field">新会话的用户设定<select aria-label="新会话的用户设定" value={newProfile} disabled={busy || dialogOpen} onChange={event => setNewProfile(event.target.value)}>
        <option value="">未绑定</option>{userProfiles.map(profile => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
      </select></label>

      {context.sessionId && <form className="ym-inline-form" onSubmit={event => { event.preventDefault(); void act(renameSession); }}>
        <label className="ym-field">会话名称<input value={title} maxLength={100} disabled={busy || dialogOpen} onChange={event => { titleDirty.current = true; setTitle(event.target.value); }} /></label>
        <button className="ym-button" disabled={busy || dialogOpen || !title.trim()} type="submit">保存名称</button>
      </form>}
      {context.sessionId && <form className="ym-inline-form" onSubmit={event => {
        event.preventDefault();
        const target = { ...context };
        void act(async () => {
          if (!sameContext(target, currentContext.current)) return;
          const saved = await sessionApi.setUserProfile(target, target.sessionId!, binding || null);
          if (alive.current && sameContext(target, currentContext.current)) { bindingDirty.current = false; setBinding(saved.userProfileId ?? ""); }
        });
      }}>
        <label className="ym-field">当前会话的用户设定<select aria-label="当前会话的用户设定" value={binding} disabled={busy || dialogOpen || Boolean(editing)} onChange={event => { bindingDirty.current = true; setBinding(event.target.value); }}>
          <option value="">未绑定</option>{userProfiles.map(profile => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
        </select></label>
        <button className="ym-button" type="submit" disabled={busy || dialogOpen || Boolean(editing) || !chat.ready || binding === (currentSession?.userProfileId ?? "")}>修改绑定</button>
      </form>}
      <p className="ym-page-note">用户设定在设置窗管理。修改绑定，或编辑、删除正在使用的设定，会先中断当前回复；已有消息和记忆保留，新设定用于下一次回复。</p>
      <p className="ym-page-note">切换会话会取消已经开始的旅行与待回信。</p>
      {context.sessionId && <button className="ym-button ym-session-delete-entry" data-variant="danger" disabled={busy || dialogOpen || Boolean(editing)}
        onClick={() => { if (currentSession) openSessionDelete(currentSession); }}>删除会话</button>}
      {error && <p role="alert" className="ym-page-error">{error}</p>}
      </div>
    </dialog>
    {!sessionSettingsOpen && !editing && !messageDeleting && (error || chat.error) && <div role="alert" className="ym-page-error">{error || chat.error}<button className="ym-button" disabled={busy || dialogOpen} onClick={() => { setError(""); void chat.reload(); }}>重新加载</button></div>}
    <section ref={history} className="ym-page-card ym-chat-history" aria-label="聊天记录" onScroll={() => { const element = history.current; if (element) followHistory.current = element.scrollHeight - element.scrollTop - element.clientHeight < 40; }}>
      <h2 className="ym-chat-history-title">聊天记录</h2>
      {!context.sessionId ? <p className="ym-page-empty">为 {persona.name} 创建一个会话，开始记录彼此的日常。</p>
        : !messages.length && !loading ? <p className="ym-page-empty">这个会话还没有消息。</p>
        : <ol className="ym-message-list">{messages.filter(message => chat.active?.regenerating || message.id !== chat.active?.round.assistantMessage?.id).map(message => <li key={message.id} data-role={message.role}>
          <div className="ym-message-meta"><strong>{message.role === "user" ? "你" : persona.name}</strong><time>{new Date(message.createdAt).toLocaleString()}</time></div>
          <ChatMessageText content={message.content} />
          {message.images?.map(image=><ChatImage key={image.id} image={image} context={context}/>)}
          {editing?.message.id === message.id ? <form className="ym-message-editor" onSubmit={event => { event.preventDefault(); void saveEdit(); }}>
            <label className="ym-field">编辑消息<textarea aria-label="编辑消息正文" value={editDraft} rows={4} disabled={busy || chat.busy}
              onChange={event => setEditDraft(event.target.value)} /></label>
            <p className="ym-page-note">仅修改消息原文，短期记忆、长期记忆和旅行游记不会同步修改；需要同步时请进入记忆小窗操作。</p>
            {messageError && <p role="alert" className="ym-page-error">{messageError}</p>}
            <div className="ym-message-actions">
              <CopyMessageButton content={message.content} />
              <button type="button" className="ym-button" disabled={busy} onClick={() => { setEditing(null); setEditDraft(""); setMessageError(""); }}>取消编辑</button>
              <button type="submit" className="ym-button" disabled={busy || chat.busy || !chat.ready || !editDraft.trim()}>{busy ? "保存中…" : "保存消息"}</button>
            </div>
          </form> : <div className="ym-message-actions">
            <CopyMessageButton content={message.content} />
            <button className="ym-button" disabled={busy || chat.busy || loading || !chat.ready || dialogOpen || Boolean(editing)}
              onClick={() => { if (!locked.current && !chat.busy) { setMessageError(""); setEditing({ context: { ...context }, message: { ...message } }); setEditDraft(message.content); } }}>编辑</button>
            {message.id === deletableTail?.id && <button className="ym-button" data-variant="danger" disabled={busy || dialogOpen || Boolean(editing)}
              onClick={() => openTailDelete(message)}>{message.roundId !== null && message.role === "assistant" ? "删除本轮" : "删除"}</button>}
            {message.id === tail?.id && finalReply?.round.status === "completed" && !chat.busy && <button className="ym-button"
              disabled={busy || !chat.ready || dialogOpen || Boolean(editing) || away || travel.loading || Boolean(travel.error)} onClick={() => { void messageAction(() => chat.regenerateReply()); }}>重说</button>}
          </div>}
          {message.id === tail?.id && finalReply && message.role === "assistant" && message.roundId === chat.lastRound?.round.id
            && chat.lastRound.round.status === "interrupted" && chat.lastRound.round.stopReason === "length_limit" && !chat.active && <div className="ym-inline-form">
              <span className="ym-page-note">已达到本次输出上限</span>
              <button className="ym-button" disabled={busy || chat.busy || !chat.ready || loading || dialogOpen || Boolean(editing) || away || travel.loading || Boolean(travel.error)} onClick={() => { void messageAction(() => chat.continueReply()); }}>继续生成</button>
            </div>}
        </li>)}</ol>}
      {chat.active && <div className="ym-stream-message" aria-label="正在生成的回复">
        <strong>{persona.name} {chat.active.expression}</strong>
        <ChatMessageText content={chat.active.partialContent || "正在想怎么回答你…"} />
        {chat.active.webActivity && <p role="status" className="ym-page-note">{chat.active.webActivity === "searching" ? "正在搜索网络资料…" : "正在读取网页…"}</p>}
        <span className="ym-page-note">{chat.active.regenerating ? "正在准备回复和记忆，完成后一起替换；中断保留旧结果。" : "正在回复，尚未保存"}</span>
        <div className="ym-message-actions">
          <CopyMessageButton content={chat.active.partialContent} />
          {!chat.active.regenerating && deletableTail && deletableTail.id === chat.active.round.assistantMessage?.id
            && <button className="ym-button" data-variant="danger" disabled={busy || dialogOpen || Boolean(editing)} onClick={() => openTailDelete(deletableTail)}>删除本轮</button>}
        </div>
      </div>}
      {chat.notice && <p role="status" className="ym-page-note">{chat.notice}</p>}
      {chat.budget && <p className="ym-page-note">本次最大输出：配置 {chat.budget.configuredMaxOutputTokens} tokens，实际 {chat.budget.effectiveMaxOutputTokens} tokens；
        上下文容量：{chat.budget.contextWindow == null ? "未知" : `${chat.budget.contextWindow} tokens`}；
        输入{chat.budget.estimated ? "估算" : "计数"}：{chat.budget.estimatedInputTokens} tokens；
        能力来源：{chat.budget.capacitySource === "manual" ? "手动设置" : chat.budget.capacitySource === "provider" ? "提供商" : chat.budget.capacitySource === "builtin" ? "已核对模型资料" : "未知"}。
        {chat.budget.effectiveMaxOutputTokens < chat.budget.configuredMaxOutputTokens && "本次输出因模型能力或剩余上下文容量而降低，已保存设置不变。"}</p>}
      {loading && <p role="status">正在读取聊天记录…</p>}
      {afterId && <button className="ym-button" disabled={loading || busy || dialogOpen} onClick={() => { void chat.loadMore(); }}>加载后续消息</button>}
    </section>
    {away && <section className="ym-page-card" aria-label="旅行中的会话">
      <h2>{persona.name}出门啦</h2><p>本次旅行期间只能写信。切换人设或会话会取消旅行和待回信，不生成返回奖励。</p>
      <div className="ym-inline-form"><button className="ym-button" disabled={busy || travel.loading || Boolean(travel.error)} onClick={() => { void act(() => navigation.openLetter(context, travel.current!.id)); }}>写信</button>
        <button className="ym-button" onClick={() => { void act(() => navigation.open("mailbox")); }}>查看邮箱</button></div>
      <p className="ym-page-note">需要继续聊天时，可在上方“当前会话”中切换其他会话。</p>
    </section>}
    {travel.error && <p role="alert" className="ym-page-error">出行状态读取失败：{travel.error}<button className="ym-button" onClick={() => { void travel.refresh(); }}>重试出行状态</button></p>}
    <ChatComposer images={images} onImagesChange={setImages} attachmentOwnerKey={`${context.personaId}:${context.sessionId}:${context.contextRevision}`} value={draft} onChange={setDraft} onSend={() => { void send(); }}
      onCancel={() => { if (!locked.current && !dialogOpen) void chat.cancel(); }} disabled={!context.sessionId || !chat.ready || busy || dialogOpen || Boolean(editing) || away || travel.loading || Boolean(travel.error)}
      busy={chat.busy} cancelling={chat.cancelling || busy || dialogOpen} />
    {profilePickerOpen && <UserProfilePicker profiles={userProfiles} selectedId={currentSession?.userProfileId ?? null} busy={busy} error={profilePickerError}
      onSelect={id => { void bindProfile(id); }} onClose={() => setProfilePickerOpen(false)} onDetails={id => {
        setProfilePickerError(""); void navigation.openSettingsDetail("userProfile", id).catch(cause => { if (alive.current) setProfilePickerError(errorMessage(cause)); });
      }} />}
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="session-delete-title" aria-describedby="session-delete-details"
      onCancel={event => { if (locked.current) event.preventDefault(); else { setDeleting(null); setDeleteError(""); } }}>
      <h2 id="session-delete-title">删除会话？</h2>
      <div className="ym-session-dialog-body" id="session-delete-details">
        <p>将删除“{deleting?.title}”，此操作不可恢复。</p>
        <p>该会话的聊天回合与消息、短期和长期记忆及来源、记事本、出行与旅行游记、饰品、信件和伴手礼将一并清理。全局自定义日期会保留。</p>
        <p>全局喵币钱包、账单、提醒，以及小游戏插件、授权、设置、存档和游戏局仍会保留。</p>
        <p>{deleting?.sessionId === context.sessionId ? "删除后切换到同一人设下最近更新的剩余会话；没有剩余会话时进入无会话状态，不会自动创建新会话。" : "删除的不是当前会话，当前聊天和记忆任务保持不变。"}</p>
        {deleteError && <p role="alert" className="ym-page-error">{deleteError}</p>}
      </div>
      <div className="ym-session-dialog-actions">
        <button className="ym-button" disabled={busy} onClick={() => { setDeleting(null); setDeleteError(""); }}>取消</button>
        <button className="ym-button" data-variant="danger" disabled={busy || !deleting} onClick={() => { void deleteSession(); }}>{busy ? "删除中…" : "确认删除"}</button>
      </div>
    </dialog>
    <dialog ref={messageDialog} className="ym-session-dialog" aria-labelledby="message-delete-title" aria-describedby="message-delete-details"
      onCancel={event => { if (locked.current) event.preventDefault(); else { setMessageDeleting(null); setMessageError(""); } }}>
      <h2 id="message-delete-title">{messageDeleting?.message.roundId !== null ? "删除本轮？" : "删除消息？"}</h2>
      <div className="ym-session-dialog-body" id="message-delete-details">
        <p>此操作不可恢复。</p>
        {messageDeleting?.kind === "complete" ? <p>将删除本轮用户消息、AI 回复、对应短期记忆，并按触发关系回退长期记忆。</p>
          : messageDeleting?.kind === "proactive" ? <p>只删除底部这一条消息，不删除前面的回合，也不修改已有记忆。</p>
          : <p>将删除本轮用户消息、已有的部分 AI 回复及未完整回合记录。中断回合不生成记忆，此操作不修改已有记忆。</p>}
        {chat.busy && <p>删除前会中断正在进行的回复；迟到的回复不会重新写入。</p>}
        {messageDeleting && messageError && <p role="alert" className="ym-page-error">{messageError}</p>}
      </div>
      <div className="ym-session-dialog-actions">
        <button className="ym-button" disabled={busy} onClick={() => { setMessageDeleting(null); setMessageError(""); }}>取消</button>
        <button className="ym-button" data-variant="danger" disabled={busy || !messageDeleting} onClick={() => { void deleteMessage(); }}>{busy ? "删除中…" : "确认删除"}</button>
      </div>
    </dialog>
    <dialog ref={renameDialog} className="ym-session-dialog" aria-labelledby="session-rename-title" onCancel={event => { event.preventDefault(); if (!locked.current) setRenaming(null); }}>
      <h2 id="session-rename-title">重命名会话</h2>
      <form className="ym-session-dialog-form" onSubmit={event => { event.preventDefault(); void renameSelected(); }}>
        <div className="ym-session-dialog-body">
        <label className="ym-field">新会话名称<input value={renameDraft} maxLength={100} disabled={busy} onChange={event => setRenameDraft(event.target.value)} /></label>
        {renameError && <p role="alert" className="ym-page-error">{renameError}</p>}
        </div>
        <div className="ym-session-dialog-actions"><button type="button" className="ym-button" disabled={busy} onClick={() => setRenaming(null)}>取消</button><button type="submit" className="ym-button" disabled={busy || !renameDraft.trim()}>保存名称</button></div>
      </form>
    </dialog>
    </div>
  </div>;
}
import ChatMessageText from "../components/ChatMessageText";
