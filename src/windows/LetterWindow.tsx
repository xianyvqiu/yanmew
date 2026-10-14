import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import LetterEditor from "../components/LetterEditor";
import { useCurrentContext } from "../hooks/useCurrentContext";
import { usePetAppearance } from "./usePetAppearance";
import { navigation } from "../platform/navigation";
import { errorMessage } from "../api";
import type { LetterTarget } from "../types";
import "../pages/workspace.css";

function sameTarget(left: LetterTarget | null, right: LetterTarget) {
  return left?.personaId === right.personaId && left.sessionId === right.sessionId && left.contextRevision === right.contextRevision && left.travelId === right.travelId && left.replyToId === right.replyToId;
}
export default function LetterWindow() {
  const { snapshot, loading, error, refresh } = useCurrentContext();
  const appearance = usePetAppearance();
  const [target, setTarget] = useState<LetterTarget | null>(null);
  const [replacement, setReplacement] = useState<LetterTarget | null>(null);
  const [draft, setDraft] = useState("");
  const [navigationError, setNavigationError] = useState("");
  const targetRef = useRef(target);
  const draftRef = useRef(draft);
  targetRef.current = target; draftRef.current = draft;
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    const load = async () => {
      try {
        const next = await navigation.takeLetterRequest();
        if (disposed || !next) return;
        if (!sameTarget(targetRef.current, next)) {
          if (draftRef.current.trim()) setReplacement(next);
          else { targetRef.current = next; setTarget(next); setReplacement(null); }
        }
        setNavigationError("");
      } catch (cause) { if (!disposed) setNavigationError(errorMessage(cause)); }
    };
    const reload = () => { if (!disposed) void load(); };
    void navigation.listenLetter(reload).then(unlisten => { if (disposed) unlisten(); else { stop = unlisten; reload(); } })
      .catch(cause => { if (!disposed) { setNavigationError(errorMessage(cause)); reload(); } });
    window.addEventListener("focus", reload);
    return () => { disposed = true; stop?.(); window.removeEventListener("focus", reload); };
  }, []);
  return <ResponsiveWindowShell title="写信" subtitle="把想说的话，寄给正在出门的它。"
    contextLabel={snapshot ? `${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}` : "正在读取当前会话"}
    loading={(loading && !snapshot) || !appearance.ready} error={snapshot ? "" : error || appearance.error} onRetry={refresh}>
    {(error || appearance.error || navigationError) && <p role="alert" className="ym-page-error">{error || appearance.error || navigationError}</p>}
    {replacement && <section className="ym-page-card" role="status"><p>另一个写信入口请求了不同的目标。当前草稿已保留，请决定是否清空并切换。</p>
      <div className="ym-inline-form"><button className="ym-button" onClick={() => setReplacement(null)}>保留当前草稿</button><button className="ym-button" onClick={() => { setDraft(""); setTarget(replacement); setReplacement(null); }}>清空草稿并切换</button></div></section>}
    {snapshot && target ? <LetterEditor context={snapshot.context} target={target} value={draft} onChange={setDraft} onCancel={() => getCurrentWindow().close()} />
      : <p className="ym-page-note">请从本次旅行的写信或回信入口打开此窗口。</p>}
  </ResponsiveWindowShell>;
}
