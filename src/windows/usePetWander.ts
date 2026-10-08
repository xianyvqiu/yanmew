import { useEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import { petApi } from "../api";
import type { PetActivity, PetWanderLayout, PetWanderPoint } from "../types";

const FRAME_MS = 100;
const STEPS = 6;
/** 临时显示偏移，不持久化；每步样式变化由既有 usePetWindow 同步原生命中区域。 */
export function usePetWander(root: RefObject<HTMLDivElement>, face: RefObject<HTMLDivElement>,
  view: PetActivity | null, enabled: boolean) {
  const [offset, setOffset] = useState<PetWanderPoint>({ x: 0, y: 0 });
  const actual = useRef(offset); actual.current = offset;
  const current = useRef({ view, enabled }); current.current = { view, enabled };
  const layout = useRef<PetWanderLayout | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const scheduleLayout = useRef<() => void>(() => {});
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    let frame = 0;
    let previous = "";
    let revision = Date.now();
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
    const pause = () => { clearTimeout(timer.current); timer.current = undefined; };
    const allowed = () => current.current.enabled && Boolean(current.current.view?.canWander)
      && document.visibilityState !== "hidden" && !reduced.matches;
    const report = () => {
      frame = 0;
      const { view } = current.current;
      const canvas = root.current; const pet = face.current;
      if (disposed || !canvas || !pet || !view?.sessionId) return;
      const canvasRect = canvas.getBoundingClientRect();
      const petRect = pet.getBoundingClientRect();
      if (canvasRect.width <= 8 || canvasRect.height <= 48 || petRect.width <= 0 || petRect.height <= 0) return;
      const p = actual.current;
      let left = petRect.left - canvasRect.left - p.x;
      let top = petRect.top - canvasRect.top - p.y;
      let right = left + petRect.width; let bottom = top + petRect.height;
      const bubble = canvas.querySelector<HTMLElement>(".pet-reminder-slot");
      if (bubble?.childElementCount) {
        const r = bubble.getBoundingClientRect();
        left = Math.min(left, r.left - canvasRect.left - p.x);
        top = Math.min(top, r.top - canvasRect.top - p.y);
        right = Math.max(right, r.right - canvasRect.left - p.x);
        bottom = Math.max(bottom, r.bottom - canvasRect.top - p.y);
      }
      const area = { x: 4, y: 44, width: canvasRect.width - 8, height: canvasRect.height - 48 };
      const clamp = (value: number, low: number, high: number) => low > high ? 0 : Math.min(high, Math.max(low, value));
      const bounded = { x: clamp(p.x, area.x - left, area.x + area.width - right),
        y: clamp(p.y, area.y - top, area.y + area.height - bottom) };
      if (bounded.x !== p.x || bounded.y !== p.y) { pause(); actual.current = bounded; setOffset(bounded); }
      const next = { personaId: view.personaId, sessionId: view.sessionId, contextRevision: view.contextRevision,
        area, body: { x: left, y: top, width: right - left, height: bottom - top }, offset: bounded, paused: !allowed() };
      const key = JSON.stringify(next);
      if (key === previous) return;
      previous = key;
      const submitted = { ...next, layoutRevision: ++revision };
      layout.current = submitted;
      void petApi.updateWanderLayout(submitted).catch(() => {
        // 布局未被接受时保持静止；下一次尺寸/上下文变化重新提交。
        if (!disposed && layout.current === submitted) { pause(); layout.current = null; previous = ""; }
      });
    };
    const schedule = () => { if (!disposed && !frame) frame = requestAnimationFrame(report); };
    scheduleLayout.current = schedule;
    const visibility = () => { if (!allowed()) pause(); schedule(); };
    const resize = new ResizeObserver(schedule);
    const observe = () => {
      resize.disconnect();
      if (root.current) resize.observe(root.current);
      if (face.current) resize.observe(face.current);
      const bubble = root.current?.querySelector(".pet-reminder-slot");
      if (bubble) resize.observe(bubble);
      schedule();
    };
    const mutation = new MutationObserver(records => {
      // root 的游走 CSS 变量每步都变化；不用把它当成新的布局请求。
      if (records.some(r => r.target !== root.current || r.type !== "attributes" || r.attributeName !== "style")) observe();
    });
    if (root.current) mutation.observe(root.current, { attributes: true, childList: true, subtree: true, characterData: true });
    observe();
    document.addEventListener("visibilitychange", visibility);
    reduced.addEventListener("change", visibility);
    window.addEventListener("resize", schedule);
    void petApi.listenWander(target => {
      const captured = layout.current;
      const view = current.current.view;
      if (disposed || !allowed() || !captured || !view || target.personaId !== view.personaId
        || target.sessionId !== view.sessionId || target.contextRevision !== view.contextRevision
        || target.layoutRevision !== captured.layoutRevision || !Number.isFinite(target.offset.x) || !Number.isFinite(target.offset.y)) return;
      const { body, area } = captured;
      if (body.x + target.offset.x < area.x || body.y + target.offset.y < area.y
        || body.x + body.width + target.offset.x > area.x + area.width
        || body.y + body.height + target.offset.y > area.y + area.height) return;
      pause();
      const start = actual.current;
      let step = 0;
      const advance = () => {
        if (disposed || !allowed() || layout.current !== captured) return;
        const fraction = ++step / STEPS;
        const next = { x: start.x + (target.offset.x - start.x) * fraction,
          y: start.y + (target.offset.y - start.y) * fraction };
        actual.current = next; setOffset(next);
        if (step < STEPS) timer.current = setTimeout(advance, FRAME_MS);
        else { timer.current = undefined; schedule(); }
      };
      timer.current = setTimeout(advance, FRAME_MS);
    }).then(unlisten => { if (disposed) unlisten(); else stop = unlisten; }).catch(() => {});
    return () => {
      disposed = true; pause(); cancelAnimationFrame(frame); stop?.(); resize.disconnect(); mutation.disconnect();
      document.removeEventListener("visibilitychange", visibility); reduced.removeEventListener("change", visibility);
      window.removeEventListener("resize", schedule); scheduleLayout.current = () => {};
    };
  }, [root, face]);
  useEffect(() => {
    clearTimeout(timer.current); timer.current = undefined;
    scheduleLayout.current();
  }, [view?.personaId, view?.sessionId, view?.contextRevision, view?.canWander, enabled]);
  return { "--ym-wander-x": `${offset.x}px`, "--ym-wander-y": `${offset.y}px` } as CSSProperties;
}
