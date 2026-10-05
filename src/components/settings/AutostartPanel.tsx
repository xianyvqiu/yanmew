import { useCallback, useEffect, useId, useRef, useState } from "react";
import { autostartApi, errorMessage } from "../../api";
import type { AutostartStatus } from "../../types";

function hasRegistration(status: AutostartStatus) {
  return status.enabled || status.needsRepair || status.blockedBySystem;
}

export default function AutostartPanel() {
  const [status, setStatus] = useState<AutostartStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const mounted = useRef(false);
  const reading = useRef(false);
  const locked = useRef(false);
  const version = useRef(0);
  const statusId = useId();

  const readStatus = useCallback(async () => {
    if (!mounted.current || locked.current) return;
    const token = ++version.current;
    reading.current = true;
    setLoading(true);
    setError("");
    setNotice("");
    try {
      const value = await autostartApi.get();
      if (mounted.current && token === version.current) setStatus(value);
    } catch (cause) {
      if (mounted.current && token === version.current) {
        setStatus(null);
        setError(`读取开机自启状态失败：${errorMessage(cause)}`);
      }
    } finally {
      if (mounted.current && token === version.current) {
        reading.current = false;
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void readStatus();
    const refresh = () => { void readStatus(); };
    window.addEventListener("focus", refresh);
    return () => {
      mounted.current = false;
      version.current++;
      window.removeEventListener("focus", refresh);
    };
  }, [readStatus]);

  const setEnabled = async (enabled: boolean) => {
    if (!mounted.current || locked.current || reading.current || !status?.supported) return;
    locked.current = true;
    const token = ++version.current;
    setSaving(true);
    setError("");
    setNotice("");
    try {
      const value = await autostartApi.set(enabled);
      if (!mounted.current || token !== version.current) return;
      setStatus(value);
      if (value.supported && enabled && value.enabled && !value.needsRepair && !value.blockedBySystem) {
        setNotice("开机自启已开启。");
      } else if (value.supported && !enabled && !hasRegistration(value)) {
        setNotice("开机自启已关闭。");
      } else if (value.supported && !value.needsRepair && !value.blockedBySystem) {
        setError("开机自启状态未达到预期，请重新读取后再试。");
      }
    } catch (cause) {
      if (!mounted.current || token !== version.current) return;
      const writeError = `设置开机自启失败：${errorMessage(cause)}`;
      setError(writeError);
      setStatus(null);
      setLoading(true);
      // 写入失败也可能已产生部分变化，必须重新读取系统的真实状态。
      try {
        const value = await autostartApi.get();
        if (mounted.current && token === version.current) setStatus(value);
      } catch (readCause) {
        if (mounted.current && token === version.current) {
          setError(`${writeError}；重新读取状态失败：${errorMessage(readCause)}`);
        }
      }
    } finally {
      if (mounted.current && token === version.current) {
        locked.current = false;
        setSaving(false);
        setLoading(false);
      }
    }
  };

  if (status && !status.supported) return null;
  const registered = status ? hasRegistration(status) : false;
  const busy = loading || saving;
  const statusLabel = !status ? "未知" : status.needsRepair ? "路径待修复" :
    status.blockedBySystem ? "已被系统禁用" : status.enabled ? "已开启" : "已关闭";
  return <section className="ym-page-card" aria-label="开机自启" aria-busy={busy}>
    <h2>开机自启</h2>
    <p className="ym-page-note">开启后，登录 Windows 时自动启动颜喵；未登记时默认关闭。</p>
    <p id={statusId} role="status">开机自启状态：{statusLabel}</p>
    {loading && <p role="status">正在读取开机自启状态…</p>}
    <div className="ym-inline-form">
      <button className="ym-button" type="button" role="switch" aria-label="开机自启"
        aria-checked={registered} aria-describedby={statusId} disabled={busy || !status}
        onClick={() => { void setEnabled(!registered); }}>
        {saving ? "正在设置…" : registered ? "关闭开机自启" : "开启开机自启"}
      </button>
      {status?.needsRepair && <button className="ym-button" type="button" disabled={busy}
        onClick={() => { void setEnabled(true); }}>修复自启路径</button>}
    </div>
    {status?.needsRepair && <p className="ym-page-note">便携版目录已变更或登记路径异常，现有启动项无法启动当前程序。可修复自启路径，也可关闭旧登记。</p>}
    {status?.blockedBySystem && <p className="ym-page-note">此启动项已被 Windows 禁用。请在 Windows 任务管理器的“启动应用”中启用颜喵（YanMew）；也可关闭自启登记。</p>}
    {notice && <p role="status">{notice}</p>}
    {error && <div className="ym-page-error" role="alert">{error}
      <button className="ym-button" type="button" disabled={busy} onClick={() => { void readStatus(); }}>重新读取</button>
    </div>}
  </section>;
}
