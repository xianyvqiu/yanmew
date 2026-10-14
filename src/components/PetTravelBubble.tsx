import { useEffect, useState } from "react";
import { travelApi } from "../api";
import type { PetActivity } from "../types";

/** 出行记录已保存后提示一次；被提醒遮挡或宠窗隐藏时不排队补弹。 */
export default function PetTravelBubble({ view, seenTravelIds }: { view: PetActivity | null; seenTravelIds: Set<string> }) {
  const [text, setText] = useState("");
  useEffect(() => {
    let disposed = false;
    let timeout: ReturnType<typeof setTimeout> | undefined;
    setText("");
    if (view?.activity !== "away" || !view.sessionId) return;
    void travelApi.current({personaId:view.personaId,sessionId:view.sessionId,contextRevision:view.contextRevision}).then(trip => {
      if (disposed || !trip || seenTravelIds.has(trip.id)) return;
      seenTravelIds.add(trip.id); setText("出门啦，想我的时候可以写信给我。回来再陪你聊天 (・ω・)");
      timeout = setTimeout(() => { if (!disposed) setText(""); }, 8000);
    }).catch(() => {});
    return () => { disposed = true; if (timeout) clearTimeout(timeout); };
  }, [view?.personaId, view?.sessionId, view?.contextRevision, view?.activity, seenTravelIds]);
  if (!text || view?.activity !== "away") return null;
  return <div className="pet-state-bubble" role="status" aria-label="出发提示" data-pet-interactive><p>{text}</p></div>;
}
