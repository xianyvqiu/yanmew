import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import { useCurrentContext } from "../hooks/useCurrentContext";
import ChatPage from "../pages/ChatPage";
import { usePetAppearance } from "./usePetAppearance";
import ChatBackground from "../components/ChatBackground";
import { chatAppearanceAttributes } from "../components/chatAppearance";
import { useState } from "react";
import { navigation } from "../platform/navigation";
import { errorMessage } from "../api";

export default function ChatWindow() {
  const { snapshot, loading, error, refresh } = useCurrentContext();
  const appearance = usePetAppearance();
  const [navigationError, setNavigationError] = useState("");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => window.innerWidth < 680);
  return <ResponsiveWindowShell compact title="聊天详情" subtitle="让每一次相伴，都有迹可循。"
    contextLabel={snapshot ? `当前人设：${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}` : "正在读取当前人设"}
    contextContent={snapshot ? <><button type="button" className="ym-context-link" aria-label={`打开人设详情：${snapshot.persona.name}`} onClick={() => {
      setNavigationError(""); void navigation.openSettingsDetail("persona", snapshot.persona.id).catch(cause => setNavigationError(errorMessage(cause)));
    }}>当前人设：{snapshot.persona.name}</button><span> · {snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}</span></> : undefined}
    loading={(loading && !snapshot) || !appearance.ready} error={snapshot ? "" : error || appearance.error} onRetry={refresh}>
    {snapshot && (error || appearance.error) && <div role="alert" className="ym-page-error">{error || appearance.error}<button className="ym-button" onClick={refresh}>重新加载</button></div>}
    {navigationError && <p role="alert" className="ym-page-error">详情打开失败：{navigationError}</p>}
    {snapshot && <div className="ym-chat-background-host ym-chat-bubbles" {...chatAppearanceAttributes(snapshot.persona.chatAppearance)}>
      <ChatBackground persona={snapshot.persona} contextRevision={snapshot.context.contextRevision} />
      <ChatPage key={snapshot.context.contextRevision} snapshot={snapshot} onRefresh={refresh} sidebarCollapsed={sidebarCollapsed} onSidebarCollapsed={setSidebarCollapsed} />
    </div>}
  </ResponsiveWindowShell>;
}
