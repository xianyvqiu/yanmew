import { useEffect, useRef, type CSSProperties, type RefObject } from "react";
import {petContact,visibleContactBounds,petHintPosition} from "./petContact";
import { petApi } from "../api";
import type { PetActivity } from "../types";

/** 上报真实身体/饰品边界与暂停资格；原生层移动窗口，不叠加画布内位移。 */
export function usePetWander(root: RefObject<HTMLDivElement>, face: RefObject<HTMLDivElement>, view: PetActivity | null, enabled: boolean) {
  const latest = useRef({ view, enabled }); latest.current = { view, enabled };
  const scheduleRef = useRef<() => void>(() => {});
  useEffect(() => {
    let frame = 0, disposed = false, revision = Date.now(), previous = "";
    const report = () => {
      frame = 0;
      const canvas = root.current, pet = face.current, { view, enabled } = latest.current;
      if (disposed || !canvas || !pet) return;
      const c = canvas.getBoundingClientRect(), p = pet.getBoundingClientRect();
      if (c.width <= 0 || c.height <= 0 || p.width <= 0 || p.height <= 0) return;
      const anchor = { x: p.left - c.left, y: p.top - c.top, width: p.width, height: p.height };
      const contact=petContact(pet,c),visible=visibleContactBounds(contact);
      const obstacles=Array.from(canvas.querySelectorAll<HTMLElement>(".pet-more,.pet-visibility,.pet-health,.pet-chat-toggle")).map(node=>{const r=node.getBoundingClientRect();return {x:r.left-c.left,y:r.top-c.top,width:r.width,height:r.height};}).filter(r=>r.width>0&&r.height>0);
      const hint=petHintPosition(visible,c,obstacles);
      const variables={"--pet-body-height":`${p.height}px`,"--pet-hint-left":`${hint.x}px`,"--pet-hint-top":`${hint.y}px`};
      for(const [name,value] of Object.entries(variables))if(canvas.style.getPropertyValue(name)!==value)canvas.style.setProperty(name,value);
      const freeY=c.height-p.height-56-(parseFloat(getComputedStyle(canvas).getPropertyValue("--pet-chat-extra"))||0);
      const bodyTop = `${anchor.y}px`;
      if (canvas.style.getPropertyValue("--pet-body-top") !== bodyTop) canvas.style.setProperty("--pet-body-top", bodyTop);
      if (canvas.dataset.dock?.startsWith("rest_")) {
        const height = `${p.height}px`;
        if (canvas.style.getPropertyValue("--dock-measured-body-height") !== height) canvas.style.setProperty("--dock-measured-body-height", height);
      } else if (canvas.style.getPropertyValue("--dock-measured-body-height")) canvas.style.removeProperty("--dock-measured-body-height");
      if (canvas.dataset.dock?.startsWith("peek_")) {
        const width = `${p.width * .78}px`;
        if (canvas.style.getPropertyValue("--dock-visible-body-width") !== width) canvas.style.setProperty("--dock-visible-body-width", width);
      } else if (canvas.style.getPropertyValue("--dock-visible-body-width")) canvas.style.removeProperty("--dock-visible-body-width");
      if (!view?.sessionId) return;
      let left = anchor.x, top = anchor.y, right = left + p.width, bottom = top + p.height;
      canvas.querySelectorAll<HTMLElement>(".pet-accessory-layer,.pet-reminder-slot,.pet-petting-hint").forEach(node => {
        if (!node.childElementCount && !node.classList.contains("pet-petting-hint")) return;
        const r = node.getBoundingClientRect();
        left = Math.min(left, r.left - c.left); top = Math.min(top, r.top - c.top);
        right = Math.max(right, r.right - c.left); bottom = Math.max(bottom, r.bottom - c.top);
      });
      const overlayOpen = Boolean(canvas.querySelector(".pet-reminder-slot > *,.pet-menu,.pet-options-menu,.pet-chat-input,.pet-food-picker,.pet-error"));
      const payload = { personaId: view.personaId, sessionId: view.sessionId, contextRevision: view.contextRevision,
        area: { x: 0, y: 0, width: c.width, height: c.height }, body: { x: left, y: top, width: right - left, height: bottom - top }, anchor, contact, freeY,
        offset: { x: 0, y: 0 }, paused: !enabled || !view.canWander || overlayOpen || document.visibilityState === "hidden" || window.matchMedia("(prefers-reduced-motion: reduce)").matches };
      const key = JSON.stringify(payload); if (key === previous) return; previous = key;
      void petApi.updateWanderLayout({ ...payload, layoutRevision: ++revision }).catch(() => { if (!disposed) previous = ""; });
    };
    const schedule = () => { if (!disposed && !frame) frame = requestAnimationFrame(report); };
    scheduleRef.current = schedule;
    const observer = new ResizeObserver(schedule), mutation = new MutationObserver(schedule);
    if (root.current) { observer.observe(root.current); mutation.observe(root.current, { childList: true, subtree: true, attributes: true, characterData: true }); }
    if (face.current) observer.observe(face.current);
    const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
    root.current?.addEventListener("load",schedule,true);
    document.addEventListener("visibilitychange", schedule); window.addEventListener("resize", schedule); reduced.addEventListener("change", schedule); schedule();
    return () => { disposed = true; cancelAnimationFrame(frame); observer.disconnect(); mutation.disconnect(); root.current?.removeEventListener("load",schedule,true); document.removeEventListener("visibilitychange", schedule); window.removeEventListener("resize", schedule); reduced.removeEventListener("change", schedule); scheduleRef.current = () => {}; };
  }, [root, face]);
  useEffect(() => { scheduleRef.current(); }, [view?.personaId, view?.sessionId, view?.contextRevision, view?.canWander, enabled]);
  return { "--ym-wander-x": "0px", "--ym-wander-y": "0px" } as CSSProperties;
}
