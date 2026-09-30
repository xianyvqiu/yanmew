import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState, type PointerEvent } from "react";
import { configurationApi, errorMessage, petApi } from "../api";
import { usePetWindow } from "./usePetWindow";
import { usePetAppearance } from "./usePetAppearance";
import { usePetActivity } from "./usePetActivity";
import { navigation, type ApplicationPage } from "../platform/navigation";
import PetReminderNotifications from "../components/PetReminderNotifications";
import "./PetWindow.css";

const DRAG_THRESHOLD_PX = 5;
type PointerStart = { pointerId: number; x: number; y: number };

export default function PetWindow() {
  const root = useRef<HTMLDivElement>(null);
  const { state, error, pending, toggleHidden, retry, fail } = usePetWindow(root);
  const pointerStart = useRef<PointerStart | null>(null);
  const draggingStarted = useRef(false);
  const [dragging, setDragging] = useState(false);
  const [startupError, setStartupError] = useState("");
  const appearance = usePetAppearance();
  const activity = usePetActivity();
  const [menuOpen, setMenuOpen] = useState(false);
  const [showProviderError, setShowProviderError] = useState(false);
  const visibleError = error || startupError || appearance.error || activity.error || (showProviderError ? activity.view?.error : "");
  const openPage = async (page: ApplicationPage) => {
    try { await navigation.open(page); }
    catch (cause) { fail(errorMessage(cause)); }
  };

  useEffect(() => {
    let disposed = false;
    void configurationApi.status().catch((cause: unknown) => {
      if (!disposed) setStartupError(`配置存储不可用：${errorMessage(cause)}。处理后请重新启动应用。`);
    });
    return () => { disposed = true; };
  }, []);
  useEffect(() => {
    pointerStart.current = null; draggingStarted.current = false; setDragging(false);
    setMenuOpen(false);
  }, [state.hidden]);
  useEffect(() => { setMenuOpen(false); }, [activity.view?.contextRevision]);
  useEffect(() => {
    const close = (event: KeyboardEvent) => { if (event.key === "Escape") { setMenuOpen(false); setShowProviderError(false); } };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, []);
  useEffect(() => {
    const reset = () => { pointerStart.current = null; draggingStarted.current = false; setDragging(false); };
    window.addEventListener("blur", reset);
    return () => window.removeEventListener("blur", reset);
  }, []);

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || pointerStart.current) return;
    setMenuOpen(false);
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
        pointerStart.current = null; draggingStarted.current = false; setDragging(false);
      }
    });
  };
  const resetPointer = (event: PointerEvent<HTMLDivElement>) => {
    if (pointerStart.current?.pointerId === event.pointerId) {
      pointerStart.current = null; draggingStarted.current = false; setDragging(false);
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
      data-dragging={dragging} data-error={Boolean(visibleError)} data-ready={appearance.ready} style={appearance.style}>
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
      {activity.view?.error && !state.hidden && <button className="ym-button pet-health" data-pet-interactive
        data-variant="danger" aria-label="查看连接错误" aria-expanded={showProviderError}
        onClick={() => { setShowProviderError(value => !value); setMenuOpen(false); }}>!</button>}
      {!state.hidden && <>
        <button className="ym-button pet-open-chat" data-pet-interactive onClick={() => { void openPage("chat"); }}>聊天</button>
        <button className="ym-button pet-open-settings" data-pet-interactive onClick={() => { void openPage("settings"); }}>设置</button>
      </>}
      {visibleError && !state.hidden && <div className="pet-error" data-pet-interactive role="alert" tabIndex={0}>
        <strong><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 6v7m0 3v2"/></svg>暂时遇到一点问题</strong>
        <p>{visibleError}</p>
        {activity.view?.error && <button className="ym-button" disabled={activity.pending} onClick={() => { void activity.refresh(); }}>刷新连接</button>}
      </div>}
      {menuOpen && !state.hidden && <div className="pet-menu" data-pet-interactive aria-label="桌宠互动">
        <p>{activity.view?.intimacy == null ? "当前未选择会话" : `亲密度 ${activity.view.intimacy} / 999${activity.view.intimacy === 999 ? " · 已达上限" : ""}`}</p>
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void activity.interact("feed"); }}>喂食</button>
        <button className="ym-button" disabled={activity.pending || !activity.view?.canInteract}
          onClick={() => { setMenuOpen(false); void activity.interact("stroke"); }}>摸摸</button>
      </div>}
      {!state.hidden && <div data-pet-interactive aria-label="颜文字桌宠" role="img" className="pet-face"
        data-away={activity.view?.activity === "away"}
        onContextMenu={event => { event.preventDefault(); setMenuOpen(value => !value); setShowProviderError(false); }}
        onLostPointerCapture={resetPointer} onPointerCancel={resetPointer}
        onPointerDown={handlePointerDown} onPointerMove={handlePointerMove} onPointerUp={releasePointer}>
        <span className="pet-expression">{activity.view?.expression || "(・ω・)"}</span>
      </div>}
      <PetReminderNotifications hidden={state.hidden} ready={appearance.ready && state.revision >= 0 && !pending}
        unobscured={!visibleError && !menuOpen} />
    </div>
  );
}
