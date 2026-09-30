import { useCallback, useEffect, useRef, useState } from "react";
import { configurationApi, errorMessage } from "../../api";
import type { AppError, Persona, PersonaInput } from "../../types";
import PersonaForm from "./PersonaForm";
import "./PersonasPanel.css";

type Confirmation = { persona: Persona; action: "delete" | "baseline" | "restore" };
const inputFor = (persona: Persona): PersonaInput => ({ name: persona.name, prompt: persona.prompt,
  color: persona.color, selfEvolveEnabled: persona.selfEvolveEnabled, chatBgOpacity: persona.chatBgOpacity,
  chatBgBlur: persona.chatBgBlur, saveAsInitial: false });

function expressionGroups(persona: Persona): { label: string; values: string[] }[] {
  const library = persona.expressions;
  const click = library?.click && typeof library.click === "object" ? library.click as Record<string, unknown> : {};
  const groups: [string, unknown][] = [["点击 · 开心", click.happy], ["点击 · 惊讶", click.surprised], ["点击 · 害羞", click.shy],
    ["饥饿", library?.hungry], ["进食", library?.eating], ["困倦", library?.sleepy], ["无聊", library?.bored], ["思考", library?.thinking]];
  return groups.map(([label, value]) => ({ label, values: Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [] }));
}

