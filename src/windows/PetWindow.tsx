import { getCurrentWindow } from "@tauri-apps/api/window";
import { useRef, type PointerEvent } from "react";

const DRAG_THRESHOLD_PX = 5;

type PointerStart = {
  pointerId: number;
  x: number;
  y: number;
};

export default function PetWindow() {
  const pointerStart = useRef<PointerStart | null>(null);
  const draggingStarted = useRef(false);

  const handlePointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
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
    void getCurrentWindow().startDragging();
  };

  const resetPointer = (event: PointerEvent<HTMLDivElement>) => {
    if (pointerStart.current?.pointerId === event.pointerId) {
      pointerStart.current = null;
      draggingStarted.current = false;
    }
  };

  return (
    <div
      data-testid="pet-window"
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
      <div
        aria-label="颜文字桌宠"
        onPointerCancel={resetPointer}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={resetPointer}
        role="img"
        style={{ cursor: "grab", fontSize: 48, lineHeight: 1, touchAction: "none" }}
      >
        (・ω・)
      </div>
    </div>
  );
}
