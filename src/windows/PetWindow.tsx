import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState, type PointerEvent } from "react";
import { configurationApi, errorMessage, petApi, travelApi } from "../api";
import { usePetWindow } from "./usePetWindow";
import { usePetAppearance } from "./usePetAppearance";
import { usePetActivity } from "./usePetActivity";
import { usePetChat } from "./usePetChat";
import { PetChatBubble, PetChatInput } from "../components/PetChat";
import { usePetMemoryErrors } from "./usePetMemoryErrors";
import { usePetWander } from "./usePetWander";
import { navigation, type ApplicationPage } from "../platform/navigation";
import PetArtworkDisplay from "../components/PetArtworkDisplay";
import AccessoryLayer from "../components/AccessoryLayer";
import { AccessoryCraftPresentation } from "../components/AccessoryCraftCard";
import { useAccessoryCraft } from "../hooks/useAccessoryCraft";
import PetReminderNotifications from "../components/PetReminderNotifications";
import PetTravelBubble from "../components/PetTravelBubble";
import PetConversationBubbles from "../components/PetConversationBubbles";
import "./PetWindow.css";

const DRAG_THRESHOLD_PX = 5;
type PointerStart = { pointerId: number; x: number; y: number };

export default function PetWindow() {
  const root = useRef<HTMLDivElement>(null);
  const face = useRef<HTMLDivElement>(null);
  const seenTravelIds = useRef(new Set<string>());
  const seenProactiveMessages = useRef(new Set<string>());
  const { state, error, pending, toggleHidden, setChatOpen, retry, fail } = usePetWindow(root);
  const pointerStart = useRef<PointerStart | null>(null);
  const draggingStarted = useRef(false);
  const [dragging, setDragging] = useState(false);
  const [pressed, setPressed] = useState(false);
  const [startupError, setStartupError] = useState("");
  const appearance = usePetAppearance();
  const activity = usePetActivity();
  const quickChat = usePetChat(activity.view);
  const [showChatLabel, setShowChatLabel] = useState(false);
  const [optionsOpen, setOptionsOpen] = useState(false);
  const options = useRef<HTMLDivElement>(null);
  const accessoryContext = { personaId: activity.view?.personaId ?? "", sessionId: activity.view?.sessionId ?? null, contextRevision: activity.view?.contextRevision ?? -1 };
  const craft = useAccessoryCraft(accessoryContext);
  const memoryErrors = usePetMemoryErrors(activity.view);
  const [menuOpen, setMenuOpen] = useState(false);
  const [showErrors, setShowErrors] = useState(false);
  const mainError = activity.view?.error;
  const reportedErrors = [mainError ? `连接错误：${mainError}` : "", ...memoryErrors].filter(Boolean);
  const visibleError = error || startupError || appearance.error || activity.error || (showErrors ? reportedErrors.join("\n") : "");
  const wanderStyle = usePetWander(root, face, activity.view, appearance.ready && !state.hidden && !pending
    && !dragging && !pressed && !menuOpen && !optionsOpen && !state.chatOpen && !quickChat.busy && !quickChat.reply && !visibleError && !craft.preview);
  const openPage = async (page: ApplicationPage) => {
    setOptionsOpen(false);
    try { await navigation.open(page); }
    catch (cause) { fail(errorMessage(cause)); }
  };

  const openLetter = async () => {
    const view = activity.view;
    if (!view?.sessionId) return;
    const context = { personaId:view.personaId,sessionId:view.sessionId,contextRevision:view.contextRevision };
    try {
      const trip = await travelApi.current(context);
      if (!trip) throw new Error("这次旅行已经结束，请刷新后再打开聊天。");
      await navigation.openLetter(context,trip.id);
    } catch (cause) { fail(errorMessage(cause)); }
  };

  useEffect(() => {
    let disposed = false;
    void configurationApi.status().catch((cause: unknown) => {
      if (!disposed) setStartupError(`配置存储不可用：${errorMessage(cause)}。处理后请重新启动应用。`);
    });
    return () => { disposed = true; };
  }, []);
  useEffect(() => {
    pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false);
    setMenuOpen(false);
  }, [state.hidden]);
  useEffect(() => { setMenuOpen(false); setOptionsOpen(false); }, [activity.view?.contextRevision, state.hidden]);
  useEffect(() => {
    const close = (event: globalThis.PointerEvent) => { if (options.current && !options.current.contains(event.target as Node)) setOptionsOpen(false); };
    document.addEventListener("pointerdown", close);
    return () => document.removeEventListener("pointerdown", close);
  }, []);
  useEffect(() => {
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") { setMenuOpen(false); setOptionsOpen(false); setShowErrors(false); if (state.chatOpen && !event.isComposing) void setChatOpen(false); } };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [state.chatOpen, setChatOpen]);
  useEffect(() => {
    const reset = () => { pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false); setOptionsOpen(false); setShowChatLabel(false); };
    window.addEventListener("blur", reset);
    return () => window.removeEventListener("blur", reset);
  }, []);

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || pointerStart.current) return;
    setMenuOpen(false); setOptionsOpen(false);
    setPressed(true);
    pointerStart.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY };
    draggingStarted.current = false;
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = pointerStart.current;
    if (!start || start.pointerId !== event.pointerId || draggingStarted.current) return;
    if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < DRAG_THRESHOLD_PX) return;
    draggingStarted.current = true; setDragging(true);
    // 原生拖动可能吞掉 pointerup；结束后清理旧指针并恢复呼吸。
    void petApi.dragging(true).catch(fail).then(() => getCurrentWindow().startDragging()).catch(fail).finally(() => {
      void petApi.dragging(false).catch(fail);
      if (pointerStart.current === start) {
        pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false);
      }
    });
  };
  const resetPointer = (event: PointerEvent<HTMLDivElement>) => {
    if (pointerStart.current?.pointerId === event.pointerId) {
      pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false);
    }
  };
  const releasePointer = (event: PointerEvent<HTMLDivElement>) => {
    const click = pointerStart.current?.pointerId === event.pointerId && !draggingStarted.current;
    resetPointer(event);
    if (click) void activity.interact("click");
  };

  return (
    <div ref={root} data-testid="pet-window" className="pet-window" data-hidden={state.hidden}
      data-transitioning={pending} data-chat-open={Boolean(state.chatOpen)}
      data-dragging={dragging} data-error={Boolean(visibleError)} data-ready={appearance.ready} style={{...appearance.style,...wanderStyle}}>
      <button className="ym-button pet-visibility" data-pet-interactive
        data-variant={error ? "danger" : undefined}
        disabled={pending || (state.revision < 0 && !error)}
        onClick={() => { void (state.revision < 0 ? retry() : toggleHidden()); }}
        title={error || (pending ? "正在更新宠窗" : state.hidden ? "显示宠窗" : "隐藏宠窗")}
        aria-description={error || undefined} aria-busy={pending}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          {state.hidden ? <><path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z"/><circle cx="12" cy="12" r="2.5"/></>
            : <><path d="M3 3l18 18M10.6 6.1A12.8 12.8 0 0 1 12 6c6.5 0 10 6 10 6a17 17 0 0 1-3.2 3.5M6.1 6.9A18 18 0 0 0 2 12s3.5 6 10 6c1.6 0 3.1-.4 4.4-1"/></>}
        </svg>
        <span>{state.revision < 0 ? (error ? "重试连接" : "正在连接…") : state.hidden ? "显示宠窗" : "隐藏宠窗"}</span>
      </button>
      {!!reportedErrors.length && !state.hidden && <button className="ym-button pet-health" data-pet-interactive
        data-variant="danger" aria-label="查看错误" aria-expanded={showErrors}
        onClick={() => { setShowErrors(value => !value); setMenuOpen(false); }}>!</button>}
      {!state.hidden && <>
        <div className="pet-options" ref={options}>
          <button className="ym-button pet-more" data-pet-interactive aria-label="更多操作" title="更多操作" aria-haspopup="menu" aria-expanded={optionsOpen}
            onClick={() => { setOptionsOpen(value => !value); setMenuOpen(false); setShowErrors(false); }}>...</button>
          {optionsOpen && <div className="pet-options-menu" data-pet-interactive role="menu" aria-label="宠窗更多操作">
            <button className="ym-button" role="menuitem" onClick={() => { void openPage("chat"); }}>聊天详情</button>
            <button className="ym-button" role="menuitem" onClick={() => { void openPage("settings"); }}>设置</button>
          </div>}
        </div>
        <div className="pet-chat-entry">
          <button className="ym-button pet-chat-toggle" data-pet-interactive aria-label="聊天" aria-expanded={Boolean(state.chatOpen)} aria-controls="pet-quick-chat"
            onPointerEnter={() => setShowChatLabel(true)} onPointerLeave={() => setShowChatLabel(false)} onFocus={() => setShowChatLabel(true)} onBlur={() => setShowChatLabel(false)}
            disabled={pending || state.revision < 0} onClick={() => { setOptionsOpen(false); setMenuOpen(false); void setChatOpen(!state.chatOpen); }}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="M20 11.5a8 8 0 0 1-8 8H5l-3 2v-10a9 9 0 1 1 18 0Z"/><path d="M7 10h8M7 14h5"/></svg>
          </button>
          {showChatLabel && <span className="pet-chat-tooltip" data-pet-interactive aria-hidden="true">聊天</span>}
        </div>
        {state.chatOpen && <div id="pet-quick-chat"><PetChatInput chat={quickChat} hasSession={Boolean(activity.view?.sessionId)} away={activity.view?.activity === "away"} pending={pending}
          onDetails={() => { void openPage("chat"); }} onLetter={() => { void openLetter(); }} /></div>}
      </>}
      {visibleError && !state.hidden && <div className="pet-error" data-pet-interactive role="alert" tabIndex={0}>
        <strong><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 6v7m0 3v2"/></svg>暂时遇到一点问题</strong>
        <p style={{ whiteSpace: "pre-line" }}>{visibleError}</p>
        {(mainError || memoryErrors.length > 0) && <div className="pet-error-actions">
          {mainError && <button className="ym-button" disabled={activity.pending} onClick={() => { void activity.refresh(); }}>{activity.pending ? "正在刷新…" : "刷新"}</button>}
          {!!memoryErrors.length && <button className="ym-button" onClick={() => { void openPage("memory"); }}>查看记忆</button>}
        </div>}
      </div>}
      {menuOpen && !state.hidden && <div className="pet-menu" data-pet-interactive aria-label="桌宠互动">
        <p>{activity.view?.intimacy == null ? "当前未选择会话" : `亲密度 ${activity.view.intimacy} / 999${activity.view.intimacy === 999 ? " · 已达上限" : ""}`}</p>
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void activity.interact("feed"); }}>喂食</button>
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void activity.interact("stroke"); }}>摸摸</button>
        <button className="ym-button" onClick={() => { setMenuOpen(false); void openPage("accessories"); }}>饰品</button>
      </div>}
      {!state.hidden && <div ref={face} data-pet-interactive aria-label="桌宠" role="img" className="pet-face"
        data-away={activity.view?.activity === "away"}
        onContextMenu={event => { event.preventDefault(); setMenuOpen(value => !value); setShowErrors(false); }}
        onLostPointerCapture={resetPointer} onPointerCancel={resetPointer}
        onPointerDown={handlePointerDown} onPointerMove={handlePointerMove} onPointerUp={releasePointer}>
        {activity.view?.sessionId && activity.view.activity !== "away" && <AccessoryLayer key={`${accessoryContext.personaId}:${accessoryContext.sessionId}:${accessoryContext.contextRevision}`} context={accessoryContext} face={face} canvas={root} />}
        <PetArtworkDisplay personaId={accessoryContext.personaId} expression={activity.view?.expression || "(・ω・)"} />
      </div>}
      <PetReminderNotifications hidden={state.hidden} ready={appearance.ready && state.revision >= 0 && !pending}
        context={activity.view ? { personaId: activity.view.personaId, contextRevision: activity.view.contextRevision } : undefined}
        unobscured={!visibleError && !menuOpen && !optionsOpen}
        stateBubble={!dragging && (craft.preview || craft.error ? <AccessoryCraftPresentation context={accessoryContext} craft={craft} pet /> : activity.view?.activity === "away" ? <PetTravelBubble view={activity.view} seenTravelIds={seenTravelIds.current} /> : quickChat.reply ? <PetChatBubble chat={quickChat} onDetails={() => { void openPage("chat"); }} /> : <PetConversationBubbles view={activity.view} seenMessageIds={seenProactiveMessages.current} />)} />
    </div>
  );
}
