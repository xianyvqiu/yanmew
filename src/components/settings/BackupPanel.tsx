import { useEffect, useRef, useState } from "react";
import { backupApi, errorMessage } from "../../api";
import "./BackupPanel.css";
import BackupImportPanel from "./BackupImportPanel";
import type { BackupArchive, BackupExportOption, BackupExportOptions, BackupKind } from "../../types";

const labels: Record<BackupKind, string> = {
  full: "全量备份", session: "指定会话", user_profile: "用户设定", persona_profile: "人设资料（不含会话）",
  persona_full: "人设及全部会话", plugin_data: "小游戏插件使用数据",
};
export default function BackupPanel() {
  const [kind, setKind] = useState<BackupKind>("full");
  const [options, setOptions] = useState<BackupExportOptions | null>(null);
  const [ids, setIds] = useState<string[]>([]);
  const [includeSessions, setIncludeSessions] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [result, setResult] = useState<BackupArchive | null>(null);
  const [attempt, setAttempt] = useState(0);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => {
    if (kind === "full") return;
    let disposed = false;
    setLoading(true); setError(""); setOptions(null);
    void backupApi.options().then(value => { if (!disposed) setOptions(value); })
      .catch(cause => { if (!disposed) setError(errorMessage(cause)); })
      .finally(() => { if (!disposed) setLoading(false); });
    return () => { disposed = true; };
  }, [kind, attempt]);
  const items: BackupExportOption[] = !options ? [] : kind === "session" ? options.sessions
    : kind === "user_profile" ? options.userProfiles : kind === "plugin_data" ? options.plugins : options.personas;
  const changeKind = (value: BackupKind) => {
    setKind(value); setIds([]); setIncludeSessions(false); setResult(null); setMessage(""); setError(""); setLoading(false);
  };
  const save = async () => {
    if (busy || loading || (kind !== "full" && ids.length === 0)) return;
    setBusy(true); setError(""); setMessage(""); setResult(null);
    try {
      const saved = await backupApi.export({ kind, ids, includeSessions: kind === "user_profile" && includeSessions });
      if (!alive.current) return;
      if (saved) { setResult(saved); setMessage(`备份已保存：${saved.filename}`); }
      else setMessage("已取消保存，原数据和原备份保持不变。");
    } catch (cause) { if (alive.current) setError(errorMessage(cause)); }
    finally { if (alive.current) setBusy(false); }
  };
  return <><section className="ym-page-card ym-backup-panel">
    <h2>备份与导出</h2>
    <p className="ym-page-note">选择要保存的数据，通过系统窗口选择保存位置。聊天记录完整备份，不限制条数。</p>
    <form onSubmit={event => { event.preventDefault(); void save(); }}>
      <label className="ym-field">备份范围<select value={kind} disabled={busy} onChange={event => changeKind(event.target.value as BackupKind)}>
        {(Object.keys(labels) as BackupKind[]).map(value => <option key={value} value={value}>{labels[value]}</option>)}
      </select></label>
      {kind !== "full" && <fieldset disabled={busy || loading} className="ym-field">
        <legend>选择要备份的对象</legend>
        {loading ? <p role="status">正在读取可选对象…</p> : !options ? <button className="ym-button" type="button" onClick={() => setAttempt(value => value + 1)}>重新读取</button>
          : items.length === 0 ? <p>暂无可备份对象。</p> : <div className="ym-backup-options">
            {items.map(item => <label className="ym-backup-option" key={item.id}><input className="ym-selection-checkbox" type="checkbox" checked={ids.includes(item.id)} onChange={event => setIds(previous => event.target.checked ? [...previous, item.id] : previous.filter(id => id !== item.id))} /><span>{item.name}</span></label>)}
          </div>}
      </fieldset>}
      {kind === "user_profile" && <label className="ym-backup-option"><input className="ym-selection-checkbox" type="checkbox" checked={includeSessions} disabled={busy} onChange={event => setIncludeSessions(event.target.checked)} /><span>同时备份绑定这些用户设定的会话</span></label>}
      {kind === "full" && <p className="ym-page-note">包含全部业务数据。恢复时全局喵币、账单、设置等按备份恢复；人设关联和会话追加，不覆盖原会话。</p>}
      {kind === "plugin_data" && <p className="ym-page-note">仅保存插件登记、来源链接、权限申请、设置和存档。不含游戏程序、喵币、账单或运行记录；恢复后权限需要重新确认。</p>}
      <p className="ym-page-note">不包含颜喵管理的 API Key、模型文件、程序包和可重建索引。插件私有存档可能包含插件自行保存的凭据，请妥善保管备份。</p>
      <button className="ym-button" type="submit" disabled={busy || loading || (kind !== "full" && ids.length === 0)}>{busy ? "正在保存备份…" : "选择位置并导出"}</button>
    </form>
    {error && <p role="alert" className="ym-page-error">{error}</p>}
    {message && <p role="status">{message}</p>}
    {result && <div>
      <p>备份文件约 {(result.size / 1024 / 1024).toFixed(2)} MB；包含 {result.manifest.record_counts.sessions ?? 0} 个会话、{result.manifest.record_counts.messages ?? 0} 条消息、{result.manifest.files.length} 个图片附件。</p>
      {result.manifest.warnings.length > 0 && <><p>备份已保存，请留意：</p><ul>{[...new Set(result.manifest.warnings.map(warning => warning.message))].map(warning => <li key={warning}>{warning}</li>)}</ul></>}
    </div>}
  </section><BackupImportPanel /></>;
}
