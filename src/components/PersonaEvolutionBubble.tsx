import { useEffect, useRef, useState, type ReactNode } from "react";
import { configurationApi } from "../api";
import type { PersonaUpdatedNotice, PetActivity } from "../types";

function matches(update: PersonaUpdatedNotice, view: PetActivity | null) {
  return Boolean(view && update.personaId === view.personaId && update.sessionId === view.sessionId
    && update.contextRevision === view.contextRevision);
}

/** 只展示已提交的实时通知；复用普通气泡插槽，隐藏或提醒遮挡时不补弹。 */
export default function PersonaEvolutionBubble({ view, fallback }: { view: PetActivity | null; fallback: ReactNode }) {
  const [update, setUpdate] = useState<PersonaUpdatedNotice | null>(null);
  const current = useRef(view);
  current.current = view;
  const seen = useRef(new Set<string>());
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void configurationApi.listenPersonaUpdates(next => {
      const key = `${next.personaId}:${next.evolvedAt}`;
      if (disposed || seen.current.has(key) || !matches(next, current.current)) return;
      seen.current.add(key);
      setUpdate(next);
    }).then(unlisten => { if (disposed) unlisten(); else stop = unlisten; }).catch(() => {});
    return () => { disposed = true; stop?.(); };
  }, []);
  useEffect(() => {
    if (!update) return;
    const timeout = window.setTimeout(() => setUpdate(null), 8000);
    return () => window.clearTimeout(timeout);
  }, [update]);
  useEffect(() => {
    if (update && (!matches(update, view) || ["away", "sleeping", "dormant"].includes(view?.activity ?? ""))) setUpdate(null);
  }, [update, view]);
  // 完成通知可能先于下一份活动快照；待聊天活动解除后再显示，过期仍自动收起。
  if (!update || !matches(update, view) || !view?.canWander) return <>{fallback}</>;
  return <div className="pet-state-bubble" role="status" aria-label="人设自动调整提示" data-pet-interactive>
    <p>人设已自动调整，可在设置的人设页面查看。</p>
  </div>;
}
