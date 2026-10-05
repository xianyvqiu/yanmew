import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import MemoryManagementPanel from "../components/settings/MemoryManagementPanel";
import MemoryGenerationPanel from "../components/settings/MemoryGenerationPanel";
import { useCurrentContext } from "../hooks/useCurrentContext";
import { usePetAppearance } from "./usePetAppearance";

export default function MemoryWindow() {
  const { snapshot, loading, error, refresh } = useCurrentContext();
  const appearance = usePetAppearance();
  const contextLabel = snapshot
    ? `当前人设：${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}`
    : "正在读取当前人设";

  return <ResponsiveWindowShell title="记忆" subtitle="当前会话记事本、短期与长期记忆。" contextLabel={contextLabel}
    loading={(loading && !snapshot) || !appearance.ready} error={snapshot ? "" : error || appearance.error} onRetry={refresh}>
    {snapshot && (error || appearance.error) && <div role="alert" className="ym-page-error">
      {error || appearance.error}<button className="ym-button" type="button" onClick={refresh}>重新加载</button>
    </div>}
    {snapshot && <MemoryManagementPanel context={snapshot.context} />}
    {snapshot && <MemoryGenerationPanel key={`generation:${snapshot.context.contextRevision}`} context={snapshot.context} />}
  </ResponsiveWindowShell>;
}
