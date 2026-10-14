import { useEffect, useState } from "react";
import { navigation } from "../platform/navigation";
import { errorMessage } from "../api";
import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import PersonasPanel from "../components/settings/PersonasPanel";
import ReminderPanel from "../components/settings/ReminderPanel";
import ProvidersPanel from "../components/settings/ProvidersPanel";
import GeneralPanel from "../components/settings/GeneralPanel";
import WeatherPanel from "../components/settings/WeatherPanel";
import PetTimingPanel from "../components/settings/PetTimingPanel";
import MemorySettingsPanel from "../components/settings/MemorySettingsPanel";
import UserProfilePanel from "../components/settings/UserProfilePanel";
import GameCenterPanel from "../components/GameCenterPanel";
import PluginManagerPanel from "../components/PluginManagerPanel";
import PetStatusPanel from "../components/settings/PetStatusPanel";
import AccessoryPanel from "../components/settings/AccessoryPanel";
import TravelPanel from "../components/TravelPanel";
import FestivalPanel from "../components/settings/FestivalPanel";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import "./workspace.css";

const groups = [
  { title: "全局设置", items: ["提供商", "人设", "用户设定", "提醒", "时间与天气", "记忆", "通用"] },
  { title: "桌宠与内容", items: ["桌宠状态", "游戏中心", "插件管理", "时间与桌宠", "出行", "饰品", "节日"] },
];
export default function SettingsPage({ snapshot, loading, error, onRefresh }: {
  snapshot: WorkspaceContext | null; loading: boolean; error: string; onRefresh: () => void;
}) {
  const [page, setPage] = useState("通用");
  const [navigationError, setNavigationError] = useState("");
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    const read = async () => {
      try {
        const requested = await navigation.takeSettingsRequest();
        if (!disposed && requested) setPage(requested === "accessories" ? "饰品" : "提醒");
        if (!disposed) setNavigationError("");
      } catch (cause) { if (!disposed) setNavigationError(errorMessage(cause)); }
    };
    const reload = () => { if (!disposed) void read(); };
    void navigation.listenSettings(reload).then(unlisten => {
      if (disposed) { unlisten(); return; }
      stop = unlisten; reload();
    }).catch(cause => { if (!disposed) { setNavigationError(errorMessage(cause)); reload(); } });
    window.addEventListener("focus", reload);
    return () => { disposed = true; stop?.(); window.removeEventListener("focus", reload); };
  }, []);
  const enabled = (item: string) => item === "节日" || item === "饰品" || item === "出行" || item === "通用" || item === "提供商" || item === "人设" || item === "用户设定" || item === "提醒" || item === "时间与天气" || item === "时间与桌宠" || item === "记忆" || item === "桌宠状态" || item === "游戏中心" || item === "插件管理";
  return <ResponsiveWindowShell title="设置" subtitle="把日常陪伴，调成你喜欢的样子。"
    contextLabel={snapshot ? `当前人设：${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}` : "正在读取当前人设"}
    loading={loading} error={snapshot ? "" : error} onRetry={onRefresh}
    sidebar={<><label className="ym-settings-compact">设置页面<select value={page} onChange={event => setPage(event.target.value)} aria-label="设置页面">{groups.map(group => <optgroup key={group.title} label={group.title}>{group.items.map(item => <option key={item} value={item} disabled={!enabled(item)}>{item}{!enabled(item) && " · 未开放"}</option>)}</optgroup>)}</select></label>
    <nav className="ym-settings-nav" aria-label="设置导航">{groups.map(group => <div key={group.title}>
      <h2>{group.title}</h2>{group.items.map(item => <button key={item} className="ym-button" disabled={!enabled(item)} onClick={() => setPage(item)}
        aria-current={item === page ? "page" : undefined} title={!enabled(item) ? "尚未开放" : undefined}>{item}{!enabled(item) && " · 未开放"}</button>)}
    </div>)}</nav></>}>
    {snapshot && <div className="ym-workspace-page">{navigationError && <p role="alert" className="ym-page-error">设置页面跳转失败：{navigationError}，可从左侧选择目标页面。</p>}{error && <div role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={onRefresh}>重新加载</button></div>}{page === "饰品" ? <AccessoryPanel key={`${snapshot.context.personaId}:${snapshot.context.sessionId}:${snapshot.context.contextRevision}`} context={snapshot.context} /> : page === "出行" ? <TravelPanel context={snapshot.context} personaName={snapshot.persona.name} /> : page === "提供商" ? <ProvidersPanel /> : page === "人设" ? <PersonasPanel /> : page === "用户设定" ? <UserProfilePanel /> : page === "提醒" ? <ReminderPanel /> : page === "时间与天气" ? <WeatherPanel key={snapshot.context.contextRevision} contextRevision={snapshot.context.contextRevision} /> : page === "时间与桌宠" ? <PetTimingPanel /> : page === "记忆" ? <MemorySettingsPanel /> : page === "游戏中心" ? <GameCenterPanel /> : page === "插件管理" ? <PluginManagerPanel /> : page === "节日" ? <FestivalPanel key={`${snapshot.context.personaId}:${snapshot.context.sessionId}:${snapshot.context.contextRevision}`} context={snapshot.context} /> : page === "桌宠状态" ? <PetStatusPanel snapshot={snapshot} /> : <GeneralPanel />}</div>}
  </ResponsiveWindowShell>;
}
