import { useEffect, useRef, useState } from "react";
import { petApi } from "../api";
import type { PetActivity, PetStateNotice } from "../types";

function matches(notice: PetStateNotice, view: PetActivity) {
  return notice.personaId === view.personaId && notice.sessionId === view.sessionId
    && notice.contextRevision === view.contextRevision && notice.activity === view.activity;
}

/** 临时提示只消费实时事件；不补查历史、不请求 AI、不建立第二份桌宠计时。 */
export default function PetStateBubble({ view, durationMs }: { view: PetActivity | null; durationMs: number }) {
  const [notice, setNotice] = useState<PetStateNotice | null>(null);
  const current = useRef(view);
  current.current = view;
  const newest = useRef(-1);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void petApi.listenStateBubble(next => {
      if (disposed || next.revision <= newest.current) return;
      const active = current.current;
      if (!active || next.revision < active.revision || next.contextRevision < active.contextRevision) return;
      newest.current = next.revision;
      setNotice(next);
    }).then(unlisten => { if (disposed) unlisten(); else stop = unlisten; })
      // 普通临时提示监听失败不覆盖现有存储、提供商或提醒错误入口。
      .catch(() => {});
    return () => { disposed = true; stop?.(); };
  }, []);
  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(null), durationMs);
    return () => window.clearTimeout(timeout);
  }, [notice, durationMs]);
  useEffect(() => {
    // 活动快照可能略晚于提示事件；只在相应版本到达后判断提示是否失效。
    if (notice && (!view || (view.revision >= notice.revision && (!matches(notice, view) || !view.canWander)))) {
      setNotice(null);
    }
  }, [notice, view]);
  if (!notice || !view || view.revision < notice.revision || !view.canWander || !matches(notice, view)) return null;
  return <div className="pet-state-bubble" role="status" aria-label="桌宠状态提示" data-pet-interactive data-activity={notice.activity}>
    <p>{notice.text}</p>
  </div>;
}
