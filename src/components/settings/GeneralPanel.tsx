import { useEffect, useState } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { Settings } from "../../types";

export default function GeneralPanel() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let disposed = false;
    setError("");
    void configurationApi.getSettings().then(value => { if (!disposed) setSettings(value); })
      .catch(cause => { if (!disposed) setError(errorMessage(cause)); });
    return () => { disposed = true; };
  }, [attempt]);
  const save = async () => {
    if (!settings || saving) return;
    setSaving(true); setError(""); setSaved(false);
    try { setSettings(await configurationApi.saveSettings({ theme_mode: settings.theme_mode })); setSaved(true); }
    catch (cause) { setError(errorMessage(cause)); }
    finally { setSaving(false); }
  };
  return <section className="ym-page-card">
    <h2>外观主题</h2>
    <p className="ym-page-note">为颜喵选择明亮或安静的颜色。小游戏保留各自的主题。</p>
    {settings ? <form onSubmit={event => { event.preventDefault(); void save(); }}>
      <label className="ym-field">主题<select disabled={saving} value={settings.theme_mode} onChange={event => { setSaved(false); setSettings({ ...settings, theme_mode: event.target.value as Settings["theme_mode"] }); }}>
        <option value="system">跟随系统</option><option value="light">亮色</option><option value="dark">暗色</option>
      </select></label>
      <div className="ym-inline-form"><button className="ym-button" disabled={saving} type="submit">{saving ? "正在保存…" : "保存主题"}</button>{saved && <span role="status">主题已保存</span>}</div>
    </form> : !error && <p role="status">正在读取设置…</p>}
    {error && <div role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={() => setAttempt(value => value + 1)}>重新读取</button></div>}
    <p className="ym-page-note">其他通用设置与导入导出将在对应功能完成后开放。</p>
  </section>;
}
