import MotionDetails from "../MotionDetails";
import PetCreationPanel from "./PetCreationPanel";
import PetImageSetup from "./PetImageSetup";
import PersonaExpressionsEditor from "./PersonaExpressionsEditor";
import StickerBindings from "./StickerBindings";
import PersonaCompanionSettings from "./PersonaCompanionSettings";
import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { AppError, Persona, PersonaInput } from "../../types";
import PersonaForm from "./PersonaForm";
import PersonaPromptDiff from "../PersonaPromptDiff";
import ChatBackgroundSettings from "../ChatBackgroundSettings";
import "./PersonasPanel.css";
import CardBrowserControls, {CardBrowserPagination} from "../CardBrowserControls";
import {useCardBrowser} from "../../hooks/useCardBrowser";
import type { SettingsDetailRequest } from "../../platform/navigation";

type Confirmation = { persona: Persona; action: "delete" | "baseline" | "restore" };
const inputFor = (persona: Persona): PersonaInput => ({ name: persona.name, prompt: persona.prompt,
  color: persona.color, selfEvolveEnabled: persona.selfEvolveEnabled, chatBgOpacity: persona.chatBgOpacity,
  chatBgBlur: persona.chatBgBlur, saveAsInitial: false });

export default function PersonasPanel({ detailRequest }: { detailRequest?: SettingsDetailRequest }) {
  const [imageSetup,setImageSetup]=useState<Persona|null>(null);
  const [selected,setSelected]=useState<string|null>(null);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [notice, setNotice] = useState("");
  const [editing, setEditing] = useState<Persona | null | undefined>(undefined);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [generating, setGenerating] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState(false);
  const [failures, setFailures] = useState<Record<string, string>>({});
  const mounted = useRef(false);
  const generation = useRef(0);
  const locked = useRef(false);
  const generatingRef = useRef<string | null>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const consumedDetail = useRef<SettingsDetailRequest>();
  const showError = useCallback((cause: unknown) => {
    if (!mounted.current) return;
    const details = cause && typeof cause === "object" ? cause as Partial<AppError> : {};
    setError({ code: details.code ?? "UNKNOWN", message: errorMessage(cause), field: details.field });
  }, []);
  const refresh = useCallback(async () => {
    const token = ++generation.current;
    setLoading(true);
    try {
      const items = await configurationApi.listPersonas();
      if (mounted.current && generation.current === token) setPersonas(items);
    } finally { if (mounted.current && generation.current === token) setLoading(false); }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    const stops: (() => void)[] = [];
    const reload = () => { if (!disposed) void refresh().catch(showError); };
    void (async () => {
      try {
        for (const subscribe of [
          () => configurationApi.listenChanges(reload),
          () => configurationApi.listenContext(reload),
          () => configurationApi.listenExpressionFailure(({ personaId, error: failure }) => {
            if (!disposed) setFailures(previous => ({ ...previous, [personaId]: failure.message }));
          }),
        ]) {
          const stop = await subscribe();
          if (disposed) { stop(); return; }
          stops.push(stop);
        }
      } catch (cause) { if (!disposed) showError(cause); }
      reload();
    })();
    window.addEventListener("focus", reload);
    return () => {
      disposed = true; mounted.current = false; generation.current++;
      stops.forEach(stop => stop()); window.removeEventListener("focus", reload);
      if (generatingRef.current) void configurationApi.cancelExpressionGeneration(generatingRef.current).catch(() => {});
    };
  }, [refresh, showError]);
  useEffect(() => { if (confirmation) dialog.current?.showModal(); else dialog.current?.close(); }, [confirmation]);
  useEffect(() => {
    if (!detailRequest || consumedDetail.current === detailRequest || loading || busy || error || generating || editing !== undefined || confirmation) return;
    consumedDetail.current = detailRequest;
    if (personas.some(item => item.id === detailRequest.id)) setSelected(detailRequest.id);
    else { setSelected(null); setError({ code: "NOT_FOUND", message: "该人设已不存在，请从列表重新选择" }); }
  }, [detailRequest, loading, busy, error, generating, editing, confirmation, personas]);

  const run = async (action: () => Promise<unknown>, success: string): Promise<boolean> => {
    if (locked.current) return false;
    locked.current = true; setBusy(true); setError(null); setNotice("");
    try {
      await action();
      if (mounted.current) { setNotice(success); await refresh().catch(showError); }
      return true;
    } catch (cause) { showError(cause); return false; }
    finally { locked.current = false; if (mounted.current) setBusy(false); }
  };
  const save = async (input: PersonaInput, imageMode=false) => {
    const target = editing;let created:Persona|undefined;
    if (await run(async()=>{if(target)await configurationApi.updatePersona(target.id,input);else created=await configurationApi.createPersona(input);},
      target ? "人设已保存" : imageMode ? "人设已创建，请上传初始图片。" : "人设与默认会话已创建。有当前聊天提供商时，颜文字表情库会在后台自动生成。")) {
      if (mounted.current) {setEditing(undefined);if(imageMode&&created){setSelected(created.id);setImageSetup(created);}}
    }
  };
  const confirm = async () => {
    if (!confirmation) return;
    const { persona, action } = confirmation;
    const done = await run(() => action === "delete" ? configurationApi.deletePersona(persona.id)
      : configurationApi.updatePersona(persona.id, { ...inputFor(persona),
        prompt: action === "restore" ? persona.initialPrompt : persona.prompt, saveAsInitial: action === "baseline" }),
      action === "delete" ? "人设及其关联数据已删除" : action === "restore" ? "已恢复初始设定" : "初始设定已更新");
    if (done && mounted.current) {setConfirmation(null);if(action==="delete")setSelected(null);}
  };
  const generate = async (persona: Persona) => {
    if (locked.current) return;
    generatingRef.current = persona.id; setGenerating(persona.id); setCancelling(false);
    setFailures(previous => { const next = { ...previous }; delete next[persona.id]; return next; });
    await run(() => configurationApi.generateExpressions(persona.id), "表情库已生成并保存");
    generatingRef.current = null;
    if (mounted.current) { setGenerating(null); setCancelling(false); }
  };
  const cancelGeneration = async () => {
    const id = generatingRef.current;
    if (!id || cancelling) return;
    setCancelling(true);
    try { await configurationApi.cancelExpressionGeneration(id); }
    catch (cause) { showError(cause); if (mounted.current) setCancelling(false); }
  };
  const browser=useCardBrowser(personas,p=>`${p.name} ${p.prompt} ${p.initialPrompt}`,p=>p.updatedAt??"",p=>p.id);
  const disabled = busy || loading;
  return <>
    {imageSetup&&<PetImageSetup key={imageSetup.id} personaId={imageSetup.id} personaName={imageSetup.name} onClose={()=>{setImageSetup(null);void refresh().catch(showError);}}/>}
    <section className="ym-page-card" aria-label="人设管理">
      <div className="ym-page-heading"><h2>人设</h2><button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(null); setError(null); }}>新建人设</button></div>
      <p className="ym-page-note">切换会中断当前回复，并取消已开始的旅行和待回信；目标人设的会话由系统恢复。修改当前设定也会中断该人设正在进行的请求。</p>
      {loading && <p role="status">正在读取人设…</p>}
      {detailRequest && consumedDetail.current !== detailRequest && (busy || generating || editing !== undefined || confirmation) && <p role="status" className="ym-page-note">有未完成的操作，完成或取消后会打开目标人设详情；当前编辑内容保留。</p>}
      {error && !confirmation && (!error.field || editing === undefined) && <div className="ym-page-error" role="alert">{error.message}<button className="ym-button" disabled={busy} onClick={() => { setError(null); void refresh().catch(showError); }}>重新读取</button></div>}
      {notice && <p role="status">{notice}</p>}
      {editing !== undefined && <PersonaForm key={editing?.id ?? "new"} persona={editing ? personas.find(persona => persona.id === editing.id) ?? editing : editing} busy={busy} error={error} onSave={save} onCancel={() => { setEditing(undefined); setError(null); }} />}
      {!selected&&<CardBrowserControls browser={browser} label="人设" timeLabel="更新时间" disabled={busy}/>}
      {selected&&<div className="ym-detail-header"><button className="ym-button" disabled={busy||editing!==undefined} onClick={()=>setSelected(null)}>返回人设列表</button><span>人设详情</span></div>}
      <div className="ym-persona-grid">{(selected?personas.filter(p=>p.id===selected):browser.items).map(persona => <article className="ym-persona-card ym-object-card" style={{"--ym-persona-mark":/^#[\da-f]{6}$/i.test(persona.color)?persona.color:"#ffb6c1"} as CSSProperties} data-current={persona.isActive} key={persona.id} aria-label={persona.name} onClick={event=>{if(!selected&&editing===undefined&&!disabled&&!(event.target as HTMLElement).closest("button"))setSelected(persona.id);}}>
        <div className="ym-page-heading"><h3><button className="ym-card-open" disabled={disabled||editing!==undefined} onClick={()=>setSelected(persona.id)}>{persona.name}</button></h3><button className={`ym-button ${persona.isActive?"ym-current-button":""}`} disabled={disabled||persona.isActive||editing!==undefined} onClick={()=>void run(()=>configurationApi.setPersona(persona.id),"当前人设已切换")}>{persona.isActive?"使用中":"切换"}</button></div>
        <p className="ym-page-note">{persona.isBuiltin?"系统默认人设":"自建人设"}{persona.isActive&&" · 当前使用"}</p>
        {!selected&&<p className="ym-card-summary">{persona.prompt.length > 140 ? persona.prompt.slice(0,140) + "…" : persona.prompt}</p>}
        <p className="ym-persona-color"><span style={{ backgroundColor: persona.color }} aria-hidden="true" />人设标记颜色 {persona.color}</p>
        {!selected&&<p className="ym-page-note">人设自调：{persona.selfEvolveEnabled ? "开启" : "关闭"}</p>}
        {selected===persona.id&&<>
        <p className="ym-page-note">人设自调：{persona.selfEvolveEnabled ? "开启" : "关闭"}。仅在确有必要时小幅调整，完整回复成功后保存。</p>
        <p className="ym-page-note">最近 AI 调整：{persona.lastEvolvedAt ? new Date(persona.lastEvolvedAt).toLocaleString() : "暂无"}</p>
        <details><summary>当前设定</summary><p className="ym-persona-prompt">{persona.prompt}</p></details>
        <details><summary>初始设定</summary><p className="ym-persona-prompt">{persona.initialPrompt}</p></details>
        <PersonaPromptDiff initialPrompt={persona.initialPrompt} currentPrompt={persona.prompt} />
        <ChatBackgroundSettings persona={persona} disabled={disabled || editing !== undefined || Boolean(confirmation)} onSaved={() => { void refresh().catch(showError); }} />
        <PersonaCompanionSettings key={persona.id} personaId={persona.id}/>
        <StickerBindings key={`stickers:${persona.id}`} personaId={persona.id}/>
        <MotionDetails className="ym-feature-module"><summary><span>表情库</span><small>颜文字与图片互动动作</small></summary><PersonaExpressionsEditor key={persona.id} persona={persona} disabled={disabled || editing !== undefined || Boolean(generating)} onSaved={() => { void refresh().catch(showError); }} /><PetCreationPanel personaId={persona.id} personaName={persona.name} onStartSetup={()=>setImageSetup(persona)}/><div className="ym-module-actions">          <button className="ym-button" aria-label={`重新生成 ${persona.name} 表情库`} disabled={disabled || editing !== undefined} onClick={() => { void generate(persona); }}>{generating === persona.id ? "正在生成表情库…" : "重新生成表情库"}</button>
          {generating === persona.id && <button className="ym-button" disabled={cancelling} onClick={() => { void cancelGeneration(); }}>{cancelling ? "正在取消…" : "取消生成"}</button>}</div></MotionDetails>
        {failures[persona.id] && <p role="alert" className="ym-page-error">表情库生成未完成：{failures[persona.id]}。已有表情库保留。</p>}
        <h3 className="ym-section-heading">人设管理</h3><div className="ym-inline-form ym-persona-management">
          <button className="ym-button" disabled={disabled || persona.isActive || editing !== undefined} onClick={() => { void run(() => configurationApi.setPersona(persona.id), "当前人设已切换"); }}>切换为 {persona.name}</button>
          <button className="ym-button" aria-label={`编辑 ${persona.name}`} disabled={disabled || editing !== undefined} onClick={() => { setEditing(persona); setError(null); }}>编辑</button>
          <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setConfirmation({ persona, action: "baseline" }); setError(null); }}>保存为初始设定</button>
          <button className="ym-button" disabled={disabled || editing !== undefined || persona.prompt === persona.initialPrompt} onClick={() => { setConfirmation({ persona, action: "restore" }); setError(null); }}>从初始设定恢复</button>

          {!persona.isBuiltin && <button className="ym-button ym-persona-danger" aria-label={`删除 ${persona.name}`} disabled={disabled || editing !== undefined} onClick={() => { setConfirmation({ persona, action: "delete" }); setError(null); }}>删除</button>}
        </div>
        {persona.isBuiltin && <p className="ym-page-note">系统默认人设不能删除。</p>}
        </>}
      </article>)}</div>
      {!selected&&<CardBrowserPagination browser={browser} label="人设" disabled={busy}/>}
    </section>
    <dialog ref={dialog} className="ym-persona-dialog" aria-labelledby="persona-confirm-title" onCancel={event => { if (busy) event.preventDefault(); else setConfirmation(null); }}>
      {confirmation && <><h2 id="persona-confirm-title">{confirmation.action === "delete" ? `删除 ${confirmation.persona.name}` : confirmation.action === "baseline" ? "更新初始设定" : "恢复初始设定"}</h2>
        <p>{confirmation.action === "delete" ? `这会删除该人设及其全部会话、聊天、记忆、记事本、出行、游记、信件、伴手礼和饰品。${confirmation.persona.isActive ? "当前使用项会切回系统默认人设。" : ""}全局自定义日期、喵币、账单、提醒、设置和小游戏插件数据保留。`
          : confirmation.action === "baseline" ? "将当前已保存的设定替换初始设定，之后的差异比较会以新初始设定为准，当前累计差异会清空。" : "将当前设定替换为初始设定，当前累计差异会清空，聊天和记忆记录保留。"}</p>
        {error && <p className="ym-page-error" role="alert">{error.message}</p>}
        <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setConfirmation(null)}>取消</button><button className="ym-button" disabled={busy} onClick={() => { void confirm(); }}>{busy ? "正在处理…" : "确认"}</button></div></>}
    </dialog>
  </>;
}
