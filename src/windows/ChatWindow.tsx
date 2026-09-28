import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import { useCurrentContext } from "../hooks/useCurrentContext";
import ChatPage from "../pages/ChatPage";
import { usePetAppearance } from "./usePetAppearance";

export default function ChatWindow() {
  const { snapshot, loading, error, refresh } = useCurrentContext();
  const appearance = usePetAppearance();
  return <ResponsiveWindowShell title="聊天" subtitle="让每一次相伴，都有迹可循。"
    contextLabel={snapshot ? `当前人设：${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}` : "正在读取当前人设"}
    loading={(loading && !snapshot) || !appearance.ready} error={error || appearance.error} onRetry={refresh}>
    {snapshot && <ChatPage key={snapshot.context.contextRevision} snapshot={snapshot} onRefresh={refresh} />}
  </ResponsiveWindowShell>;
}
