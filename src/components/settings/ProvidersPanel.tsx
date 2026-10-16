import { useCallback, useEffect, useRef, useState } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { AppError, Provider, ProviderInput } from "../../types";
import ProviderForm from "./ProviderForm";
import EmbeddingProvidersPanel from "./EmbeddingProvidersPanel";
import ProviderRolesPanel from "./ProviderRolesPanel";
import CardBrowserControls from "../CardBrowserControls";
import {useCardBrowser} from "../../hooks/useCardBrowser";
import "./ProvidersPanel.css";

export default function ProvidersPanel() {
  const [selected,setSelected]=useState<string|null>(null);
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
  const protectedProvider = deleting && (deleting.isActive || deleting.memoryIsActive || Boolean(deleting.purposes?.length));
  const browser=useCardBrowser(providers,p=>`${p.name} ${p.baseUrl??""}`,p=>p.createdAt??"",p=>p.id);
  const disabled = busy || loading;
  return <>
    <section className="ym-page-card" aria-label="通用模型提供商">
      <div className="ym-page-heading"><h2>通用模型提供商</h2>
        <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(null); setError(null); setNotice(""); }}>新增提供商</button></div>
      <p className="ym-page-note">统一保存服务连接与Key，下方按用途选择模型。当前支持OpenAI-compatible接口，Embedding和联网搜索单独管理。</p>
      {loading && <p role="status">正在读取提供商…</p>}
      {error && !deleting && (!error.field || editing === undefined) && <div className="ym-page-error" role="alert">{error.message}<button className="ym-button" disabled={busy} onClick={() => { setError(null); void refresh().catch(showError); }}>重新读取</button></div>}
      {notice && <p role="status">{notice}</p>}
      {loaded && !providers.length && <p className="ym-page-empty">暂无提供商，请先新增。</p>}
      {editing !== undefined && <ProviderForm connectionOnly key={editing?.id ?? "new"} provider={editing} busy={busy} error={error} onSave={save} onCancel={() => { setEditing(undefined); setError(null); }} />}
      {!selected&&<CardBrowserControls browser={browser} label="提供商" timeLabel="创建时间" disabled={busy}/>}
      {selected&&<button className="ym-button" disabled={busy||editing!==undefined} onClick={()=>setSelected(null)}>返回提供商列表</button>}
      <div className="ym-object-grid">{(selected?providers.filter(p=>p.id===selected):browser.items).map(provider=><article className="ym-object-card" key={provider.id} onClick={event=>{if(!selected&&!disabled&&editing===undefined&&!(event.target as HTMLElement).closest("button"))setSelected(provider.id);}}>
        <h3><button className="ym-card-open" disabled={disabled||editing!==undefined} onClick={()=>setSelected(provider.id)}>{provider.name}</button></h3><p className="ym-page-note ym-card-summary">{provider.baseUrl||"https://api.openai.com/v1"}</p>
        <p className="ym-page-note">Key：{provider.hasApiKey?"已保存":"未保存"} · {provider.purposes?.map(role=>({chat:"聊天",memory:"记忆",image:"生图",vision:"识图"})[role as "chat"]).join("、")||"尚未指定用途"}</p>
        <p className="ym-page-note">创建：{provider.createdAt?new Date(provider.createdAt).toLocaleString():"时间未知"}</p>
        {selected===provider.id&&<div className="ym-inline-form"><button className="ym-button" disabled={disabled||editing!==undefined} onClick={()=>{setEditing(provider);setError(null);setNotice("");}}>编辑连接</button><button className="ym-button" disabled={disabled||provider.model==="未配置"} title="使用该连接的聊天模型进行请求，可能消耗服务额度" onClick={()=>void test(provider)}>{testingId===provider.id?"正在测试…":"测试聊天连接"}</button><button className="ym-button" data-variant="danger" disabled={disabled} onClick={()=>{setDeleting(provider);setError(null);}}>删除连接</button></div>}
      </article>)}</div>
    </section>
    <ProviderRolesPanel providers={providers}/>
    <EmbeddingProvidersPanel />
    <WebSearchProvidersPanel />
    <dialog ref={dialog} className="ym-provider-dialog" aria-labelledby="provider-delete-title" onCancel={event => { if (busy) event.preventDefault(); else setDeleting(null); }}>
      <h2 id="provider-delete-title">{protectedProvider ? "此提供商正在使用" : "删除提供商"}</h2>
      <p>{protectedProvider ? "正在使用的提供商不能删除，请先更改引用它的聊天、记忆、生图或识图配置。" : `确定删除“${deleting?.name ?? ""}”的配置和密钥吗？已保存的聊天与记忆不会删除。`}</p>
      {deleting && error && <p role="alert" className="ym-page-error">{error.message}</p>}
      <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDeleting(null)}>{protectedProvider ? "知道了" : "取消"}</button>
        {!protectedProvider && <button className="ym-button ym-provider-delete" disabled={busy} onClick={() => { void remove(); }}>{busy ? "正在删除…" : "确认删除"}</button>}</div>
    </dialog>
  </>;
}
import WebSearchProvidersPanel from "./WebSearchProvidersPanel";
