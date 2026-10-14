import { useEffect, useState } from "react";
import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import TravelPanel, { type TravelTab } from "../components/TravelPanel";
import { useCurrentContext } from "../hooks/useCurrentContext";
import { usePetAppearance } from "./usePetAppearance";
import { navigation } from "../platform/navigation";
import { errorMessage } from "../api";

export default function TravelWindow() {
  const { snapshot, loading, error, refresh } = useCurrentContext();
  const appearance = usePetAppearance();
  const [tab, setTab] = useState<TravelTab>("history");
  const [navigationError, setNavigationError] = useState("");
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    const load = async () => {
      try { const mailbox = await navigation.takeTravelRequest(); if (!disposed && mailbox !== null) { setTab(mailbox ? "mailbox" : "history"); setNavigationError(""); } }
      catch (cause) { if (!disposed) setNavigationError(errorMessage(cause)); }
    };
    const reload = () => { if (!disposed) void load(); };
    void navigation.listenTravel(reload).then(unlisten => { if (disposed) unlisten(); else { stop = unlisten; reload(); } })
      .catch(cause => { if (!disposed) { setNavigationError(errorMessage(cause)); reload(); } });
    window.addEventListener("focus", reload);
    return () => { disposed = true; stop?.(); window.removeEventListener("focus", reload); };
  }, []);
  return <ResponsiveWindowShell title="出行与邮箱" subtitle="看看它的旅程，收下寄来的信。"
    contextLabel={snapshot ? `${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}` : "正在读取当前会话"}
    loading={(loading && !snapshot) || !appearance.ready} error={snapshot ? "" : error || appearance.error} onRetry={refresh}>
    {(error || appearance.error || navigationError) && <p role="alert" className="ym-page-error">{error || appearance.error || navigationError}</p>}
    {snapshot && <TravelPanel context={snapshot.context} personaName={snapshot.persona.name} tab={tab} onTabChange={setTab} />}
  </ResponsiveWindowShell>;
}
