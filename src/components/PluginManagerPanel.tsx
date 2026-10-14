import { useEffect, useRef, useState } from "react";
import { errorMessage, gamePluginsApi, webSearchApi } from "../api";
import { useGamePlugins } from "../hooks/useGamePlugins";
import type { GameInstallPreview, GamePermissionChoice, GamePluginDetail, GamePluginLedger, GameRun, PluginJson, PluginSetting } from "../types";
import GamePermissions from "./GamePermissions";
import "./GamePlugins.css";

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
    {setting.type === "boolean" ? <select value={String(draft)} disabled={disabled} onChange={event => setDraft(event.target.value === "true")}><option value="true">开启</option><option value="false">关闭</option></select>
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
  const { plugins, error: listError, loading, refresh } = useGamePlugins();
  const [selected, setSelected] = useState("");
  const [detail, setDetail] = useState<GamePluginDetail | null>(null);
  const [choices, setChoices] = useState<GamePermissionChoice[]>([]);
  const [preview, setPreview] = useState<GameInstallPreview | null>(null);
  const [installChoices, setInstallChoices] = useState<GamePermissionChoice[]>([]);
  const [confirmation, setConfirmation] = useState<Confirmation>(null);
  const [clearOnUninstall, setClearOnUninstall] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [runs, setRuns] = useState<GameRun[]>([]);
  const [ledger, setLedger] = useState<GamePluginLedger[]>([]);
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
      historyDepth.current = { runs: 1, ledger: 1 };
      previousSelected.current = selected; choicesDirty.current = false;
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
    if (confirmation === "clear") await gamePluginsApi.clearData(value.plugin.pluginId, value.plugin.installedVersion);
    else { await gamePluginsApi.uninstall(value.plugin.pluginId, value.plugin.installedVersion, clearOnUninstall); setSelected(""); }
    setConfirmation(null);
  }, confirmation === "clear" ? "私有存档已清除，钱包及账单保留。" : "插件程序已卸载。");
  const pageHistory = (kind: "runs" | "ledger") => run(async () => {
    if (!detail) return;
    const token = ++generation.current;
    if (kind === "runs") {
      const rows = await gamePluginsApi.history(detail.plugin.pluginId, runs[runs.length - 1]?.id ?? null);
      if (!mounted.current || token !== generation.current) return;
      historyDepth.current.runs++;
      setRuns(previous => [...previous, ...rows.filter(row => !previous.some(old => old.id === row.id))]); setRunsMore(rows.length === 30);
    } else {
      const rows = await gamePluginsApi.ledger(detail.plugin.pluginId, ledger[ledger.length - 1]?.id ?? null);
      if (!mounted.current || token !== generation.current) return;
      historyDepth.current.ledger++;
      setLedger(previous => [...previous, ...rows.filter(row => !previous.some(old => old.id === row.id))]); setLedgerMore(rows.length === 30);
    }
  });
  const sourceLink = (url: string | null, title: string) => url && <button className="ym-button" disabled={busy} onClick={() => void run(() => webSearchApi.open(url))}>{title}</button>;
  return <>
    <section className="ym-page-card"><div className="ym-page-heading"><h2>插件管理</h2><button className="ym-button" disabled={busy || !!preview} onClick={() => fileInput.current?.click()}>{busy ? "正在处理…" : "安装 / 更新本地插件"}</button><input ref={fileInput} hidden type="file" accept=".yanmew-game,.zip" disabled={busy || !!preview} aria-label="安装本地插件包" onChange={event => { const file = event.target.files?.[0]; event.target.value = ""; if (file) void previewFile(file); }} /></div>
      <p className="ym-page-note">从作者 GitHub 下载插件包后在这里安装。钱包和联网权限由你确认；升级时作者负责迁移存档。自带插件可停用，不能永久卸载。</p>
      {loading && <p>正在读取插件…</p>}{listError && <p role="alert" className="ym-page-error">{listError}<button className="ym-button" onClick={() => void refresh()}>重试</button></p>}
      {error && <p role="alert" className="ym-page-error">{error}</p>}{notice && <p role="status">{notice}</p>}
      <label className="ym-field">选择插件<select value={selected} disabled={busy} onChange={event => setSelected(event.target.value)}><option value="">请选择</option>{plugins.map(plugin => <option key={plugin.pluginId} value={plugin.pluginId}>{plugin.name}{plugin.packageStatus === "missing_package" ? " · 程序包缺失" : !plugin.isEnabled ? " · 已停用" : ""}</option>)}</select></label>
    </section>
    {selected && !detail && <p className="ym-page-note">正在读取插件详情…</p>}
    {detail && <>
      <section className="ym-page-card"><div className="ym-page-heading"><h2>{detail.plugin.name}</h2><span>{detail.plugin.isBundled ? "随应用提供" : "本地安装"}</span></div>
        <p>{detail.plugin.description}</p><p className="ym-game-plugin-id">{detail.plugin.pluginId} · {detail.plugin.author} · v{detail.plugin.installedVersion} · 存档版本 {detail.usage.storage_version}</p>
        <div className="ym-game-tools">{sourceLink(detail.plugin.repository, "GitHub 源码")}{sourceLink(detail.plugin.releasePage, "下载页面")}{sourceLink(detail.plugin.homepage, "作者主页")}
          <button className="ym-button" disabled={busy || detail.plugin.packageStatus !== "available"} onClick={() => void actOnDetail(value => gamePluginsApi.enable(value.plugin.pluginId, value.plugin.installedVersion, !value.plugin.isEnabled), "插件状态已保存。")}>{detail.plugin.isEnabled ? "停用" : "启用"}</button>
          <button className="ym-button" disabled={busy} onClick={() => { setConfirmation("clear"); setError(""); }}>清除私有存档</button>
          <button className="ym-button" disabled={busy || detail.plugin.isBundled} onClick={() => { setClearOnUninstall(false); setConfirmation("uninstall"); setError(""); }}>卸载程序</button>
        </div>
        {detail.plugin.packageStatus === "missing_package" && <p className="ym-page-note">数据仍在，但程序包缺失。可从来源页找到相同插件 ID 的包并重新安装。</p>}
        {!detail.plugin.repository && !detail.plugin.releasePage && !detail.plugin.homepage && <p className="ym-page-note">作者未提供来源链接，可用插件 ID 查找程序包。</p>}
        <h3>权限</h3><GamePermissions choices={choices} disabled={busy} onChange={values => { choicesDirty.current = true; setChoices(values); }} />
        <button className="ym-button" disabled={busy || detail.plugin.packageStatus !== "available"} onClick={() => void actOnDetail(async value => { for (const choice of choices) await gamePluginsApi.permission(value.plugin.pluginId, value.plugin.installedVersion, choice); choicesDirty.current = false; }, "权限已保存。新增钱包授权需重新打开游戏后生效；撤销会立即阻止新扣款。")}>保存权限</button>
        <p className="ym-page-note">拒绝钱包权限时可纯玩；“我的家”需要钱包权限才能玩。插件不能访问颜喵的聊天、记忆或提供商密钥。</p>
      </section>
      <section className="ym-page-card"><h2>插件设置与存档</h2>
        {detail.manifest?.settings_schema.length ? detail.manifest.settings_schema.map(setting => <SettingField key={`${detail.plugin.pluginId}:${detail.plugin.installedVersion}:${setting.key}`} setting={setting} value={Object.prototype.hasOwnProperty.call(detail.usage.settings, setting.key) ? detail.usage.settings[setting.key] : setting.default} disabled={busy} onSave={value => actOnDetail(item => gamePluginsApi.setting(item.plugin.pluginId, item.plugin.installedVersion, setting.key, value), "插件设置已保存。重新打开游戏使用新设置。")} />) : <p className="ym-page-note">此插件没有额外设置表单。</p>}
        <p className="ym-page-note">已保存 {Object.keys(detail.usage.data).length} 个存档键；这些是插件自己的使用数据。</p>
        <details><summary>查看存档内容</summary><pre className="ym-game-data">{JSON.stringify(detail.usage.data, null, 2)}</pre></details>
      </section>
      <section className="ym-page-card"><h2>运行历史</h2><p className="ym-page-note">一条记录是一次打开到关闭，不代表游戏的一局。游戏战绩由插件自己保存。</p>
        <ol className="ym-game-history">{runs.map(item => <li key={item.id}>{({running:"进行中",completed:"已完成",cancelled:"已关闭",error:"异常结束"})[item.status]} · 喵币变化 {item.coinDelta}<small>{item.startedAt} · 运行 ID {item.id} · v{item.pluginVersion}{item.outcome && ` · ${item.outcome}`}</small></li>)}</ol>
        {!runs.length && <p className="ym-page-note">暂无运行记录。</p>}{runsMore && <button className="ym-button" disabled={busy} onClick={() => void pageHistory("runs")}>加载更早的运行</button>}
      </section>
      <section className="ym-page-card"><h2>喵币账单</h2><ol className="ym-game-history">{ledger.map(item => <li key={item.id}>{BigInt(item.delta) > 0n ? "+" : ""}{item.delta} · {item.reason}<small>{item.createdAt} · 运行 ID {item.gameRunId} · 请求 ID {item.requestId}</small></li>)}</ol>
        {!ledger.length && <p className="ym-page-note">暂无喵币账单。</p>}{ledgerMore && <button className="ym-button" disabled={busy} onClick={() => void pageHistory("ledger")}>加载更早的账单</button>}
      </section>
    </>}
    <dialog ref={installDialog} className="ym-session-dialog" aria-labelledby="game-install-title" onCancel={event => { event.preventDefault(); if (!busy) void cancelInstall(); }}>
      {preview && <><h2 id="game-install-title">{preview.installed ? "更新" : "安装"} {preview.manifest.name}</h2><div className="ym-session-dialog-body"><p>{preview.manifest.description}</p>
        <p>{preview.manifest.author} · v{preview.manifest.version}</p><p className="ym-game-plugin-id">{preview.manifest.id} · 来源：{preview.manifest.repository ?? "作者未提供"}</p>
        {preview.migrationRequired && <p>这次升级会运行作者提供的存档迁移。迁移失败会保留旧插件和数据；请先关闭该插件窗口。</p>}
        <GamePermissions choices={installChoices} disabled={busy} onChange={setInstallChoices} />{error && <p role="alert" className="ym-page-error">{error}</p>}
      </div><div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => void cancelInstall()}>取消</button><button className="ym-button" disabled={busy || installChoices.some(item => item.decision === "pending")} onClick={() => void install()}>确认{preview.installed ? "更新" : "安装"}</button></div></>}
    </dialog>
    <dialog ref={confirmDialog} className="ym-session-dialog" aria-labelledby="game-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setConfirmation(null); }}>
      {confirmation && detail && <><h2 id="game-delete-title">{confirmation === "clear" ? "清除私有存档" : "卸载插件程序"} · {detail.plugin.name}</h2><div className="ym-session-dialog-body">
        <p>{confirmation === "clear" ? "将删除该插件的私有进度，无法恢复。插件设置、授权、全局钱包和历史账单保留。请先关闭游戏窗口。" : "程序卸载后不能打开游戏。默认保留设置、存档、来源和历史账单，方便以后重装。"}</p>
        {confirmation === "uninstall" && <label className="ym-game-check"><input type="checkbox" checked={clearOnUninstall} disabled={busy} onChange={event => setClearOnUninstall(event.target.checked)} />同时清除插件设置和私有存档（钱包和账单保留）</label>}
        {error && <p role="alert" className="ym-page-error">{error}</p>}
      </div><div className="ym-session-dialog-actions"><button className="ym-button" disabled={busy} onClick={() => setConfirmation(null)}>取消</button><button className="ym-button" disabled={busy} onClick={() => void confirm()}>确认{confirmation === "clear" ? "清除" : "卸载"}</button></div></>}
    </dialog>
  </>;
}
