import {useRevealForm} from "../../hooks/useRevealForm";
import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, webSearchApi } from "../../api";
import type { AppError, WebSearchKind, WebSearchProvider, WebSearchProviderInput, WebSearchSettings } from "../../types";

const endpoints = { tavily: "https://api.tavily.com", bocha: "https://api.bocha.cn", custom: "" };
const serviceNames = { tavily: "Tavily", bocha: "博查", custom: "自定义（Tavily 兼容）" };
function SearchForm({ provider, busy, error, save, cancel }: { provider: WebSearchProvider | null; busy: boolean; error: AppError | null; save: (input: WebSearchProviderInput) => Promise<void>; cancel: () => void }) {
  const form=useRevealForm();
  const [name, setName] = useState(provider?.name ?? "");
  const [kind, setKind] = useState<WebSearchKind>(provider?.kind ?? "tavily");
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? endpoints.tavily);
  const [key, setKey] = useState("");
  const [clear, setClear] = useState(false);
  return <form ref={form} aria-label="搜索提供商表单" className="ym-provider-form" onSubmit={event => {
    event.preventDefault();
    void save({ name, kind, baseUrl, apiKey: clear ? { action: "clear" } : key ? { action: "set", value: key } : { action: "keep" } });
  }}>
    <h3>{provider ? "编辑搜索提供商" : "新增搜索提供商"}</h3>
    <label className="ym-field">名称<input required maxLength={80} disabled={busy} value={name} onChange={e => setName(e.target.value)} /></label>
    <label className="ym-field">服务类型<select disabled={busy} value={kind} onChange={e => {
      const value = e.target.value as WebSearchKind; setKind(value); setBaseUrl(endpoints[value]); setKey(""); setClear(false);
    }}><option value="tavily">Tavily</option><option value="bocha">博查</option><option value="custom">自定义（Tavily 兼容）</option></select></label>
    <label className="ym-field">接口地址<input required type="url" disabled={busy} value={baseUrl} onChange={e => setBaseUrl(e.target.value)} /></label>
    <label className="ym-field">API Key<input type="password" autoComplete="new-password" disabled={busy || clear} value={key} placeholder={provider?.hasApiKey ? "留空保留已保存的 Key" : "填写自己的 Key"} onChange={e => setKey(e.target.value)} /></label>
    {provider?.hasApiKey && <label><input type="checkbox" disabled={busy} checked={clear} onChange={e => { setClear(e.target.checked); setKey(""); }} />清除已保存的 Key</label>}
    <p className="ym-page-note">更换服务类型或地址时，请重新填写或清除 Key。Key 加密保存，不进入备份。</p>
    {kind === "custom" && <p className="ym-page-note">自定义接口须兼容 Tavily：填写接口根地址，搜索时追加 /search，使用 Bearer Key 并解析 results。普通聊天模型接口不能直接作为搜索接口。</p>}
    {error && <p role="alert" className="ym-page-error">{error.message}</p>}
    <div className="ym-inline-form"><button className="ym-button" type="submit" disabled={busy}>{busy ? "处理中…" : "保存"}</button><button className="ym-button" type="button" disabled={busy} onClick={cancel}>取消</button></div>
  </form>;
}

