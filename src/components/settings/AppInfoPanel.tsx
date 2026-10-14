import { useEffect, useRef, useState } from "react";
import { appApi, errorMessage } from "../../api";

export default function AppInfoPanel() {
  const [info, setInfo] = useState<{ name: string; version: string } | null>(null);
  const [error, setError] = useState("");
  const [exitError, setExitError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const [exiting, setExiting] = useState(false);
  const locked = useRef(false);
  const alive = useRef(false);
  useEffect(() => {
    alive.current = true; setError("");
    void appApi.info().then(value => { if (alive.current) setInfo(value); })
      .catch(cause => { if (alive.current) setError(errorMessage(cause)); });
    return () => { alive.current = false; };
  }, [attempt]);
  const exit = async () => {
    if (locked.current) return;
    locked.current = true; setExiting(true); setExitError("");
    try { await appApi.exit(); }
    catch (cause) { locked.current = false; if (alive.current) { setExitError(errorMessage(cause)); setExiting(false); } }
  };
  return <section className="ym-page-card" aria-label="关于颜喵">
    <div className="ym-page-heading"><div><h2>关于颜喵</h2>
      {info ? <p className="ym-page-note">{info.name} · 版本 {info.version}</p> : !error && <p role="status">正在读取版本…</p>}</div>
      <button className="ym-button" disabled={exiting} onClick={() => { void exit(); }}>{exiting ? "正在退出…" : "退出应用"}</button></div>
    <p className="ym-page-note">退出会关闭全部颜喵窗口并停止后台任务；当前聊天和桌宠计时按现有规则保存。只关闭设置窗口不会退出应用。</p>
    {error && <div role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={() => setAttempt(value => value + 1)}>重新读取版本</button></div>}
    {exitError && <p role="alert" className="ym-page-error">{exitError}</p>}
  </section>;
}
