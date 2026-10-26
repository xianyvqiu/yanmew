import MotionDetails from "./MotionDetails";
import GameSaveSummary from "./GameSaveSummary";
import CardBrowserControls from "./CardBrowserControls";
import {useCardBrowser} from "../hooks/useCardBrowser";
import { useEffect, useRef, useState } from "react";
import { errorMessage, gamePluginsApi, webSearchApi } from "../api";
import { useGamePlugins } from "../hooks/useGamePlugins";
import type { GameInstallPreview, GamePermissionChoice, GamePlugin, GamePluginDetail, GamePluginLedger, GameRun, PluginJson, PluginSetting } from "../types";
import GamePermissions from "./GamePermissions";
import "./GamePlugins.css";

const requiresWallet = (plugin: GamePlugin) => plugin.pluginId === "com.yanmew.game.my-home";

type Confirmation = "clear" | "uninstall" | null;
function SettingField({ setting, value, disabled, onSave }: {
  setting: PluginSetting; value: PluginJson; disabled: boolean; onSave: (value: PluginJson) => Promise<void>;
}) {
  const [draft, setDraft] = useState(value);
  const [invalid, setInvalid] = useState(false);
  // 相同JSON值的刷新不重置草稿；对象/数组每次读取会产生新的引用。
  const savedValue = JSON.stringify(value);
  useEffect(() => { setDraft(JSON.parse(savedValue) as PluginJson); setInvalid(false); }, [savedValue]);
  return <div className="ym-inline-form"><label className="ym-field">{setting.title}
    {setting.type === "boolean" ? <input type="checkbox" role="switch" checked={draft===true} disabled={disabled} onChange={event=>setDraft(event.target.checked)}/>
      : setting.type === "enum" ? <select value={JSON.stringify(draft)} disabled={disabled} onChange={event => setDraft(JSON.parse(event.target.value) as PluginJson)}>{setting.options.map((option, index) => <option key={index} value={JSON.stringify(option.value)}>{option.label}</option>)}</select>
        : <input type={setting.type === "string" ? "text" : "number"} value={String(draft ?? "")} disabled={disabled} min={setting.minimum ?? undefined} max={setting.maximum ?? undefined} step={setting.type === "integer" ? 1 : "any"} onChange={event => {
          const input = event.target.value;
          const numeric = setting.type !== "string";
          setInvalid(numeric && (input.trim() === "" || !Number.isFinite(Number(input))));
          setDraft(numeric ? Number(input) : input);
        }} />}
    {setting.description && <span className="ym-page-note">{setting.description}</span>}
  </label><button className="ym-button" disabled={disabled || invalid} onClick={() => void onSave(draft)}>保存 {setting.title}</button></div>;
}

