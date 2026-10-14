import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState, type PointerEvent } from "react";
import { configurationApi, errorMessage, petApi, travelApi } from "../api";
import { usePetWindow } from "./usePetWindow";
import { usePetAppearance } from "./usePetAppearance";
import { usePetActivity } from "./usePetActivity";
import { usePetMemoryErrors } from "./usePetMemoryErrors";
import { usePetWander } from "./usePetWander";
import { navigation, type ApplicationPage } from "../platform/navigation";
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
  const { state, error, pending, toggleHidden, retry, fail } = usePetWindow(root);
  const pointerStart = useRef<PointerStart | null>(null);
  const draggingStarted = useRef(false);
  const [dragging, setDragging] = useState(false);
  const [pressed, setPressed] = useState(false);
  const [startupError, setStartupError] = useState("");
  const appearance = usePetAppearance();
  const activity = usePetActivity();
  const accessoryContext = { personaId: activity.view?.personaId ?? "", sessionId: activity.view?.sessionId ?? null, contextRevision: activity.view?.contextRevision ?? -1 };
  const craft = useAccessoryCraft(accessoryContext);
  const memoryErrors = usePetMemoryErrors(activity.view);
  const [menuOpen, setMenuOpen] = useState(false);
  const [showErrors, setShowErrors] = useState(false);
  const mainError = activity.view?.error;
  const reportedErrors = [mainError ? `连接错误：${mainError}` : "", ...memoryErrors].filter(Boolean);
  const visibleError = error || startupError || appearance.error || activity.error || (showErrors ? reportedErrors.join("\n") : "");
  const wanderStyle = usePetWander(root, face, activity.view, appearance.ready && !state.hidden && !pending
    && !dragging && !pressed && !menuOpen && !visibleError && !craft.preview);
  const openPage = async (page: ApplicationPage) => {
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
  useEffect(() => { setMenuOpen(false); }, [activity.view?.contextRevision]);
  useEffect(() => {
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") { setMenuOpen(false); setShowErrors(false); } };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, []);
  useEffect(() => {
    const reset = () => { pointerStart.current = null; draggingStarted.current = false; setDragging(false); setPressed(false); };
    window.addEventListener("blur", reset);
    return () => window.removeEventListener("blur", reset);
  }, []);

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || pointerStart.current) return;
    setMenuOpen(false);
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
      data-transitioning={pending}
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
        <button className="ym-button pet-open-chat" data-pet-interactive onClick={() => { void (activity.view?.activity === "away" ? openLetter() : openPage("chat")); }}>{activity.view?.activity === "away" ? "写信" : "聊天"}</button>
        <button className="ym-button pet-open-settings" data-pet-interactive onClick={() => { void openPage("settings"); }}>设置</button>
      </>}
      {visibleError && !state.hidden && <div className="pet-error" data-pet-interactive role="alert" tabIndex={0}>
        <strong><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 6v7m0 3v2"/></svg>暂时遇到一点问题</strong>
        <p style={{ whiteSpace: "pre-line" }}>{visibleError}</p>
        {mainError && <button className="ym-button" disabled={activity.pending} onClick={() => { void activity.refresh(); }}>{activity.pending ? "正在刷新…" : "刷新"}</button>}
        {!!memoryErrors.length && <button className="ym-button" onClick={() => { void openPage("memory"); }}>查看记忆</button>}
      </div>}
      {menuOpen && !state.hidden && <div className="pet-menu" data-pet-interactive aria-label="桌宠互动">
        <p>{activity.view?.intimacy == null ? "当前未选择会话" : `亲密度 ${activity.view.intimacy} / 999${activity.view.intimacy === 999 ? " · 已达上限" : ""}`}</p>
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void activity.interact("feed"); }}>喂食</button>
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void activity.interact("stroke"); }}>摸摸</button>
        <button className="ym-button" onClick={() => { setMenuOpen(false); void openPage("accessories"); }}>饰品</button>
      </div>}
      {!state.hidden && <div ref={face} data-pet-interactive aria-label="颜文字桌宠" role="img" className="pet-face"
        data-away={activity.view?.activity === "away"}
        onContextMenu={event => { event.preventDefault(); setMenuOpen(value => !value); setShowErrors(false); }}
        onLostPointerCapture={resetPointer} onPointerCancel={resetPointer}
        onPointerDown={handlePointerDown} onPointerMove={handlePointerMove} onPointerUp={releasePointer}>
        {activity.view?.sessionId && activity.view.activity !== "away" && <AccessoryLayer key={`${accessoryContext.personaId}:${accessoryContext.sessionId}:${accessoryContext.contextRevision}`} context={accessoryContext} face={face} canvas={root} />}
        <span className="pet-expression">{activity.view?.expression || "(・ω・)"}</span>
      </div>}
      <PetReminderNotifications hidden={state.hidden} ready={appearance.ready && state.revision >= 0 && !pending}
        context={activity.view ? { personaId: activity.view.personaId, contextRevision: activity.view.contextRevision } : undefined}
        unobscured={!visibleError && !menuOpen}
        stateBubble={!dragging && (craft.preview || craft.error ? <AccessoryCraftPresentation context={accessoryContext} craft={craft} pet /> : activity.view?.activity === "away" ? <PetTravelBubble view={activity.view} seenTravelIds={seenTravelIds.current} /> : <PetConversationBubbles view={activity.view} seenMessageIds={seenProactiveMessages.current} />)} />
    </div>
  );
}