export default function PersonasPanel() {
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
  const save = async (input: PersonaInput) => {
    const target = editing;
    if (await run(() => target ? configurationApi.updatePersona(target.id, input) : configurationApi.createPersona(input),
      target ? "人设已保存" : "人设已创建。有当前聊天提供商时，表情库会在后台自动生成。")) {
      if (mounted.current) setEditing(undefined);
    }
  };
  const confirm = async () => {
    if (!confirmation) return;
    const { persona, action } = confirmation;
    const done = await run(() => action === "delete" ? configurationApi.deletePersona(persona.id)
      : configurationApi.updatePersona(persona.id, { ...inputFor(persona),
        prompt: action === "restore" ? persona.initialPrompt : persona.prompt, saveAsInitial: action === "baseline" }),
      action === "delete" ? "人设及其关联数据已删除" : action === "restore" ? "已恢复初始设定" : "初始设定已更新");
    if (done && mounted.current) setConfirmation(null);
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
  const disabled = busy || loading;
  return <>
    <section className="ym-page-card" aria-label="人设管理">
      <div className="ym-page-heading"><h2>人设</h2><button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setEditing(null); setError(null); }}>新建人设</button></div>
      <p className="ym-page-note">切换会中断当前回复，并取消已开始的旅行和待回信；目标人设的会话由系统恢复。修改当前设定也会中断该人设正在进行的请求。</p>
      {loading && <p role="status">正在读取人设…</p>}
      {error && !confirmation && (!error.field || editing === undefined) && <div className="ym-page-error" role="alert">{error.message}<button className="ym-button" disabled={busy} onClick={() => { setError(null); void refresh().catch(showError); }}>重新读取</button></div>}
      {notice && <p role="status">{notice}</p>}
      {editing !== undefined && <PersonaForm key={editing?.id ?? "new"} persona={editing} busy={busy} error={error} onSave={save} onCancel={() => { setEditing(undefined); setError(null); }} />}
      <div className="ym-persona-grid">{personas.map(persona => <article className="ym-persona-card" key={persona.id} aria-label={persona.name}>
        <div className="ym-page-heading"><h3>{persona.name}</h3><span>{persona.isActive ? "当前使用" : ""}{persona.isBuiltin ? " · 系统默认" : ""}</span></div>
        <p className="ym-persona-color"><span style={{ backgroundColor: persona.color }} aria-hidden="true" />本体颜色 {persona.color}</p>
        <p className="ym-page-note">自动调整偏好：{persona.selfEvolveEnabled ? "开启" : "关闭"}（自动调整功能尚未开放）</p>
        <p className="ym-page-note">最近 AI 调整：{persona.lastEvolvedAt ? new Date(persona.lastEvolvedAt).toLocaleString() : "暂无"}</p>
        <details><summary>当前设定</summary><p className="ym-persona-prompt">{persona.prompt}</p></details>
        <details><summary>初始设定</summary><p className="ym-persona-prompt">{persona.initialPrompt}</p></details>
        <details><summary>表情库</summary>{persona.expressions ? <dl className="ym-expression-list">{expressionGroups(persona).map(group => <div key={group.label}><dt>{group.label}</dt><dd>{group.values.join("　") || "暂无"}</dd></div>)}</dl> : <p className="ym-page-note">尚无已保存的表情库。可以使用当前聊天提供商生成。</p>}</details>
        {failures[persona.id] && <p role="alert" className="ym-page-error">表情库生成未完成：{failures[persona.id]}。已有表情库保留。</p>}
        <div className="ym-inline-form">
          <button className="ym-button" disabled={disabled || persona.isActive || editing !== undefined} onClick={() => { void run(() => configurationApi.setPersona(persona.id), "当前人设已切换"); }}>切换为 {persona.name}</button>
          <button className="ym-button" aria-label={`编辑 ${persona.name}`} disabled={disabled || editing !== undefined} onClick={() => { setEditing(persona); setError(null); }}>编辑</button>
          <button className="ym-button" disabled={disabled || editing !== undefined} onClick={() => { setConfirmation({ persona, action: "baseline" }); setError(null); }}>保存为初始设定</button>
          <button className="ym-button" disabled={disabled || editing !== undefined || persona.prompt === persona.initialPrompt} onClick={() => { setConfirmation({ persona, action: "restore" }); setError(null); }}>从初始设定恢复</button>
          <button className="ym-button" aria-label={`重新生成 ${persona.name} 表情库`} disabled={disabled || editing !== undefined} onClick={() => { void generate(persona); }}>{generating === persona.id ? "正在生成表情库…" : "重新生成表情库"}</button>
          {generating === persona.id && <button className="ym-button" disabled={cancelling} onClick={() => { void cancelGeneration(); }}>{cancelling ? "正在取消…" : "取消生成"}</button>}
          {!persona.isBuiltin && <button className="ym-button ym-persona-danger" aria-label={`删除 ${persona.name}`} disabled={disabled || editing !== undefined} onClick={() => { setConfirmation({ persona, action: "delete" }); setError(null); }}>删除</button>}
        </div>
        {persona.isBuiltin && <p className="ym-page-note">系统默认人设不能删除。</p>}
      </article>)}</div>
    </section>
    <dialog ref={dialog} className="ym-persona-dialog" aria-labelledby="persona-confirm-title" onCancel={event => { if (busy) event.preventDefault(); else setConfirmation(null); }}>
      {confirmation && <><h2 id="persona-confirm-title">{confirmation.action === "delete" ? `删除 ${confirmation.persona.name}` : confirmation.action === "baseline" ? "更新初始设定" : "恢复初始设定"}</h2>
        <p>{confirmation.action === "delete" ? `这会删除该人设及其全部会话、聊天、记忆、记事本、出行、游记、信件、伴手礼、饰品和节日记录。${confirmation.persona.isActive ? "当前使用项会切回系统默认人设。" : ""}全局喵币、账单、提醒、设置和小游戏插件数据保留。`
          : confirmation.action === "baseline" ? "将当前已保存的设定替换初始设定，之后的差异比较会以新初始设定为准。" : "将当前设定替换为初始设定，聊天和记忆记录保留。"}</p>
        {error && <p className="ym-page-error" role="alert">{error.message}</p>}
        <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setConfirmation(null)}>取消</button><button className="ym-button" disabled={busy} onClick={() => { void confirm(); }}>{busy ? "正在处理…" : "确认"}</button></div></>}
    </dialog>
  </>;
}
