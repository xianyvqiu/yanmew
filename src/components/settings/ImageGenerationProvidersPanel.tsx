import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, imageGenerationApi } from "../../api";
import type { AppError, ImageGenerationConfig, ImageGenerationProvider, ImageGenerationProviderInput } from "../../types";

function ProviderForm({ provider, busy, error, save, cancel }: { provider: ImageGenerationProvider | null; busy: boolean; error: AppError | null; save: (input: ImageGenerationProviderInput) => Promise<void>; cancel: () => void }) {
  const [name, setName] = useState(provider?.name ?? "");
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? "https://api.openai.com/v1");
  const [model, setModel] = useState(provider?.model ?? "");
  const [key, setKey] = useState("");
  const [clear, setClear] = useState(false);
  return <form aria-label="生图提供商表单" className="ym-provider-form" onSubmit={event => {
    event.preventDefault();
    void save({ name, protocol: "openai-compatible", baseUrl, model, apiKey: clear ? { action: "clear" } : key ? { action: "set", value: key } : { action: "keep" } });
  }}>
    <h3>{provider ? "编辑生图提供商" : "新增生图提供商"}</h3>
    <label className="ym-field">名称<input required maxLength={80} disabled={busy} value={name} onChange={e => setName(e.target.value)} /></label>
    <label className="ym-field">接口协议<select disabled value="openai-compatible"><option value="openai-compatible">OpenAI-compatible</option></select></label>
    <label className="ym-field">HTTPS 接口根地址<input required type="url" disabled={busy} value={baseUrl} onChange={e => setBaseUrl(e.target.value)} /></label>
    <label className="ym-field">生图模型<input required maxLength={200} disabled={busy} value={model} placeholder="填写服务商支持透明输出的模型名" onChange={e => setModel(e.target.value)} /></label>
    <label className="ym-field">API Key<input type="password" autoComplete="new-password" disabled={busy || clear} value={key} placeholder={provider?.hasApiKey ? "留空保留已保存的 Key" : "填写自己的生图 Key"} onChange={e => setKey(e.target.value)} /></label>
    {provider && <label><input type="checkbox" disabled={busy} checked={clear} onChange={e => { setClear(e.target.checked); setKey(""); }} />{provider.hasApiKey ? "清除已保存的 Key" : "不保存 Key（更换地址时可选）"}</label>}
    <p className="ym-page-note">地址填写接口根目录，例如以 /v1 结尾；模型须支持透明图片。生图 Key 独立加密保存，更换根地址必须重新填写或明确清除，不借用聊天 Key。</p>
    {error && <p className="ym-page-error" role="alert">{error.message}</p>}
    <div className="ym-inline-form"><button className="ym-button" type="submit" disabled={busy}>{busy ? "处理中…" : "保存"}</button><button className="ym-button" type="button" disabled={busy} onClick={cancel}>取消</button></div>
  </form>;
}

