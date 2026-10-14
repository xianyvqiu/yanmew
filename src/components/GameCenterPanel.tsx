import { useEffect, useRef, useState } from "react";
import { errorMessage, gamePluginsApi } from "../api";
import { useGamePlugins } from "../hooks/useGamePlugins";
import type { GamePermissionChoice, GamePlugin, GamePluginDetail } from "../types";
import GamePermissions from "./GamePermissions";
import "./GamePlugins.css";

const requiresWallet = (plugin: GamePlugin) => plugin.pluginId === "com.yanmew.game.my-home";
export default function GameCenterPanel() {
  const { plugins, wallet, error, loading, refresh } = useGamePlugins();
  const [busy, setBusy] = useState(false);
  const lock = useRef(false);
  const [actionError, setActionError] = useState("");
  const [permission, setPermission] = useState<GamePluginDetail | null>(null);
  const [choices, setChoices] = useState<GamePermissionChoice[]>([]);
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { if (permission) dialog.current?.showModal(); else dialog.current?.close(); }, [permission]);
  const run = async (action: () => Promise<void>) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setActionError("");
    try { await action(); } catch (cause) { setActionError(errorMessage(cause)); }
    finally { lock.current = false; setBusy(false); }
  };
  const open = (plugin: GamePlugin) => run(async () => {
    const detail = await gamePluginsApi.detail(plugin.pluginId);
    if (detail.permissions.some(item => item.decision === "pending") || requiresWallet(plugin) && !detail.permissions.some(item => item.permission === "wallet.modify" && item.decision === "allowed")) {
      setPermission(detail); setChoices(detail.permissions.map(({permission, scope, decision}) => ({permission, scope, decision}))); return;
    }
    await gamePluginsApi.open(plugin.pluginId);
  });
  const authorize = () => run(async () => {
    if (!permission) return;
    for (const choice of choices) await gamePluginsApi.permission(permission.plugin.pluginId, permission.plugin.installedVersion, choice);
    await gamePluginsApi.open(permission.plugin.pluginId);
    setPermission(null); await refresh();
  });
  return <>
    <section className="ym-page-card"><div className="ym-page-heading"><h2>游戏中心</h2><span>全局喵币 · {wallet?.coins ?? "读取中…"}</span></div>
      <p className="ym-page-note">游戏和存档不随人设切换。再次打开会聚焦已有游戏窗口；各游戏自己的次数限制由插件保存。</p>
      {loading && <p>正在读取游戏…</p>}{error && <p role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={() => void refresh()}>重试</button></p>}
      {actionError && <p role="alert" className="ym-page-error">{actionError}</p>}
      <div className="ym-game-grid">{plugins.map(plugin => <article key={plugin.pluginId} className="ym-game-card"><h3>{plugin.name}</h3><p>{plugin.description}</p>
        <p className="ym-page-note">{plugin.author} · v{plugin.installedVersion}{plugin.isBundled && " · 随应用提供"}</p>
        {plugin.packageStatus === "missing_package" && <p className="ym-page-note">存档已保留，请到插件管理重新安装程序包。</p>}
        {!plugin.isEnabled && plugin.packageStatus === "available" && <p className="ym-page-note">已停用，可到插件管理启用。</p>}
        <button className="ym-button" disabled={busy || !plugin.isEnabled || plugin.packageStatus !== "available"} onClick={() => void open(plugin)}>打开 {plugin.name}</button>
      </article>)}</div>
      {!loading && plugins.length === 0 && <p className="ym-page-empty">暂无游戏，请到插件管理安装本地插件包。</p>}
    </section>
    <dialog ref={dialog} className="ym-session-dialog" aria-labelledby="game-permission-title" onCancel={event => { if (busy) event.preventDefault(); else setPermission(null); }}>
      {permission && <><h2 id="game-permission-title">{permission.plugin.name} · 权限确认</h2><div className="ym-session-dialog-body">
        <p>允许修改喵币后，游戏可以增减整个应用的钱包余额。联网授权只对列出的来源生效。</p>
        <p>{requiresWallet(permission.plugin) ? "我的家必须允许钱包权限才能游玩，不提供纯玩模式。" : "拒绝钱包权限可纯玩，不会增加或扣除喵币。"}</p>
        <GamePermissions choices={choices} disabled={busy} onChange={setChoices} />
        {actionError && <p role="alert" className="ym-page-error">{actionError}</p>}
      </div><div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setPermission(null)}>取消</button>
        <button className="ym-button" disabled={busy || choices.some(item => item.decision === "pending") || requiresWallet(permission.plugin) && !choices.some(item => item.permission === "wallet.modify" && item.decision === "allowed")} onClick={() => void authorize()}>保存权限并打开</button>
      </div></>}
    </dialog>
  </>;
}