export default function WebSearchProvidersPanel() {
  const [data, setData] = useState<WebSearchSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<WebSearchProvider | null | undefined>();
  const [deleting, setDeleting] = useState<WebSearchProvider | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState("");
  const mounted = useRef(false), locked = useRef(false), generation = useRef(0);
  const dialog = useRef<HTMLDialogElement>(null);
  const showError = useCallback((cause: unknown) => {
    if (!mounted.current) return;
    const value = cause && typeof cause === "object" ? cause as Partial<AppError> : {};
    setError({ code: value.code ?? "UNKNOWN", message: errorMessage(cause), field: value.field });
  }, []);
  const refresh = useCallback(async () => {
    const token = ++generation.current, result = await webSearchApi.get();
    if (mounted.current && token === generation.current) setData(result);
  }, []);
  useEffect(() => {
    mounted.current = true; let disposed = false, stop: (() => void) | undefined;
    const reload = () => { void refresh().catch(showError); };
    void webSearchApi.listen(reload).then(unsubscribe => { if (disposed) unsubscribe(); else { stop = unsubscribe; reload(); } }).catch(cause => { showError(cause); reload(); });
    window.addEventListener("focus", reload);
    return () => { disposed = true; mounted.current = false; generation.current++; stop?.(); window.removeEventListener("focus", reload); };
  }, [refresh, showError]);
  useEffect(() => { if (deleting) dialog.current?.showModal(); else dialog.current?.close(); }, [deleting]);
  const run = async (action: () => Promise<unknown>, message: string) => {
    if (locked.current) return false;
    locked.current = true; setBusy(true); setError(null); setNotice("");
    try { await action(); if (mounted.current) { setNotice(message); await refresh().catch(showError); } return true; }
    catch (cause) { showError(cause); return false; }
    finally { locked.current = false; if (mounted.current) setBusy(false); }
  };
  const save = async (input: WebSearchProviderInput) => {
    if (await run(() => webSearchApi.save(editing?.id ?? null, input), "搜索配置已保存")) { if (mounted.current) setEditing(undefined); }
  };
  const disabled = !data || busy || deleting !== null;
  return <section className="ym-page-card" aria-label="联网搜索">
    <div className="ym-page-heading"><h2>联网搜索</h2><button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(null); setError(null); }}>新增搜索服务</button></div>
    <p className="ym-page-note">默认关闭。启用后，AI 仅在正常聊天需要查资料时搜索；无聊主动消息、旅行、信件和后台记忆不搜索。只发送必要搜索词，不自动上传完整聊天。搜索结果和网页文字会交给当前聊天模型。</p>
    <p className="ym-page-note">使用你自己的 Key。搜索及测试连接可能消耗服务额度；费用以服务商为准。网页读取不登录、不执行脚本，受限制的网站可能无法读取。</p>
    {!data && !error && <p role="status">正在读取搜索配置…</p>}
    {error && editing === undefined && <p role="alert" className="ym-page-error">{error.message}<button className="ym-button" disabled={busy} onClick={() => { setError(null); void refresh().catch(showError); }}>重新加载</button></p>}
    {notice && <p role="status">{notice}</p>}
    {editing !== undefined && <SearchForm key={editing?.id ?? "new"} provider={editing ?? null} busy={busy} error={error} save={save} cancel={() => { setEditing(undefined); setError(null); }} />}
    {data && <>
      <label><input type="checkbox" disabled={disabled || editing !== undefined} checked={data.enabled} onChange={e => { void run(() => webSearchApi.enable(e.target.checked), e.target.checked ? "联网搜索已开启" : "联网搜索已关闭"); }} />允许正常聊天联网搜索</label>
      <label className="ym-field">当前搜索服务<select disabled={disabled || editing !== undefined || !data.providers.length} value={data.providers.find(p => p.isActive)?.id ?? ""} onChange={e => { void run(() => webSearchApi.select(e.target.value), "搜索服务已切换"); }}>
        {!data.providers.length && <option value="">暂无搜索服务，请先新增</option>}
        {data.providers.map(p => <option key={p.id} value={p.id}>{p.name} · {serviceNames[p.kind]}</option>)}
      </select></label>
      <ul className="ym-provider-list">{data.providers.map(p => <li key={p.id}>
        <div className="ym-page-heading"><h3>{p.name}</h3><span>{p.isActive ? "当前项" : ""}</span></div>
        <dl><dt>服务</dt><dd>{serviceNames[p.kind]}</dd><dt>接口地址</dt><dd>{p.baseUrl}</dd><dt>API Key</dt><dd>{p.hasApiKey ? "已保存" : "未保存"}</dd></dl>
        <div className="ym-inline-form">
          <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(p); setError(null); }} aria-label={`编辑搜索 ${p.name}`}>编辑</button>
          <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { let count = 0; void run(async () => { count = await webSearchApi.test(p.id); }, "搜索测试成功").then(ok => { if (ok && mounted.current) setNotice(`${p.name}：测试成功，返回 ${count} 条结果（测试可能消耗服务额度）。`); }); }} aria-label={`测试搜索 ${p.name}`}>测试连接</button>
          <button className="ym-button ym-provider-delete" disabled={disabled || editing !== undefined} onClick={() => { setDeleting(p); setError(null); }} aria-label={`删除搜索 ${p.name}`}>删除</button>
        </div>
      </li>)}</ul>
    </>}

    <dialog ref={dialog} className="ym-provider-dialog" aria-labelledby="search-delete-title" onCancel={e => { if (busy) e.preventDefault(); else setDeleting(null); }}>
      <h2 id="search-delete-title">{deleting?.isActive ? "此搜索服务正在使用" : "删除搜索配置"}</h2>
      <p>{deleting?.isActive ? "当前搜索服务不能删除，请先切换当前项。" : `删除“${deleting?.name ?? ""}”的配置和 Key？已保存的聊天保留。`}</p>
      {error && <p role="alert" className="ym-page-error">{error.message}</p>}
      <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>{deleting?.isActive ? "知道了" : "取消"}</button>
        {deleting && !deleting.isActive && <button className="ym-button ym-provider-delete" disabled={busy} onClick={() => { const id = deleting.id; void run(() => webSearchApi.delete(id), "搜索配置已删除").then(ok => { if (ok && mounted.current) setDeleting(null); }); }}>确认删除</button>}
      </div>
    </dialog>
  </section>;
}