export default function ImageGenerationProvidersPanel() {
  const [data, setData] = useState<ImageGenerationConfig | null>(null);
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<ImageGenerationProvider | null | undefined>();
  const [deleting, setDeleting] = useState<ImageGenerationProvider | null>(null);
  const [enabling, setEnabling] = useState(false);
  const [editRevision, setEditRevision] = useState(0);
  const [enableRevision, setEnableRevision] = useState(0);
  const [deleteRevision, setDeleteRevision] = useState(0);
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState("");
  const mounted = useRef(false), locked = useRef(false), generation = useRef(0);
  const deleteDialog = useRef<HTMLDialogElement>(null), enableDialog = useRef<HTMLDialogElement>(null);
  const showError = useCallback((cause: unknown) => {
    if (!mounted.current) return;
    const value = cause && typeof cause === "object" ? cause as Partial<AppError> : {};
    setError({ code: value.code ?? "UNKNOWN", message: errorMessage(cause), field: value.field });
  }, []);
  const refresh = useCallback(async () => {
    const token = ++generation.current;
    const result = await imageGenerationApi.get();
    if (mounted.current && token === generation.current) setData(result);
  }, []);
  useEffect(() => {
    mounted.current = true; let disposed = false, stop: (() => void) | undefined;
    const reload = () => { if (!disposed) void refresh().catch(showError); };
    void imageGenerationApi.listen(reload).then(unsubscribe => { if (disposed) unsubscribe(); else { stop = unsubscribe; reload(); } }).catch(cause => { showError(cause); reload(); });
    window.addEventListener("focus", reload);
    return () => { disposed = true; mounted.current = false; generation.current++; stop?.(); window.removeEventListener("focus", reload); };
  }, [refresh, showError]);
  useEffect(() => { if (deleting) deleteDialog.current?.showModal(); else deleteDialog.current?.close(); }, [deleting]);
  useEffect(() => { if (enabling) enableDialog.current?.showModal(); else enableDialog.current?.close(); }, [enabling]);
  const run = async (action: () => Promise<ImageGenerationConfig>, message: string) => {
    if (locked.current) return false;
    locked.current = true; setBusy(true); setError(null); setNotice("");
    try {
      const result = await action();
      if (mounted.current) { generation.current++; setData(current => current && current.revision > result.revision ? current : result); setNotice(message); }
      return true;
    } catch (cause) { showError(cause); return false; }
    finally { locked.current = false; if (mounted.current) setBusy(false); }
  };
  const save = async (input: ImageGenerationProviderInput) => {
    if (!data) return;
    const ok = await run(() => imageGenerationApi.save(editing?.id ?? null, editRevision, input), "生图配置已保存；没有进行付费测试。");
    if (ok && mounted.current) setEditing(undefined);
  };
  const current = data?.providers.find(provider => provider.id === data.activeId);
  const disabled = busy || !data;
  const formOpen = editing !== undefined;
  return <section className="ym-page-card" aria-label="生图模型">
    <div className="ym-page-heading"><h2>生图模型</h2><button className="ym-button" disabled={disabled || formOpen} onClick={() => { setEditing(null); setEditRevision(data?.revision ?? 0); setError(null); setNotice(""); }}>新增生图提供商</button></div>
    <p className="ym-page-note">饰品图片只使用此处选中的提供商。未开启、未配置或生成失败时，仍显示图标、Emoji、颜文字或符号组合；已保存图片不受关闭开关影响。</p>
    <p className="ym-page-note">图片生成可能在准备预览时产生服务商 API 费用，即使没有购买饰品；喵币只在确认购买时扣除，两者独立。失败与超时不自动重试，取消不保证服务商退费。</p>
    {!data && !error && <p role="status">正在读取生图配置…</p>}
    {error && !formOpen && !deleting && !enabling && <div className="ym-page-error" role="alert">{error.message}<button className="ym-button" disabled={busy} onClick={() => { setError(null); void refresh().catch(showError); }}>重新读取</button></div>}
    {formOpen && error?.code === "CONFIG_CONFLICT" && <button className="ym-button" disabled={busy} onClick={() => { setEditing(undefined); setError(null); void refresh().catch(showError); }}>取消编辑并重新读取</button>}
    {notice && <p role="status">{notice}</p>}
    {data && <>
      <label className="ym-field">当前生图提供商<select disabled={disabled || formOpen} value={data.activeId ?? ""} onChange={event => { const id = event.target.value || null; void run(() => imageGenerationApi.select(id, data.revision), id ? "生图当前项已保存，未完成的旧图片请求已取消。" : "已清除当前生图项并关闭生图。"); }}>
        <option value="">不选择</option>{data.providers.map(provider => <option key={provider.id} value={provider.id}>{provider.name} · {provider.model}</option>)}
      </select></label>
      <label><input type="checkbox" aria-label="开启饰品生图" checked={data.enabled} disabled={disabled || formOpen || (!data.enabled && !current?.hasApiKey)} onChange={event => {
        if (event.target.checked) { setError(null); setEnableRevision(data.revision); setEnabling(true); }
        else void run(() => imageGenerationApi.enable(false, data.revision), "生图已关闭；未完成请求取消，已保存图片保留。");
      }} />开启饰品生图</label>
      {!data.enabled && !current?.hasApiKey && <p className="ym-page-note">先选择配置完整且已保存 Key 的提供商，才能开启。</p>}
      {!data.providers.length && <p className="ym-page-empty">暂无生图提供商，请按需新增。</p>}
      <ul className="ym-provider-list">{data.providers.map(provider => <li key={provider.id}>
        <div className="ym-page-heading"><h3>{provider.name}</h3>{provider.id === data.activeId && <span>生图当前项</span>}</div>
        <dl><dt>模型</dt><dd>{provider.model}</dd><dt>接口地址</dt><dd>{provider.baseUrl}</dd><dt>API Key</dt><dd>{provider.hasApiKey ? "已保存" : "未保存"}</dd></dl>
        <div className="ym-inline-form">
          <button className="ym-button" disabled={disabled || formOpen} onClick={() => { setEditing(provider); setEditRevision(data.revision); setError(null); setNotice(""); }} aria-label={`编辑生图 ${provider.name}`}>编辑</button>
          <button className="ym-button" disabled={disabled || formOpen} onClick={() => { setError(null); setNotice(provider.hasApiKey ? `${provider.name}：配置字段齐备。没有请求图片，也尚未验证模型的透明输出能力。` : `${provider.name}：尚未保存 Key。检查配置没有调用付费接口。`); }} aria-label={`检查生图配置 ${provider.name}`}>检查配置</button>
          <button className="ym-button ym-provider-delete" disabled={disabled || formOpen} onClick={() => { setDeleting(provider); setDeleteRevision(data.revision); setError(null); }} aria-label={`删除生图 ${provider.name}`}>删除</button>
        </div>
      </li>)}</ul>
    </>}
    {formOpen && <ProviderForm key={editing?.id ?? "new"} provider={editing ?? null} busy={busy} error={error} save={save} cancel={() => { setEditing(undefined); setError(null); }} />}
    <dialog ref={deleteDialog} className="ym-provider-dialog" aria-labelledby="image-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="image-delete-title">{deleting?.id === data?.activeId ? "当前生图项不能删除" : "删除生图配置"}</h2>
      <p>{deleting?.id === data?.activeId ? "请先切换到其他提供商，或把当前项设为“不选择”，再删除。" : `删除“${deleting?.name ?? ""}”及其 Key？已保存饰品图片保留。`}</p>
      {error && <p className="ym-page-error" role="alert">{error.message}</p>}
      <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>{deleting?.id === data?.activeId ? "知道了" : "取消"}</button>
        {deleting && data && deleting.id !== data.activeId && <button className="ym-button ym-provider-delete" disabled={busy} onClick={() => { void run(() => imageGenerationApi.delete(deleting.id, deleteRevision), "生图配置和 Key 已删除。").then(ok => { if (ok && mounted.current) setDeleting(null); }); }}>确认删除</button>}
      </div>
    </dialog>
    <dialog ref={enableDialog} className="ym-provider-dialog" aria-labelledby="image-enable-title" onCancel={event => { if (busy) event.preventDefault(); else setEnabling(false); }}>
      <h2 id="image-enable-title">开启饰品生图</h2>
      <p>AI 准备饰品预览时，会向当前生图提供商请求一次图片。服务商可能计费，即使你随后不购买；喵币不抵扣 API 费用。确认后开启，不会给旧饰品补生图。</p>
      {error && <p className="ym-page-error" role="alert">{error.message}</p>}
      <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setEnabling(false)}>取消</button><button className="ym-button" disabled={disabled} onClick={() => { if (data) void run(() => imageGenerationApi.enable(true, enableRevision), "生图已开启；后续预览可能产生服务商费用。").then(ok => { if (ok && mounted.current) setEnabling(false); }); }}>确认开启</button></div>
    </dialog>
  </section>;
}
