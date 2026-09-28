import ResponsiveWindowShell from "../components/ResponsiveWindowShell";
import GeneralPanel from "../components/settings/GeneralPanel";
import type { WorkspaceContext } from "../hooks/useCurrentContext";
import "./workspace.css";

const groups = [
  { title: "全局设置", items: ["提供商", "人设", "用户设定", "提醒", "时间与天气", "记忆", "通用"] },
  { title: "桌宠与内容", items: ["桌宠状态", "游戏中心", "插件管理", "时间与桌宠", "出行", "饰品", "节日"] },
];
export default function SettingsPage({ snapshot, loading, error, onRefresh }: {
  snapshot: WorkspaceContext | null; loading: boolean; error: string; onRefresh: () => void;
}) {
  return <ResponsiveWindowShell title="设置" subtitle="把日常陪伴，调成你喜欢的样子。"
    contextLabel={snapshot ? `当前人设：${snapshot.persona.name} · ${snapshot.sessions.find(item => item.id === snapshot.context.sessionId)?.title ?? "尚无会话"}` : "正在读取当前人设"}
    loading={loading} error={error} onRetry={onRefresh}
    sidebar={<><label className="ym-settings-compact">设置页面<select value="通用" onChange={() => {}} aria-label="设置页面">{groups.map(group => <optgroup key={group.title} label={group.title}>{group.items.map(item => <option key={item} value={item} disabled={item !== "通用"}>{item}{item !== "通用" && " · 未开放"}</option>)}</optgroup>)}</select></label>
    <nav className="ym-settings-nav" aria-label="设置导航">{groups.map(group => <div key={group.title}>
      <h2>{group.title}</h2>{group.items.map(item => <button key={item} className="ym-button" disabled={item !== "通用"}
        aria-current={item === "通用" ? "page" : undefined} title={item !== "通用" ? "尚未开放" : undefined}>{item}{item !== "通用" && " · 未开放"}</button>)}
    </div>)}</nav></>}>
    {snapshot && <div className="ym-workspace-page"><GeneralPanel /></div>}
  </ResponsiveWindowShell>;
}
