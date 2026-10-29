import { useEffect, useRef, useState } from "react";
import { backupApi, errorMessage } from "../../api";
import type { BackupImportNode, BackupImportPreview, BackupImported, BackupKind } from "../../types";
import "./BackupPanel.css";

const labels: Record<BackupKind, string> = { full: "全量备份", session: "会话包", user_profile: "用户设定包", persona_profile: "人设资料包", persona_full: "人设及会话包", plugin_data: "小游戏插件使用数据包" };
function defaults(nodes: BackupImportNode[]): Record<string, string> {
  return Object.fromEntries(nodes.map(node => [node.sourceId, node.requiresChoice ? "choose" : node.suggestedTargetId ?? "new"]));
}
export default function BackupImportPanel() {
  const [preview, setPreview] = useState<BackupImportPreview | null>(null);
  const [personas, setPersonas] = useState<Record<string, string>>({});
  const [profiles, setProfiles] = useState<Record<string, string>>({});
  const [consent, setConsent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [result, setResult] = useState<BackupImported | null>(null);
  const alive = useRef(true);
  const token = useRef<string | null>(null);
  const committing = useRef(false);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; if (token.current && !committing.current) void backupApi.cancelImport(token.current).catch(() => {}); };
  }, []);
  const show = (value: BackupImportPreview) => {
    token.current = value.previewId; setPreview(value); setPersonas(defaults(value.personas)); setProfiles(defaults(value.profiles)); setConsent(false);
  };
  const read = async (refresh = false) => {
    if (busy) return;
    setBusy(true); setError(""); setMessage(""); setResult(null);
    try {
      const value = refresh && token.current ? await backupApi.refreshImport(token.current) : await backupApi.prepareImport();
      if (!alive.current) { if (value) void backupApi.cancelImport(value.previewId).catch(() => {}); return; }
      if (value) show(value); else setMessage("已取消选择，当前数据未修改。");
    } catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { if (alive.current) setBusy(false); }
  };
  const cancel = async () => {
    if (!preview || busy) return;
    setBusy(true); setError("");
    try { await backupApi.cancelImport(preview.previewId); token.current = null; if (alive.current) { setPreview(null); setMessage("已取消导入，当前数据未修改。"); } }
    catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { if (alive.current) setBusy(false); }
  };
  const missing = preview && ([...preview.personas.map(n => personas[n.sourceId]), ...preview.profiles.map(n => profiles[n.sourceId])].some(value => !value || value === "choose") || (preview.kind === "full" && !consent));
  const confirm = async () => {
    if (!preview || busy || missing) return;
    setBusy(true); committing.current = true; setError(""); setMessage("");
    try {
      const imported = await backupApi.confirmImport({ previewId: preview.previewId, personas: preview.personas.map(n => ({ sourceId: n.sourceId, targetId: personas[n.sourceId] === "new" ? null : personas[n.sourceId] })), profiles: preview.profiles.map(n => ({ sourceId: n.sourceId, targetId: profiles[n.sourceId] === "new" ? null : profiles[n.sourceId] })), confirmGlobalRestore: consent });
      token.current = null;
      if (alive.current) { setPreview(null); setResult(imported); setMessage(`导入完成，已新增 ${imported.sessions} 个会话。`); }
    } catch (cause) {
      if (alive.current) setError(errorMessage(cause));
      else if (token.current) void backupApi.cancelImport(token.current).catch(() => {});
    } finally { committing.current = false; if (alive.current) setBusy(false); }
  };
  const choices = (nodes: BackupImportNode[], selected: Record<string, string>, change: (values: Record<string, string>) => void, profile: boolean) => nodes.map(node => <div className="ym-backup-binding" key={node.sourceId}>
    <label className="ym-field">{node.name}<select aria-label={`${profile ? "用户设定" : "人设"}关联：${node.name}`} value={selected[node.sourceId] ?? "choose"} disabled={busy || node.isBuiltin} onChange={event => change({ ...selected, [node.sourceId]: event.target.value })}>
      {node.requiresChoice && <option value="choose" disabled>请选择关联目标或新建</option>}
      {!node.isBuiltin && <option value="new">新建{profile ? "用户设定" : "人设"}</option>}
      {node.candidates.map(candidate => <option value={candidate.id} key={candidate.id}>{profile ? "恢复到" : "关联本机"}：{candidate.name}（{candidate.id}）</option>)}
    </select></label>
    {node.isBuiltin ? <p className="ym-page-note">关联系统默认人设，保留本机设定。</p> : selected[node.sourceId] === "new" ? <p className="ym-page-note">导入备份中的设定；同名时自动添加后缀。</p> : selected[node.sourceId] === "choose" ? <p className="ym-page-note">名称相同不代表同一个人设，请明确选择。</p> : profile ? <p className="ym-page-note">更新这份用户设定，保留原有绑定会话，并追加备份会话。</p> : <p className="ym-page-note">保留本机人设的设定和外观，只追加备份会话。{node.candidates.find(c => c.id === selected[node.sourceId])?.different && "两边设定有差异，备份设定不会覆盖本机。"}</p>}
  </div>);
  return <section className="ym-page-card ym-backup-panel">
    <h2>导入备份</h2>
    <p className="ym-page-note">通过系统窗口选择颜喵备份。先校验和预览，确认后才保存数据。已有会话保留，重复导入也会新增副本。</p>
    <button className="ym-button" disabled={busy} onClick={() => void read()}>{busy ? "正在处理…" : "选择备份并预览"}</button>
    {preview && <div className="ym-backup-import-preview">
      <h3>{labels[preview.kind]}</h3>
      <p>备份时间：{new Date(preview.exportedAt).toLocaleString()}</p>
      <p>包含 {preview.counts.sessions ?? 0} 个会话、{preview.counts.messages ?? 0} 条消息、{preview.counts.long_memories ?? 0} 条长期记忆。</p>
      {preview.personas.length > 0 && <fieldset><legend>人设关联</legend>{choices(preview.personas, personas, setPersonas, false)}</fieldset>}
      {preview.profiles.length > 0 && <fieldset><legend>用户设定</legend>{choices(preview.profiles, profiles, setProfiles, true)}</fieldset>}
      {preview.plugins.length > 0 && <><h3>插件使用数据</h3><ul>{preview.plugins.map(id => <li key={id}>{id}</li>)}</ul><p className="ym-page-note">插件程序不在备份中。可兼容本机程序时恢复存档；需要升级时调用开发者迁移程序，否则保留数据并提示重新安装。</p></>}
      {preview.warnings.length > 0 && <ul>{[...new Set(preview.warnings)].map(warning => <li key={warning}>{warning}</li>)}</ul>}
      {preview.kind === "full" && <label className="ym-backup-option"><input className="ym-selection-checkbox" type="checkbox" disabled={busy} checked={consent} onChange={event => setConsent(event.target.checked)} /><span>确认用备份恢复全局喵币与喵币账单、现实记账（账单、资产、预算、周期）、设置、提醒、提供商、聊天表情包库和插件使用数据。现有会话保留；密钥重新填写，搜索和生图默认关闭。</span></label>}
      <div className="ym-backup-actions"><button className="ym-button" disabled={busy || !!missing} onClick={() => void confirm()}>确认导入</button><button className="ym-button" disabled={busy} onClick={() => void read(true)}>刷新预览</button><button className="ym-button" disabled={busy} onClick={() => void cancel()}>取消导入</button></div>
    </div>}
    {error && <p className="ym-page-error" role="alert">{error}</p>}
    {message && <p role="status">{message}</p>}
    {result && <><p>新建 {result.personasCreated} 个人设，关联 {result.personasReused} 个已有的人设。</p>{result.warnings.length > 0 && <ul>{[...new Set(result.warnings)].map(warning => <li key={warning}>{warning}</li>)}</ul>}</>}
  </section>;
}
