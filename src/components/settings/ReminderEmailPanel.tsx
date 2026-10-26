import "./ReminderEmailPanel.css";
import { useEffect, useRef, useState } from "react";
import { errorMessage, reminderEmailApi, webSearchApi } from "../../api";
import type { ReminderEmailConnection, ReminderEmailView } from "../../types";

const empty: ReminderEmailConnection = { email: "", host: "", port: 465, encryption: "tls", username: "", from: "", to: "" };
const presets: Record<string, { host: string; name: string; site: string; help: string; guide: string }> = {
  "qq.com": { host: "smtp.qq.com", name: "QQ邮箱", site: "https://mail.qq.com/", help: "https://service.mail.qq.com/", guide: "打开网页版邮箱 → 设置 → 查找“账号”“SMTP”或“客户端授权码” → 开启SMTP服务 → 按邮箱要求完成验证 → 生成并复制授权码。新旧网页版的菜单名称可能不同，可在官方帮助搜索“SMTP 授权码”。" },
  "163.com": { host: "smtp.163.com", name: "163邮箱", site: "https://mail.163.com/", help: "https://help.mail.163.com/", guide: "打开网页版邮箱 → 设置 → POP3/SMTP/IMAP或客户端授权密码 → 开启包含SMTP的客户端服务 → 按邮箱要求完成身份验证 → 获取并复制客户端授权码。" },
  "126.com": { host: "smtp.126.com", name: "126邮箱", site: "https://mail.126.com/", help: "https://help.mail.126.com/", guide: "打开网页版邮箱 → 设置 → POP3/SMTP/IMAP或客户端授权密码 → 开启包含SMTP的客户端服务 → 按邮箱要求完成身份验证 → 获取并复制客户端授权码。" },
};
export function recommendedEmail(email: string): ReminderEmailConnection {
  const normalized = email.trim();
  const preset = presets[normalized.split("@")[1]?.toLowerCase()];
  return { email: normalized, host: preset?.host ?? "", port: 465, encryption: "tls", username: normalized, from: normalized, to: "" };
}
const stateNames: Record<string, string> = { pending: "待发送", sending: "发送中", retry_wait: "等待重试", sent: "已发送", failed: "发送失败", uncertain: "结果不确定", cancelled: "已取消" };

