import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState, type PointerEvent } from "react";
import { configurationApi, errorMessage } from "../api";
import { usePetWindow } from "./usePetWindow";
import "./PetWindow.css";

const DRAG_THRESHOLD_PX = 5;

type PointerStart = {
  pointerId: number;
  x: number;
  y: number;
};

export default function PetWindow() {
  const root = useRef<HTMLDivElement>(null);
  const { state, error, pending, toggleHidden, retry, fail } = usePetWindow(root);
  const pointerStart = useRef<PointerStart | null>(null);
  const draggingStarted = useRef(false);
  const [startupError, setStartupError] = useState("");

  useEffect(() => {
    let disposed = false;
    void configurationApi.status().catch((cause: unknown) => {
      if (!disposed) setStartupError(`配置存储不可用：${errorMessage(cause)}。处理后请重新启动应用。`);
    });
    return () => { disposed = true; };
  }, []);

  useEffect(() => {
    pointerStart.current = null;
    draggingStarted.current = false;
  }, [state.hidden]);

  useEffect(() => {
    const reset = () => { pointerStart.current = null; draggingStarted.current = false; };
    window.addEventListener("blur", reset);
    return () => window.removeEventListener("blur", reset);
  }, []);

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || pointerStart.current) {
      return;
    }

    pointerStart.current = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
    };
    draggingStarted.current = false;
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };

  const handlePointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = pointerStart.current;
    if (!start || start.pointerId !== event.pointerId || draggingStarted.current) {
      return;
    }

    const distance = Math.hypot(event.clientX - start.x, event.clientY - start.y);
    if (distance < DRAG_THRESHOLD_PX) {
      return;
    }

    draggingStarted.current = true;
    // 原生拖动可能接管并吞掉 pointerup；结束后不能残留旧指针。
    const activePointer = start;
    void getCurrentWindow().startDragging().catch(fail).finally(() => {
      if (pointerStart.current === activePointer) {
        pointerStart.current = null;
        draggingStarted.current = false;
      }
    });
  };

  const resetPointer = (event: PointerEvent<HTMLDivElement>) => {
    if (pointerStart.current?.pointerId === event.pointerId) {
      pointerStart.current = null;
      draggingStarted.current = false;
    }
  };

  return (
    <div
      ref={root}
      data-testid="pet-window"
      className="pet-window"
      style={{
        alignItems: "center",
        background: "transparent",
        display: "flex",
        height: "100%",
        justifyContent: "center",
        userSelect: "none",
        width: "100%",
      }}
    >
      <button
        className="pet-visibility"
        data-pet-interactive
        disabled={pending || (state.revision < 0 && !error)}
        onClick={() => { void (state.revision < 0 ? retry() : toggleHidden()); }}
        title={error || undefined}
        aria-description={error || undefined}
        style={error ? { color: "#b91c1c" } : undefined}
      >
        {state.revision < 0 ? (error ? "重试连接" : "正在连接…") : state.hidden ? "显示宠窗" : "隐藏宠窗"}
      </button>
      {(error || startupError) && !state.hidden && <div className="pet-error" data-pet-interactive role="alert">{error || startupError}</div>}
      {!state.hidden && <div
        data-pet-interactive
        aria-label="颜文字桌宠"
        onLostPointerCapture={resetPointer}
        onPointerCancel={resetPointer}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={resetPointer}
        role="img"
        className="pet-face"
      >
        (・ω・)
      </div>}
    </div>
  );
}