export default function PluginManagerPanel() {
  const { plugins, wallet, error: listError, loading, refresh } = useGamePlugins();
  const [launchDetail, setLaunchDetail] = useState<GamePluginDetail | null>(null);
  const [launchChoices, setLaunchChoices] = useState<GamePermissionChoice[]>([]);
  const launchDialog = useRef<HTMLDialogElement>(null);
  const [selected, setSelected] = useState("");
  const pendingCardAction=useRef<{id:string;kind:"clear"|"uninstall"}|null>(null);
  const browser=useCardBrowser(plugins,p=>`${p.name} ${p.description} ${p.author} ${p.pluginId}`,p=>p.installedAt,p=>p.pluginId);
  const [detail, setDetail] = useState<GamePluginDetail | null>(null);
  const [choices, setChoices] = useState<GamePermissionChoice[]>([]);
  const [preview, setPreview] = useState<GameInstallPreview | null>(null);
  const [installChoices, setInstallChoices] = useState<GamePermissionChoice[]>([]);
  const [confirmation, setConfirmation] = useState<Confirmation>(null);
  const [resetPluginId, setResetPluginId] = useState("");
  const [clearOnUninstall, setClearOnUninstall] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [runs, setRuns] = useState<GameRun[]>([]);
  const [ledger, setLedger] = useState<GamePluginLedger[]>([]);
  const [runsPage,setRunsPage]=useState(1), [ledgerPage,setLedgerPage]=useState(1);
  const [runsMore, setRunsMore] = useState(false);
  const [ledgerMore, setLedgerMore] = useState(false);
  const lock = useRef(false);
  const generation = useRef(0);
  const historyDepth = useRef({ runs: 1, ledger: 1 });
  const installDialog = useRef<HTMLDialogElement>(null);
  const confirmDialog = useRef<HTMLDialogElement>(null);
  const previewToken = useRef<string | null>(null);
  const choicesDirty = useRef(false);
  const previousSelected = useRef("");
  const fileInput = useRef<HTMLInputElement>(null);
  const mounted = useRef(false);
  const read = async (id: string) => {
    const token = ++generation.current;
    const readPages = async <T extends { id: string },>(fetch: (id: string, before?: string | null) => Promise<T[]>, depth: number) => {
      const records: T[] = [];
      let cursor: string | null = null;
      let lastCount = 0;
      for (let page = 0; page < depth; page++) {
        const rows: T[] = await (cursor ? fetch(id, cursor) : fetch(id));
        if (!mounted.current || generation.current !== token) return { records: [], lastCount: 0 };
        records.push(...rows); lastCount = rows.length;
        cursor = rows[rows.length - 1]?.id ?? null;
        if (lastCount < 30) break;
      }
      return { records, lastCount };
    };
    const [item, history, transactions] = await Promise.all([gamePluginsApi.detail(id),
      readPages(gamePluginsApi.history, historyDepth.current.runs), readPages(gamePluginsApi.ledger, historyDepth.current.ledger)]);
    if (!mounted.current || generation.current !== token) return;
    setDetail(item);
    if(pendingCardAction.current?.id===id){setConfirmation(pendingCardAction.current.kind);setClearOnUninstall(false);pendingCardAction.current=null;}
    if (!choicesDirty.current) setChoices(item.permissions.map(({ permission, scope, decision }) => ({ permission, scope, decision })));
    setRuns(history.records); setLedger(transactions.records); setRunsMore(history.lastCount === 30); setLedgerMore(transactions.lastCount === 30);
  };
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false; generation.current++;
      if (previewToken.current) void gamePluginsApi.cancelInstall(previewToken.current).catch(() => {});
    };
  }, []);
  useEffect(() => {
    if (previousSelected.current !== selected) {
      generation.current++;
      historyDepth.current = { runs: 1, ledger: 1 }; setRunsPage(1);setLedgerPage(1);
      previousSelected.current = selected; choicesDirty.current = false;
      if (resetPluginId && resetPluginId !== selected) setResetPluginId("");
      setDetail(null); setError(""); setRuns([]); setLedger([]); setConfirmation(null);
    }
    const token = generation.current + 1;
    if (selected) void read(selected).catch(cause => { if (mounted.current && generation.current === token) setError(errorMessage(cause)); });
  }, [selected, plugins]);
  useEffect(() => { if (preview) installDialog.current?.showModal(); else installDialog.current?.close(); }, [preview]);
  useEffect(() => { if (confirmation) confirmDialog.current?.showModal(); else confirmDialog.current?.close(); }, [confirmation]);
  const run = async (action: () => Promise<void>, success = "") => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(""); setNotice("");
    try { await action(); if (mounted.current) setNotice(success); }
    catch (cause) { if (mounted.current) setError(errorMessage(cause)); }
    finally { lock.current = false; if (mounted.current) setBusy(false); }
  };
  useEffect(() => { if (launchDetail) launchDialog.current?.showModal(); else launchDialog.current?.close(); }, [launchDetail]);
  const open = (plugin: GamePlugin) => run(async () => {
    const value = await gamePluginsApi.detail(plugin.pluginId);
    if (!mounted.current) return;
    if (value.permissions.some(item => item.decision === "pending") || requiresWallet(plugin) && !value.permissions.some(item => item.permission === "wallet.modify" && item.decision === "allowed")) {
      setLaunchDetail(value);
      setLaunchChoices(value.permissions.map(({permission, scope, decision}) => ({permission, scope, decision})));
      return;
    }
    await gamePluginsApi.open(plugin.pluginId);
  });
  const authorizeAndOpen = () => run(async () => {
    if (!launchDetail) return;
    if (launchChoices.some(item => item.decision === "pending")) throw new Error("请先决定全部钱包与联网权限。");
    if (requiresWallet(launchDetail.plugin) && !launchChoices.some(item => item.permission === "wallet.modify" && item.decision === "allowed")) throw new Error("“我的家”需要允许钱包权限才能游玩。");
    for (const choice of launchChoices) await gamePluginsApi.permission(launchDetail.plugin.pluginId, launchDetail.plugin.installedVersion, choice);
    await gamePluginsApi.open(launchDetail.plugin.pluginId);
    if (mounted.current) { setLaunchDetail(null); await refresh(); }
  });
  const cancelInstall = () => run(async () => {
    if (preview) await gamePluginsApi.cancelInstall(preview.token);
    previewToken.current = null; setPreview(null);
  });
  const previewFile = (file: File) => run(async () => {
    if (file.size > 64 * 1024 * 1024) throw new Error("插件包不能超过 64 MiB。");
    const bytes = new Uint8Array(await file.arrayBuffer());
    const chunks: string[] = [];
    for (let index = 0; index < bytes.length; index += 32768) chunks.push(String.fromCharCode(...bytes.subarray(index, index + 32768)));
    const result = await gamePluginsApi.previewInstall(file.name, btoa(chunks.join("")));
    if (!mounted.current) { await gamePluginsApi.cancelInstall(result.token); return; }
    previewToken.current = result.token; setPreview(result);
    setInstallChoices(result.permissions.map(({permission, scope, decision}) => ({permission, scope, decision})));
  });
  const install = () => run(async () => {
    if (!preview) return;
    const plugin = await gamePluginsApi.confirmInstall(preview.token, installChoices);
    previewToken.current = null; setPreview(null); setSelected(plugin.pluginId); await refresh();
  }, "插件安装完成。已有存档按插件版本规则保留或迁移。");
  const actOnDetail = (action: (value: GamePluginDetail) => Promise<void>, success: string) => run(async () => {
    if (!detail) return;
    await action(detail); await refresh();
  }, success);
  const confirm = () => actOnDetail(async value => {
    if (confirmation === "clear") {
      await gamePluginsApi.clearData(value.plugin.pluginId, value.plugin.installedVersion);
      setConfirmation(null);setResetPluginId(value.plugin.pluginId);choicesDirty.current=false;
    }
    else { await gamePluginsApi.uninstall(value.plugin.pluginId, value.plugin.installedVersion, clearOnUninstall); setSelected(""); }
    setConfirmation(null);
  }, confirmation === "clear" ? "小游戏已恢复初始数据；钱包和联网权限需要重新确认。原有喵币、账单和运行历史保留。" : "插件程序已卸载。");
  const reopenAfterReset = () => actOnDetail(async value => {
    if (choices.some(item => item.decision === "pending")) throw new Error("请先决定全部钱包与联网权限。");
    if (value.plugin.pluginId === "com.yanmew.game.my-home" && !choices.some(item => item.permission === "wallet.modify" && item.decision === "allowed")) throw new Error("“我的家”需要允许钱包权限才能游玩。");
    for (const choice of choices) await gamePluginsApi.permission(value.plugin.pluginId,value.plugin.installedVersion,choice);
    choicesDirty.current=false;
    await gamePluginsApi.open(value.plugin.pluginId);
    setResetPluginId("");
  }, "小游戏已用全新进度打开。");
  const pageHistory = async (kind: "runs" | "ledger") => {
    let advanced=false;
    await run(async () => {
    if (!detail) return;
    const token = ++generation.current;
    if (kind === "runs") {
      const rows = await gamePluginsApi.history(detail.plugin.pluginId, runs[runs.length - 1]?.id ?? null);
      if (!mounted.current || token !== generation.current) return;
      historyDepth.current.runs++; advanced=rows.length>0;
      setRuns(previous => [...previous, ...rows.filter(row => !previous.some(old => old.id === row.id))]); setRunsMore(rows.length === 30);
    } else {
      const rows = await gamePluginsApi.ledger(detail.plugin.pluginId, ledger[ledger.length - 1]?.id ?? null);
      if (!mounted.current || token !== generation.current) return;
      historyDepth.current.ledger++; advanced=rows.length>0;
      setLedger(previous => [...previous, ...rows.filter(row => !previous.some(old => old.id === row.id))]); setLedgerMore(rows.length === 30);
    }
 });
    return advanced;
  };
  const sourceLink = (url: string | null, title: string) => url && <button className="ym-button" disabled={busy} onClick={() => void run(() => webSearchApi.open(url))}>{title}</button>;
  return <>
    <section className="ym-page-card"><div className="ym-page-heading"><h2>小游戏插件</h2><span className="ym-page-note">全局喵币 · {wallet?.coins ?? "读取中…"}</span><button className="ym-button" disabled={busy || !!preview} onClick={() => fileInput.current?.click()}>{busy ? "正在处理…" : "安装 / 更新本地小游戏插件"}</button><input ref={fileInput} hidden type="file" accept=".yanmew-game,.zip" disabled={busy || !!preview} aria-label="安装本地小游戏插件包" onChange={event => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void previewFile(file); }} /></div>
      <p className="ym-page-note">在这里打开和管理所有小游戏，也可安装从作者 GitHub 下载的小游戏插件包。游戏与存档不随人设切换，各游戏自己的次数限制由插件保存；钱包和联网权限由你确认，自带小游戏可以停用，不能卸载。</p>
      {loading && <p>正在读取插件…</p>}{listError && <p role="alert" className="ym-page-error">{listError}<button className="ym-button" onClick={() => void refresh()}>重试</button></p>}
      {error && <p role="alert" className="ym-page-error">{error}</p>}{notice && <p role="status">{notice}</p>}
      {!selected&&<><CardBrowserControls browser={browser} label="小游戏插件" timeLabel="安装时间" disabled={busy}/><div className="ym-object-grid">{browser.items.map(plugin=><article className="ym-object-card" key={plugin.pluginId} onClick={event=>{if(!busy&&!(event.target as HTMLElement).closest("button"))setSelected(plugin.pluginId);}}>
        <div className="ym-page-heading"><h3><button className="ym-card-open" disabled={busy} onClick={()=>setSelected(plugin.pluginId)}>{plugin.name}</button></h3><span className="ym-page-note">{plugin.isBundled?"自带":"已安装"}</span></div>
        <p className="ym-card-summary">{plugin.description}</p><p className="ym-page-note">{plugin.author} · v{plugin.installedVersion}</p><p className="ym-page-note">{plugin.packageStatus==="missing_package"?"程序包缺失 · 使用数据保留":plugin.isEnabled?"已启用":"已停用"}</p>
        <div className="ym-inline-form"><button className="ym-button" disabled={busy||!plugin.isEnabled||plugin.packageStatus!=="available"} onClick={()=>void open(plugin)} aria-label={`打开 ${plugin.name}`}>打开</button><button className="ym-button" disabled={busy||plugin.isBundled} title={plugin.isBundled?"随应用提供，不能卸载":"卸载插件程序"} onClick={()=>{pendingCardAction.current={id:plugin.pluginId,kind:"uninstall"};setSelected(plugin.pluginId);}}>卸载</button>
          <button className="ym-button" role="switch" aria-label={`启用 ${plugin.name}`} aria-checked={plugin.isEnabled} disabled={busy||plugin.packageStatus!=="available"} onClick={()=>void run(async()=>{await gamePluginsApi.enable(plugin.pluginId,plugin.installedVersion,!plugin.isEnabled);await refresh();},"插件状态已保存。")}>启用小游戏</button>
          <button className="ym-button" disabled={busy} onClick={()=>{pendingCardAction.current={id:plugin.pluginId,kind:"clear"};setSelected(plugin.pluginId);}}>清除数据并重载</button></div>
        {plugin.isBundled&&<p className="ym-page-note">自带小游戏不可卸载，可停用或恢复初始数据。</p>}
      </article>)}</div>{!loading&&!browser.total&&<p className="ym-page-empty">{plugins.length?"没有匹配的小游戏。":"暂无小游戏，可使用上方按钮安装本地小游戏插件包。"}</p>}</>}
      {selected&&<button className="ym-button" disabled={busy} onClick={()=>{pendingCardAction.current=null;setSelected("");}}>返回小游戏列表</button>}
    </section>
    {selected && !detail && <p className="ym-page-note">正在读取插件详情…</p>}
    {detail && <>
      <section className="ym-page-card"><div className="ym-page-heading"><h2>{detail.plugin.name}</h2><span>{detail.plugin.isBundled ? "随应用提供" : "本地安装"}</span></div>
        <p>{detail.plugin.description}</p><p className="ym-game-plugin-id">{detail.plugin.author} · v{detail.plugin.installedVersion}</p>
        <div className="ym-game-tools"><button className="ym-button" disabled={busy||!detail.plugin.isEnabled||detail.plugin.packageStatus!=="available"||resetPluginId===detail.plugin.pluginId} onClick={()=>void open(detail.plugin)} aria-label={`打开 ${detail.plugin.name}`}>打开</button>{sourceLink(detail.plugin.repository, "GitHub 源码")}{sourceLink(detail.plugin.releasePage, "下载页面")}{sourceLink(detail.plugin.homepage, "作者主页")}
          <button className="ym-button" role="switch" aria-label={`启用 ${detail.plugin.name}`} aria-checked={detail.plugin.isEnabled} disabled={busy || detail.plugin.packageStatus !== "available"} onClick={() => void actOnDetail(value => gamePluginsApi.enable(value.plugin.pluginId, value.plugin.installedVersion, !value.plugin.isEnabled), "插件状态已保存。")}>启用小游戏</button>
        </div>
        <MotionDetails className="ym-game-maintenance"><summary>维护与版本信息</summary><p className="ym-game-plugin-id">插件 ID：{detail.plugin.pluginId} · 存档版本 {detail.usage.storage_version}</p><div className="ym-game-tools">
          <button className="ym-button" data-variant="danger" disabled={busy} onClick={() => { setConfirmation("clear"); setError(""); }}>清除数据并重载</button>
          <button className="ym-button" data-variant="danger" disabled={busy || detail.plugin.isBundled} onClick={() => { setClearOnUninstall(false); setConfirmation("uninstall"); setError(""); }}>卸载程序</button>
        </div></MotionDetails>
        {detail.plugin.packageStatus === "missing_package" && <p className="ym-page-note">数据仍在，但程序包缺失。可从来源页找到相同插件 ID 的包并重新安装。</p>}
        {!detail.plugin.repository && !detail.plugin.releasePage && !detail.plugin.homepage && <p className="ym-page-note">作者未提供来源链接，可用插件 ID 查找程序包。</p>}
        <h3>权限</h3><GamePermissions choices={choices} disabled={busy} onChange={values => { choicesDirty.current = true; setChoices(values); }} />
        <button className="ym-button" disabled={busy || detail.plugin.packageStatus !== "available"} onClick={() => void actOnDetail(async value => { for (const choice of choices) await gamePluginsApi.permission(value.plugin.pluginId, value.plugin.installedVersion, choice); choicesDirty.current = false; }, "权限已保存。新增钱包授权需重新打开游戏后生效；撤销会立即阻止新扣款。")}>保存权限</button>
        {resetPluginId===detail.plugin.pluginId&&<><p className="ym-page-note">已恢复初始数据。选择钱包和联网权限后，重新打开这个小游戏。</p><button className="ym-button" disabled={busy||!detail.plugin.isEnabled||detail.plugin.packageStatus!=="available"||choices.some(item=>item.decision==="pending")||detail.plugin.pluginId==="com.yanmew.game.my-home"&&!choices.some(item=>item.permission==="wallet.modify"&&item.decision==="allowed")} onClick={()=>void reopenAfterReset()}>重新授权并打开</button></>}
        <p className="ym-page-note">拒绝钱包权限时可纯玩；“我的家”需要钱包权限才能玩。插件不能访问颜喵的聊天、记忆或提供商密钥。</p>
      </section>
      <section className="ym-page-card ym-game-save-section"><h2>游戏存档</h2>
        <GameSaveSummary data={detail.usage.data} pluginId={detail.plugin.pluginId}/>
        {!!detail.manifest?.settings_schema.length&&<MotionDetails className="ym-game-save-settings"><summary>游戏设置</summary>{detail.manifest.settings_schema.map(setting => <SettingField key={`${detail.plugin.pluginId}:${detail.plugin.installedVersion}:${setting.key}`} setting={setting} value={Object.prototype.hasOwnProperty.call(detail.usage.settings, setting.key) ? detail.usage.settings[setting.key] : setting.default} disabled={busy} onSave={value => actOnDetail(item => gamePluginsApi.setting(item.plugin.pluginId, item.plugin.installedVersion, setting.key, value), "插件设置已保存。重新打开游戏使用新设置。")} />)}</MotionDetails>}
      </section>
      <MotionDetails className="ym-page-card ym-feature-module"><summary>运行历史</summary><p className="ym-page-note">一条记录是一次打开到关闭，不代表游戏的一局。游戏战绩由插件自己保存。</p>
        <ol className="ym-game-history">{runs.slice((runsPage-1)*5,runsPage*5).map(item => <li key={item.id}>{({running:"进行中",completed:"已完成",cancelled:"已关闭",error:"异常结束"})[item.status]} · 喵币变化 {item.coinDelta}<small>{new Date(item.startedAt).toLocaleString("zh-CN")} · v{item.pluginVersion}{item.outcome && ` · ${item.outcome}`}</small></li>)}</ol>
        {!runs.length && <p className="ym-page-note">暂无运行记录。</p>}
        <nav className="ym-browser-pages" aria-label="运行历史分页"><button className="ym-button" disabled={busy||runsPage<=1} onClick={()=>setRunsPage(v=>v-1)}>上一页</button><span>第{runsPage}页</span><button className="ym-button" disabled={busy||(!runsMore&&runsPage*5>=runs.length)} onClick={()=>{if(runsPage*5>=runs.length)void pageHistory("runs").then(ok=>{if(ok)setRunsPage(v=>v+1);});else setRunsPage(v=>v+1);}}>下一页</button></nav>
      </MotionDetails>
      <MotionDetails className="ym-page-card ym-feature-module"><summary>喵币账单</summary><ol className="ym-game-history">{ledger.slice((ledgerPage-1)*5,ledgerPage*5).map(item => <li key={item.id}>{BigInt(item.delta) > 0n ? "+" : ""}{item.delta} · {item.reason}<small>{new Date(item.createdAt).toLocaleString("zh-CN")}</small></li>)}</ol>
        {!ledger.length && <p className="ym-page-note">暂无喵币账单。</p>}
        <nav className="ym-browser-pages" aria-label="游戏喵币账单分页"><button className="ym-button" disabled={busy||ledgerPage<=1} onClick={()=>setLedgerPage(v=>v-1)}>上一页</button><span>第{ledgerPage}页</span><button className="ym-button" disabled={busy||(!ledgerMore&&ledgerPage*5>=ledger.length)} onClick={()=>{if(ledgerPage*5>=ledger.length)void pageHistory("ledger").then(ok=>{if(ok)setLedgerPage(v=>v+1);});else setLedgerPage(v=>v+1);}}>下一页</button></nav>
      </MotionDetails>
    </>}
    <dialog ref={launchDialog} className="ym-session-dialog" aria-labelledby="game-permission-title" onCancel={event => { if (busy) event.preventDefault(); else setLaunchDetail(null); }}>
      {launchDetail && <><h2 id="game-permission-title">{launchDetail.plugin.name} · 权限确认</h2><div className="ym-session-dialog-body">
        <p>允许修改喵币后，游戏可以增减整个应用的钱包余额。联网授权只对列出的来源生效。</p>
        <p>{requiresWallet(launchDetail.plugin) ? "我的家必须允许钱包权限才能游玩，不提供纯玩模式。" : "拒绝钱包权限可纯玩，不会增加或扣除喵币。"}</p>
        <GamePermissions choices={launchChoices} disabled={busy} onChange={setLaunchChoices}/>
        {error && <p role="alert" className="ym-page-error">{error}</p>}
      </div><div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={()=>setLaunchDetail(null)}>取消</button>
        <button className="ym-button" disabled={busy||launchChoices.some(item=>item.decision==="pending")||requiresWallet(launchDetail.plugin)&&!launchChoices.some(item=>item.permission==="wallet.modify"&&item.decision==="allowed")} onClick={()=>void authorizeAndOpen()}>保存权限并打开</button>
      </div></>}
    </dialog>
    <dialog ref={installDialog} className="ym-session-dialog" aria-labelledby="game-install-title" onCancel={event => { event.preventDefault(); if (!busy) void cancelInstall(); }}>
      {preview && <><h2 id="game-install-title">{preview.installed ? "更新" : "安装"} {preview.manifest.name}</h2><div className="ym-session-dialog-body"><p>{preview.manifest.description}</p>
        <p>{preview.manifest.author} · v{preview.manifest.version}</p><p className="ym-game-plugin-id">{preview.manifest.id} · 来源：{preview.manifest.repository ?? "作者未提供"}</p>
        {preview.migrationRequired && <p>这次升级会运行作者提供的存档迁移。迁移失败会保留旧插件和数据；请先关闭该插件窗口。</p>}
        <GamePermissions choices={installChoices} disabled={busy} onChange={setInstallChoices} />{error && <p role="alert" className="ym-page-error">{error}</p>}
      </div><div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => void cancelInstall()}>取消</button><button className="ym-button" disabled={busy || installChoices.some(item => item.decision === "pending")} onClick={() => void install()}>确认{preview.installed ? "更新" : "安装"}</button></div></>}
    </dialog>
    <dialog ref={confirmDialog} className="ym-session-dialog" aria-labelledby="game-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setConfirmation(null); }}>
      {confirmation && detail && <><h2 id="game-delete-title">{confirmation === "clear" ? "清除数据并重载" : "卸载插件程序"} · {detail.plugin.name}</h2><div className="ym-session-dialog-body">
        <p>{confirmation === "clear" ? "将关闭这个小游戏，清空它的进度和自有设置，钱包与联网权限重置为待确认。安装包、全局喵币、已有账单和运行历史保留。清除后重新选择权限，再以初始进度打开。" : "程序卸载后不能打开游戏。默认保留设置、存档、来源和历史账单，方便以后重装。"}</p>
        {confirmation === "uninstall" && <label className="ym-game-check"><input className="ym-selection-checkbox" type="checkbox" checked={clearOnUninstall} disabled={busy} onChange={event => setClearOnUninstall(event.target.checked)} />同时清除插件设置和私有存档（钱包和账单保留）</label>}
        {error && <p role="alert" className="ym-page-error">{error}</p>}
      </div><div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setConfirmation(null)}>取消</button><button className="ym-button" disabled={busy} onClick={() => void confirm()}>确认{confirmation === "clear" ? "清除" : "卸载"}</button></div></>}
    </dialog>
  </>;
}