export default function ReminderEmailPanel() {
  const [view, setView] = useState<ReminderEmailView | null>(null);
  const [connection, setConnection] = useState(empty);
  const [secret, setSecret] = useState("");
  const [code, setCode] = useState("");
  const [codeSent, setCodeSent] = useState(false);
  const [cooldownUntil, setCooldownUntil] = useState(0);
  const [seconds, setSeconds] = useState(0);
  const [custom, setCustom] = useState<Set<keyof ReminderEmailConnection>>(new Set());
  const [editing, setEditing] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const mounted = useRef(false);
  const locked = useRef(false);
  const dirty = useRef(false);
  const actionVersion = useRef(0);
  const draftRevision = useRef(0);
  const seenRevision = useRef<number | null>(null);
  const sync = (next: ReminderEmailView, draft = true) => {
    setView(next);
    seenRevision.current = next.revision;
    if (draft) {
      setConnection(next.connection); setSecret(""); dirty.current = false; draftRevision.current = next.revision;
      const recommended = recommendedEmail(next.connection.email);
      setCustom(new Set((Object.keys(recommended) as (keyof ReminderEmailConnection)[]).filter(key => next.connection[key] !== recommended[key])));
    }
  };
  const refresh = async (initial = false) => {
    if (locked.current) return;
    const version = actionVersion.current;
    try {
      const next = await reminderEmailApi.get();
      if (!mounted.current || locked.current || version !== actionVersion.current) return;
      if (seenRevision.current !== null && seenRevision.current !== next.revision) { setCodeSent(false); setCode(""); }
      seenRevision.current = next.revision;
      sync(next, initial || !dirty.current);
      if (initial || !next.verified) setEditing(!next.verified);
    } catch (cause) { if (mounted.current) setError(errorMessage(cause)); }
  };
  useEffect(() => {
    mounted.current = true;
    void refresh(true);
    const interval = window.setInterval(() => { void refresh(); }, 5000);
    const focus = () => { void refresh(); };
    window.addEventListener("focus", focus);
    return () => { mounted.current = false; window.clearInterval(interval); window.removeEventListener("focus", focus); };
  }, []);
  useEffect(() => {
    const tick = () => setSeconds(Math.max(0, Math.ceil((cooldownUntil - Date.now()) / 1000)));
    tick(); const interval = window.setInterval(tick, 1000);
    return () => window.clearInterval(interval);
  }, [cooldownUntil]);
  const run = async (action: () => Promise<void>) => {
    if (locked.current) return;
    actionVersion.current++;
    locked.current = true; setBusy(true); setError(""); setNotice("");
    try { await action(); }
    catch (cause) { if (mounted.current) setError(errorMessage(cause)); }
    finally { locked.current = false; if (mounted.current) setBusy(false); }
  };
  const changed = () => { dirty.current = true; setCodeSent(false); setCode(""); setNotice(""); };
  const changeEmail = (email: string) => {
    changed();
    const defaults = recommendedEmail(email);
    setConnection(previous => {
      const next = { ...previous, email };
      for (const key of ["host", "port", "encryption", "username", "from"] as const) {
        if (!custom.has(key)) Object.assign(next, { [key]: defaults[key] });
      }
      return next;
    });
  };
  const advance = <K extends keyof ReminderEmailConnection>(key: K, value: ReminderEmailConnection[K]) => {
    changed(); setCustom(previous => new Set([...previous, key]));
    setConnection(previous => ({ ...previous, [key]: value, ...(key === "encryption" && !custom.has("port") ? { port: value === "starttls" ? 587 : 465 } : {}) }));
  };
  const input = () => ({ connection, revision: dirty.current ? draftRevision.current : view!.revision, secret: secret.trim() ? { action: "set" as const, value: secret.trim() } : { action: "keep" as const } });
  const sendCode = () => run(async () => {
    try {
      const next = await reminderEmailApi.sendCode(input());
      if (!mounted.current) return;
      sync(next); setCodeSent(true); setCode(""); setCooldownUntil(Date.now() + 60_000);
      setNotice(`验证码已发送到 ${next.connection.to}，10分钟内有效。请同时检查垃圾邮件。`);
    } catch (cause) {
      // SMTP失败前配置可能已保存，取回权威修订号后才能重试；不丢弃未保存的表单。
      try { const next = await reminderEmailApi.get(); if (mounted.current) { sync(next, false); if (JSON.stringify(next.connection) === JSON.stringify(connection) && next.hasSecret) { draftRevision.current = next.revision; setSecret(""); } setCooldownUntil(Date.now() + 60_000); } } catch { /* 保留原始发信错误 */ }
      throw cause;
    }
  });
  const verify = () => run(async () => {
    const next = await reminderEmailApi.verify(code, view!.revision);
    if (!mounted.current) return;
    sync(next); setEditing(false); setCodeSent(false); setCode(""); setNotice("邮箱已验证，邮件提醒已开启。");
  });
  const preset = presets[connection.email.trim().split("@")[1]?.toLowerCase()];
  return <section className="ym-page-card ym-email-panel">
    <div className="ym-email-header"><h2>邮件提醒</h2>
      {view && <button type="button" className="ym-email-switch" role="switch" aria-label="邮件提醒" aria-checked={view.enabled} disabled={busy || !view.verified} onClick={() => {
        const enabled = !view.enabled;
        void run(async () => { const next = await reminderEmailApi.enable(enabled, view.revision); if (mounted.current) { sync(next, false); setCodeSent(false); setNotice(enabled ? "邮件提醒已开启，只发送此后触发的提醒。" : "邮件提醒已关闭，未开始发送的任务已取消。"); } });
      }}><span>{view.enabled ? "已开启" : "已关闭"}</span></button>}
    </div>
    <p className="ym-page-note">到点时同时显示界面提醒并发送邮件。关闭颜喵期间无法发信，重新打开后会汇总离开期间的过期事项。</p>
    {!view && !error && <p role="status">正在读取邮件设置…</p>}
    {view && <>
      <p className="ym-page-note">{view.verified ? `已验证 · ${view.connection.to}` : "尚未绑定邮箱。请分别填写发信邮箱和收信邮箱，完成验证码验证后才能开启。"}</p>
      {view.verified && <button type="button" className="ym-button" disabled={busy} onClick={() => setEditing(value => !value)}>{editing ? "收起配置" : "修改邮箱配置"}</button>}
      {editing && <form onSubmit={event => { event.preventDefault(); void (codeSent ? verify() : sendCode()); }}>
        <fieldset disabled={busy} className="ym-email-fields">
          <div><label className="ym-field">发信邮箱<input type="email" autoComplete="email" value={connection.email} onChange={event => changeEmail(event.target.value)} placeholder="例如 yourname@qq.com" required /></label>
            <p className="ym-page-note">用这个邮箱发送提醒；下方授权码也必须由这个邮箱生成。收信邮箱在下方单独填写。</p>
          </div>
          <div className="ym-email-guide">
            <strong>如何获取授权码{preset ? ` · ${preset.name}` : ""}</strong>
            <p>{preset?.guide ?? "在邮箱服务商的账号安全、应用专用密码或SMTP帮助中开启SMTP并获取授权凭据。其他邮箱还需在下方填写SMTP服务器；请使用服务商提供的真实配置。"}</p>
            {preset && <div className="ym-inline-form"><button type="button" className="ym-button" onClick={() => { void run(() => webSearchApi.open(preset.site)); }}>打开邮箱网页</button><button type="button" className="ym-button" onClick={() => { void run(() => webSearchApi.open(preset.help)); }}>打开官方帮助</button></div>}
            <p className="ym-page-note">授权码用于连接发信邮箱，不是邮箱登录密码，也不是下一步收到的6位验证码。获取授权码时的短信、扫码等验证请在邮箱官网完成。</p>
          </div>
          <label className="ym-field">发信邮箱授权码<input type="password" autoComplete="new-password" value={secret} placeholder={view.hasSecret ? "已保存，留空保留；输入新授权码可替换" : "粘贴发信邮箱生成的授权码"} onChange={event => { changed(); setSecret(event.target.value); }} /></label>
          <div><label className="ym-field">收信邮箱<input type="email" autoComplete="email" value={connection.to} onChange={event => advance("to", event.target.value)} placeholder="填写接收提醒的邮箱地址" required /></label>
            <p className="ym-page-note">验证码和之后的提醒都发到这里。不需要提供收信邮箱的授权码；可以与发信邮箱相同，也可以不同，请自行填写。</p>
          </div>
          {view.hasSecret && <button type="button" className="ym-button" onClick={() => {
            if (!window.confirm("清除已保存的邮箱授权码？邮件提醒将关闭，需要重新验证。")) return;
            void run(async () => { const next = await reminderEmailApi.save({ connection: view.connection, revision: view.revision, secret: { action: "clear" } }); if (mounted.current) { sync(next); setCodeSent(false); setNotice("授权码已清除，邮件提醒已关闭。"); } });
          }}>清除已保存的授权码</button>}
          {preset ? <p className="ym-page-note">已自动配置发信服务器：{connection.host} · {connection.port} · {connection.encryption === "tls" ? "TLS" : "STARTTLS"}。通常无需修改；特殊情况可展开高级设置。</p> : <div>
            <label className="ym-field">发信服务器（SMTP）<input value={connection.host} onChange={event => advance("host", event.target.value)} placeholder="例如 smtp.example.com，不填 https://" required /></label>
            <p className="ym-page-note">这是发信邮箱服务商提供的服务器域名，不是邮箱地址。请到发信邮箱官网的“SMTP/客户端设置”中查询；QQ、163、126邮箱会自动填写。</p>
          </div>}
          <details className="ym-email-advanced"><summary>高级设置{[...custom].some(key => key !== "to") ? " · 使用了自定义配置" : " · 已自动预填"}</summary>
            <p className="ym-page-note">下面只配置如何登录发信服务器，普通邮箱通常不用修改。不需要POP或IMAP收信服务器。修改已绑定的连接配置后必须重新验证。</p>
            <div className="ym-email-grid">
              {preset && <div><label className="ym-field">发信服务器（SMTP）<input value={connection.host} onChange={event => advance("host", event.target.value)} required /></label><p className="ym-page-note">发信邮箱服务商提供的服务器域名；例如QQ邮箱为smtp.qq.com，不填邮箱地址或网页链接。</p></div>}
              <label className="ym-field">连接加密<select value={connection.encryption} onChange={event => advance("encryption", event.target.value as "tls" | "starttls")}><option value="tls">TLS（推荐465端口）</option><option value="starttls">STARTTLS（推荐587端口）</option></select></label>
              <label className="ym-field">SMTP端口<input type="number" min={1} max={65535} value={connection.port} onChange={event => advance("port", Number(event.target.value))} required /></label>
              <div><label className="ym-field">SMTP登录账号<input value={connection.username} onChange={event => advance("username", event.target.value)} required /></label><p className="ym-page-note">用于登录发信服务器，通常就是上方的完整发信邮箱，已自动填写。仅服务商明确要求其他账号时修改，不是收信邮箱。</p></div>
              <div><label className="ym-field">实际发件地址<input type="email" value={connection.from} onChange={event => advance("from", event.target.value)} required /></label><p className="ym-page-note">邮件上显示的发件地址，默认与发信邮箱相同。仅使用服务商允许的邮箱别名发信时需要修改。</p></div>
            </div>
            <p className="ym-page-note">更改上方邮箱时保留你手动设置过的字段。</p>
            <button type="button" className="ym-button" onClick={() => { changed(); setConnection(previous => ({ ...recommendedEmail(previous.email), to: previous.to })); setCustom(new Set(connection.to ? ["to"] : [])); }}>恢复推荐配置</button>
            <p className="ym-page-note">恢复推荐配置仅重置发信服务器等选项，不改变你填写的收信邮箱。</p>
          </details>
          {codeSent && <label className="ym-field">6位验证码<input inputMode="numeric" autoComplete="one-time-code" pattern="[0-9]{6}" maxLength={6} value={code} onChange={event => setCode(event.target.value.replace(/\D/g, ""))} placeholder="收到邮件后填写" required /></label>}
          <div className="ym-inline-form">
            <button type={codeSent ? "button" : "submit"} className="ym-button" disabled={busy || seconds > 0} onClick={codeSent ? () => { void sendCode(); } : undefined}>{seconds > 0 ? `${seconds}秒后可重发` : "发送验证码"}</button>
            {codeSent && <button type="submit" className="ym-button" disabled={busy || code.length !== 6}>验证并开启</button>}
            {view.verified && <button type="button" className="ym-button" onClick={() => { void run(async () => { const next = await reminderEmailApi.save(input()); if (mounted.current) { sync(next); setCodeSent(false); setNotice(next.verified ? "配置已保存。" : "配置已保存，旧绑定已失效，请重新发送验证码。 "); } }); }}>保存配置</button>}
          </div>
        </fieldset>
      </form>}
      <details className="ym-email-history"><summary>最近发送记录{view.deliveries.length ? `（${view.deliveries.length}）` : ""}</summary>
        <p className="ym-page-note">显示最近20条。仅当前已验证配置下的失败任务可重试；结果不确定时先检查收件箱。</p>
        {!view.deliveries.length && <p className="ym-page-note">暂无邮件提醒记录。</p>}
        {view.deliveries.map(item => <article key={item.id} className="ym-email-delivery">
          <div className="ym-inline-form"><strong>{item.subject || "提醒邮件"}</strong><span>{stateNames[item.state] ?? item.state}</span></div>
          <p className="ym-page-note">{item.recipient} · {new Date(item.createdAt).toLocaleString()} · 尝试{item.attempts}次</p>
          {item.error && <p className="ym-page-error">{item.error}</p>}
          {item.nextRetryAt && <p className="ym-page-note">下次重试：{new Date(item.nextRetryAt).toLocaleString()}</p>}
          {["failed", "uncertain"].includes(item.state) && <button type="button" className="ym-button" disabled={busy || !view.enabled} onClick={() => {
            if (item.state === "uncertain" && !window.confirm("该邮件可能已经送达。请先检查收件箱；再次发送可能收到重复邮件。仍要重发吗？")) return;
            void run(async () => { const next = await reminderEmailApi.retry(item.id, view.revision, item.state === "uncertain"); if (mounted.current) { sync(next, false); setNotice("已登记重试，将在后台发送。"); } });
          }}>{item.state === "uncertain" ? "确认后重新发送" : "重试发送"}</button>}
        </article>)}
      </details>
    </>}
    {busy && <p role="status" className="ym-page-note">正在处理，请稍候…</p>}
    {notice && <p role="status" className="ym-page-note">{notice}</p>}
    {error && <div role="alert" className="ym-page-error">{error}{!view ? <button className="ym-button" onClick={() => { void refresh(true); }}>重新读取</button> : <button type="button" className="ym-button" disabled={busy} onClick={() => { void run(async () => { const next = await reminderEmailApi.get(); if (mounted.current) { sync(next); setEditing(true); setCodeSent(false); setCode(""); setNotice("已重新读取邮件配置。"); } }); }}>{dirty.current ? "放弃修改并重新读取" : "重新读取配置"}</button>}</div>}
  </section>;
}
