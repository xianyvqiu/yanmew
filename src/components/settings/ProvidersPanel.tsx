import { useCallback, useEffect, useRef, useState } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { AppError, Provider, ProviderInput } from "../../types";
import ProviderForm from "./ProviderForm";
import EmbeddingProvidersPanel from "./EmbeddingProvidersPanel";
import "./ProvidersPanel.css";

export default function ProvidersPanel() {
  const [providers, setProviders] = useState<Provider[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState("");
  const [editing, setEditing] = useState<Provider | null | undefined>(undefined);
  const [deleting, setDeleting] = useState<Provider | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const mounted = useRef(false);
  const lock = useRef(false);
  const generation = useRef(0);
  const dialog = useRef<HTMLDialogElement>(null);
  const refresh = useCallback(async () => {
    const token = ++generation.current;
    setLoading(true);
    try {
      const items = await configurationApi.listProviders();
      if (mounted.current && token === generation.current) { setProviders(items); setLoaded(true); }
    } finally { if (mounted.current && token === generation.current) setLoading(false); }
  }, []);
  const showError = useCallback((cause: unknown) => {
    if (!mounted.current) return;
    const detail = cause && typeof cause === "object" ? cause as Partial<AppError> : {};
    setError({ code: detail.code ?? "UNKNOWN", message: errorMessage(cause), field: detail.field });
  }, []);
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    let stop: (() => void) | undefined;
    const reload = () => { if (!disposed) void refresh().catch(showError); };
    void (async () => {
      try {
        const unsubscribe = await configurationApi.listenChanges(reload);
        if (disposed) { unsubscribe(); return; }
        stop = unsubscribe; reload();
      } catch (cause) { if (!disposed) { showError(cause); reload(); } }
    })();
    window.addEventListener("focus", reload);
    return () => { disposed = true; mounted.current = false; generation.current++; stop?.(); window.removeEventListener("focus", reload); };
  }, [refresh, showError]);
  useEffect(() => {
    if (deleting) dialog.current?.showModal();
    else dialog.current?.close();
  }, [deleting]);

  const run = async (action: () => Promise<unknown>, message: string, reload = true): Promise<boolean> => {
    if (lock.current) return false;
    lock.current = true; setBusy(true); setError(null); setNotice("");
    try {
      await action();
      if (!mounted.current) return true;
      // 写入已经成功，即使后续列表读取失败也不能让用户误以为需要重新提交新增。
      setNotice(message);
      if (reload) await refresh().catch(showError);
      return true;
    } catch (cause) { showError(cause); return false; }
    finally { lock.current = false; if (mounted.current) setBusy(false); }
  };
  const save = async (input: ProviderInput) => {
    const target = editing;
    const success = await run(() => target ? configurationApi.updateProvider(target.id, input) : configurationApi.createProvider(input), "提供商已保存");
    if (success && mounted.current) setEditing(undefined);
  };
  const test = async (provider: Provider) => {
    if (lock.current) return;
    setTestingId(provider.id);
    await run(() => configurationApi.testProvider(provider.id), `${provider.name}：连接成功（已保存配置）`);
    if (mounted.current) setTestingId(null);
  };
  const remove = async () => {
    if (!deleting) return;
    const success = await run(() => configurationApi.deleteProvider(deleting.id), "提供商已删除，聊天与记忆记录保留");
    if (mounted.current && success) {
      if (editing?.id === deleting.id) setEditing(undefined);
      setDeleting(null);
    }
  };
  const protectedProvider = deleting && (deleting.isActive || deleting.memoryIsActive);
  const disabled = busy || loading;
  return <>
    <section className="ym-page-card" aria-label="聊天与记忆整合模型">
      <div className="ym-page-heading"><h2>聊天与记忆整合模型</h2>
        <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(null); setError(null); setNotice(""); }}>新增提供商</button></div>
      <p className="ym-page-note">支持 OpenAI-compatible 接口。先保存配置，再测试连接；聊天和记忆可以分别选择模型。</p>
      {loading && <p role="status">正在读取提供商…</p>}
      {error && !deleting && (!error.field || editing === undefined) && <div className="ym-page-error" role="alert">{error.message}<button className="ym-button" disabled={busy} onClick={() => { setError(null); void refresh().catch(showError); }}>重新读取</button></div>}
      {notice && <p role="status">{notice}</p>}
      {loaded && !providers.length && <p className="ym-page-empty">暂无提供商，请先新增。</p>}
      {loaded && <div className="ym-provider-selectors">
        <label className="ym-field">聊天提供商<select disabled={disabled || !providers.length} value={providers.find(provider => provider.isActive)?.id ?? ""}
          onChange={event => { const id = event.target.value; void run(() => configurationApi.setChatProvider(id), "聊天提供商已切换"); }}>
          <option value="" disabled>请选择聊天提供商</option>{providers.map(provider => <option key={provider.id} value={provider.id}>{provider.name} · {provider.model}</option>)}
        </select></label>
        <label className="ym-field">记忆提供商<select disabled={disabled || !providers.length} value={providers.find(provider => provider.memoryIsActive)?.id ?? ""}
          onChange={event => { const id = event.target.value || null; void run(() => configurationApi.setMemoryProvider(id), id ? "记忆提供商已保存" : "记忆整合已设为跟随聊天提供商"); }}>
          <option value="">跟随聊天提供商</option>{providers.map(provider => <option key={provider.id} value={provider.id}>{provider.name} · {provider.model}</option>)}
        </select></label>
      </div>}
      <p className="ym-page-note">切换或编辑当前聊天提供商会中断正在生成的回复。记忆提供商用于后台短期记忆生成与长期整合。</p>
      {editing !== undefined && <ProviderForm key={editing?.id ?? "new"} provider={editing} busy={busy} error={error} onSave={save} onCancel={() => { setEditing(undefined); setError(null); }} />}
      <ul className="ym-provider-list">{providers.map(provider => <li key={provider.id}>
        <div className="ym-page-heading"><h3>{provider.name}</h3><span>{provider.isActive && "聊天当前项"}{provider.isActive && provider.memoryIsActive && " · "}{provider.memoryIsActive && "记忆专用项"}</span></div>
        <dl><dt>模型</dt><dd>{provider.model}</dd><dt>接口地址</dt><dd>{provider.baseUrl || "https://api.openai.com/v1"}</dd><dt>API Key</dt><dd>{provider.hasApiKey ? "已保存" : "未保存"}</dd></dl>
        <dl><dt>上下文容量</dt><dd>{provider.capacity.contextWindow == null ? "未知" : `${provider.capacity.contextWindow} tokens`}</dd>
          <dt>模型最大输出</dt><dd>{provider.capacity.maxOutputTokens == null ? "未知" : `${provider.capacity.maxOutputTokens} tokens`}</dd>
          <dt>能力来源</dt><dd>{provider.capacity.source === "manual" ? "手动设置" : provider.capacity.source === "provider" ? "提供商" : provider.capacity.source === "builtin" ? "已核对模型资料" : "未知"}{provider.capacity.checkedAt && `（${provider.capacity.checkedAt}）`}</dd>
          <dt>配置最大输出</dt><dd>{provider.maxOutputTokens} tokens</dd>
          <dt>输出生效上限</dt><dd>{Math.min(provider.maxOutputTokens, provider.capacity.maxOutputTokens ?? provider.maxOutputTokens)} tokens；发送时还会按实际上下文剩余容量调整。</dd></dl>
        <div className="ym-inline-form">
          <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(provider); setError(null); setNotice(""); }} aria-label={`编辑 ${provider.name}`}>编辑</button>
          <button className="ym-button" disabled={disabled} onClick={() => { void test(provider); }} aria-label={`测试连接 ${provider.name}`}>{testingId === provider.id ? "正在测试…" : "测试连接"}</button>
          <button className="ym-button ym-provider-delete" disabled={disabled} onClick={() => { setDeleting(provider); setError(null); }} aria-label={`删除 ${provider.name}`}>删除</button>
        </div>
      </li>)}</ul>
    </section>
    <EmbeddingProvidersPanel />
    <dialog ref={dialog} className="ym-provider-dialog" aria-labelledby="provider-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="provider-delete-title">{protectedProvider ? "此提供商正在使用" : "删除提供商"}</h2>
      <p>{protectedProvider ? "正在使用的提供商不能删除，请先切换聊天提供商或更改记忆提供商。" : `确定删除“${deleting?.name ?? ""}”的配置和密钥吗？已保存的聊天与记忆不会删除。`}</p>
      {deleting && error && <p role="alert" className="ym-page-error">{error.message}</p>}
      <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>{protectedProvider ? "知道了" : "取消"}</button>
        {!protectedProvider && <button className="ym-button ym-provider-delete" disabled={busy} onClick={() => { void remove(); }}>{busy ? "正在删除…" : "确认删除"}</button>}</div>
    </dialog>
  </>;
}
