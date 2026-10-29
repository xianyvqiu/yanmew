import ProviderTestStatus, {type ConnectionTestFeedback} from "./ProviderTestStatus";
import { useCallback, useEffect, useRef, useState } from "react";
import { embeddingApi, errorMessage } from "../../api";
import type { AppError, EmbeddingProvider, EmbeddingStatus, ProviderInput } from "../../types";
import ProviderForm from "./ProviderForm";

export default function EmbeddingProvidersPanel() {
  const [providers, setProviders] = useState<EmbeddingProvider[]>([]);
  const [status, setStatus] = useState<EmbeddingStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<EmbeddingProvider | null | undefined>();
  const [deleting, setDeleting] = useState<EmbeddingProvider | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState("");
  const [tests, setTests] = useState<Partial<Record<string, ConnectionTestFeedback>>>({});
  const mounted = useRef(false);
  const locked = useRef(false);
  const generation = useRef(0);
  const dialog = useRef<HTMLDialogElement>(null);
  const showError = useCallback((cause: unknown) => {
    if (!mounted.current) return;
    const value = cause && typeof cause === "object" ? cause as Partial<AppError> : {};
    setError({ code: value.code ?? "UNKNOWN", message: errorMessage(cause), field: value.field });
  }, []);
  const refresh = useCallback(async () => {
    const token = ++generation.current;
    const [items, progress] = await Promise.all([embeddingApi.list(), embeddingApi.status()]);
    if (mounted.current && token === generation.current) { setProviders(items); setStatus(progress); setLoading(false); }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    let stop: (() => void) | undefined;
    const reload = () => { void refresh().catch(showError).finally(() => { if (mounted.current) setLoading(false); }); };
    void embeddingApi.listen(reload).then(unsubscribe => {
      if (disposed) unsubscribe(); else { stop = unsubscribe; reload(); }
    }).catch(cause => { showError(cause); reload(); });
    window.addEventListener("focus", reload);
    return () => { disposed = true; mounted.current = false; generation.current++; stop?.(); window.removeEventListener("focus", reload); };
  }, [refresh, showError]);
  useEffect(() => { if (deleting) dialog.current?.showModal(); else dialog.current?.close(); }, [deleting]);
  const run = async (action: () => Promise<unknown>, message: string | (() => string), testId?: string) => {
    if (locked.current) return false;
    locked.current = true; setBusy(true); setError(null); setNotice("");
    if (testId) setTests(current=>({...current,[testId]:{phase:"testing",message:"请稍候，正在检查连接。"}}));
    try {
      await action();
      if (mounted.current) {
        const text=typeof message === "function"?message():message;
        if (testId) setTests(current=>({...current,[testId]:{phase:"success",message:text}}));
        else setNotice(text);
        await refresh().catch(showError);
      }
      return true;
    }
    catch (cause) {
      if (testId) { if (mounted.current) setTests(current=>({...current,[testId]:{phase:"error",message:errorMessage(cause)}})); }
      else showError(cause);
      return false;
    }
    finally { locked.current = false; if (mounted.current) setBusy(false); }
  };
  const save = async (input: ProviderInput) => {
    if (await run(() => embeddingApi.save(editing?.id ?? null,{...input,model:input.model??""}), "Embedding 提供商已保存")) {
      if (mounted.current) { if (editing) setTests(current=>({...current,[editing.id]:undefined})); setEditing(undefined); }
    }
  };
  const test = async (provider: EmbeddingProvider) => {
    let dimensions = 0;
    await run(async () => { dimensions = await embeddingApi.test(provider.id); }, () => `${provider.name}：测试成功，返回 ${dimensions} 维向量。`, provider.id);
  };
  const protectedItem = deleting && (deleting.isBuiltin || deleting.isActive);
  const disabled = loading || busy;
  return <section className="ym-page-card" aria-label="Embedding 模型">
    <div className="ym-page-heading"><h2>Embedding 模型</h2><button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(null); setError(null); }}>新增联网模型</button></div>
    <p className="ym-page-note">默认本地离线运行。联网模型会把待索引的长期记忆和检索文字发送给你配置的服务，可能产生费用。切换后后台重建向量，记忆正文保留，全文搜索仍可使用。</p>
    <p className="ym-page-note">聊天已接入长期记忆自动回忆；向量不可用时仍会使用本地全文检索。</p>
    {loading && <p role="status">正在读取 Embedding 配置…</p>}
    {error && (!error.field || editing === undefined) && <p className="ym-page-error" role="alert">{error.message}</p>}
    {notice && <p role="status">{notice}</p>}
    {editing !== undefined && <ProviderForm embedding key={editing?.id ?? "new"} provider={editing ?? null} busy={busy} error={error} onSave={save} onCancel={() => { setEditing(undefined); setError(null); }} />}
    {!loading && <label className="ym-field">当前 Embedding 模型<select disabled={disabled} value={providers.find(p => p.isActive)?.id ?? ""} onChange={event => { void run(() => embeddingApi.select(event.target.value), "模型已切换，后台将恢复补建"); }}>
      {providers.map(provider => <option key={provider.id} value={provider.id}>{provider.name} · {provider.kind === "local" ? "本地" : "联网"}</option>)}
    </select></label>}
    {status && <div role="status"><p>长期记忆向量：{status.ready}/{status.total} 已就绪，{status.pending} 条待补建{status.running ? "（正在处理）" : ""}。</p>
      {status.unavailable > 0 && <p>{status.unavailable} 条超出本地模型长度，本次仅使用全文搜索。</p>}
      {status.error && <p className="ym-page-error">{status.error.message}；全文搜索仍可使用。修复后点击当前模型的“测试连接”恢复补建。</p>}
    </div>}

    <ul className="ym-provider-list">{providers.map(provider => <li key={provider.id}>
      <div className="ym-page-heading"><h3>{provider.name}</h3><span>{provider.isBuiltin ? "内置 · " : ""}{provider.isActive ? "当前项" : ""}</span></div>
      <dl><dt>模型</dt><dd>{provider.model}</dd><dt>运行方式</dt><dd>{provider.kind === "local" ? "本地 CPU，无需联网" : provider.baseUrl}</dd>{provider.kind === "online" && <><dt>API Key</dt><dd>{provider.hasApiKey ? "已保存" : "未保存"}</dd></>}</dl>
      <div className="ym-inline-form">
        {!provider.isBuiltin && <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(provider); setTests(current=>({...current,[provider.id]:undefined})); setError(null); }} aria-label={`编辑 Embedding ${provider.name}`}>编辑</button>}
        <button className="ym-button" disabled={disabled} onClick={() => { void test(provider); }} aria-label={`测试 Embedding ${provider.name}`}>{tests[provider.id]?.phase === "testing" ? "正在测试…" : "测试连接"}</button>
        {!provider.isBuiltin && <button className="ym-button ym-provider-delete" disabled={disabled} onClick={() => { setDeleting(provider); setError(null); }} aria-label={`删除 Embedding ${provider.name}`}>删除</button>}
      </div>
      <ProviderTestStatus feedback={tests[provider.id]}/>
    </li>)}</ul>
    <dialog ref={dialog} className="ym-provider-dialog" aria-labelledby="embedding-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="embedding-delete-title">{protectedItem ? "此模型正在使用" : "删除 Embedding 配置"}</h2>
      <p>{protectedItem ? "内置项和当前项不能删除，请先切换当前模型。" : `删除“${deleting?.name ?? ""}”的配置和密钥？长期记忆保留。`}</p>
      {error && <p role="alert" className="ym-page-error">{error.message}</p>}
      <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>{protectedItem ? "知道了" : "取消"}</button>
        {!protectedItem && <button className="ym-button ym-provider-delete" disabled={busy} onClick={() => { if (deleting) void run(() => embeddingApi.delete(deleting.id), "Embedding 配置已删除").then(success => { if (success && mounted.current) setDeleting(null); }); }}>确认删除</button>}
      </div>
    </dialog>
  </section>;
}
