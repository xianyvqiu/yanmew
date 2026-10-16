import CardBrowserControls from "../CardBrowserControls";
import {useCardBrowser} from "../../hooks/useCardBrowser";
import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { configurationApi, errorMessage, userProfileApi } from "../../api";
import type { UserProfile } from "../../types";
import "./PersonasPanel.css";
import type { SettingsDetailRequest } from "../../platform/navigation";

type Draft = { id?: string; name: string; content: string };

export default function UserProfilePanel({ detailRequest }: { detailRequest?: SettingsDetailRequest }) {
  const [profiles, setProfiles] = useState<UserProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [draft, setDraft] = useState<Draft | null>(null);
  const [confirmation, setConfirmation] = useState<UserProfile | null>(null);
  const mounted = useRef(false);
  const lifetime = useRef(0);
  const generation = useRef(0);
  const locked = useRef(false);
  const pendingRefresh = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const consumedDetail = useRef<SettingsDetailRequest>();

  const refresh = useCallback(async () => {
    if (!mounted.current) return;
    if (locked.current) { pendingRefresh.current = true; return; }
    const token = ++generation.current;
    setLoading(true);
    try {
      const items = await userProfileApi.list();
      if (mounted.current && generation.current === token) setProfiles(items);
    } catch (cause) {
      if (mounted.current && generation.current === token) setError(errorMessage(cause));
    } finally {
      if (mounted.current && generation.current === token) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    lifetime.current++;
    let disposed = false;
    let stop: (() => void) | undefined;
    const reload = () => { if (!disposed) void refresh(); };
    void (async () => {
      try {
        const unsubscribe = await configurationApi.listenChanges(reload);
        if (disposed) { unsubscribe(); return; }
        stop = unsubscribe;
      } catch (cause) {
        if (!disposed) setError(errorMessage(cause));
      }
      reload();
    })();
    window.addEventListener("focus", reload);
    return () => {
      disposed = true;
      mounted.current = false;
      lifetime.current++;
      generation.current++;
      stop?.();
      window.removeEventListener("focus", reload);
    };
  }, [refresh]);

  useEffect(() => {
    const element = dialog.current;
    if (confirmation) element?.showModal(); else element?.close();
    return () => { element?.close(); };
  }, [confirmation]);

  const run = async <T,>(action: () => Promise<T>, success: (result: T) => void, message: string) => {
    if (locked.current || !mounted.current) return;
    const token = lifetime.current;
    locked.current = true;
    generation.current++;
    setLoading(false);
    setBusy(true);
    setError("");
    setNotice("");
    let done = false;
    try {
      const result = await action();
      if (mounted.current && lifetime.current === token) {
        success(result);
        setNotice(message);
        done = true;
      }
    } catch (cause) {
      if (mounted.current && lifetime.current === token) setError(errorMessage(cause));
    } finally {
      if (mounted.current && lifetime.current === token) {
        locked.current = false;
        setBusy(false);
        const reload = done || pendingRefresh.current;
        pendingRefresh.current = false;
        if (reload) void refresh();
      }
    }
  };

  const save = (event: FormEvent) => {
    event.preventDefault();
    if (!draft || locked.current) return;
    const name = draft.name.trim();
    const content = draft.content.trim();
    if (!name || !content) { setError(!name ? "名称不能为空" : "内容不能为空"); return; }
    const id = draft.id;
    void run(() => id ? userProfileApi.update({ id, name, content }) : userProfileApi.create({ name, content }),
      result => {
        setProfiles(previous => id ? previous.map(item => item.id === id ? result : item) : [...previous, result]);
        setDraft(null);
      }, id ? "用户设定已保存" : "用户设定已创建");
  };
  const remove = () => {
    if (!confirmation) return;
    const id = confirmation.id;
    void run(() => userProfileApi.delete(id), () => {
      setProfiles(previous => previous.filter(item => item.id !== id));
      setConfirmation(null);setSelected(null);
    }, "用户设定已删除，绑定它的会话已变为未绑定");
  };
  const cancel = () => { setDraft(null); setError(""); };
  const cancelDelete = () => { setConfirmation(null); setError(""); };
  const [selected,setSelected]=useState<string|null>(null);
  useEffect(() => {
    if (!detailRequest || consumedDetail.current === detailRequest || loading || busy || error || draft || confirmation) return;
    consumedDetail.current = detailRequest;
    if (profiles.some(item => item.id === detailRequest.id)) setSelected(detailRequest.id);
    else { setSelected(null); setError("该用户设定已不存在，请从列表重新选择"); }
  }, [detailRequest, loading, busy, error, draft, confirmation, profiles]);
  const browser=useCardBrowser(profiles,p=>`${p.name} ${p.content}`,p=>p.updatedAt,p=>p.id);
  const disabled = busy || loading;

  return <>
    <section className="ym-page-card" aria-label="用户设定管理">
      <div className="ym-page-heading"><h2>用户设定</h2><button className="ym-button" disabled={disabled || draft !== null || confirmation !== null} onClick={() => { setDraft({ name: "", content: "" }); setError(""); setNotice(""); }}>新建用户设定</button></div>
      <p className="ym-page-note">在此管理用户设定，可在聊天窗选择会话绑定的设定。</p>
      <p className="ym-page-note">编辑或删除设定前，会先中断使用它的会话正在生成的回复；普通回复保留已生成片段，重说保留原回复和记忆。</p>
      {loading && <p role="status">正在读取用户设定…</p>}
      {detailRequest && consumedDetail.current !== detailRequest && (busy || draft || confirmation) && <p role="status" className="ym-page-note">有未完成的操作，完成或取消后会打开目标用户设定详情；当前编辑内容保留。</p>}
      {notice && <p role="status">{notice}</p>}
      {error && !confirmation && !draft && <div className="ym-page-error" role="alert">{error}<button className="ym-button" disabled={disabled} onClick={() => { setError(""); void refresh(); }}>重新读取</button></div>}
      {draft && <form className="ym-persona-form" onSubmit={save}>
        <fieldset disabled={busy}>
          <legend>{draft.id ? "编辑用户设定" : "新建用户设定"}</legend>
          <label className="ym-field">名称<input value={draft.name} onChange={event => setDraft({ ...draft, name: event.target.value })} /></label>
          <label className="ym-field">内容<textarea rows={8} value={draft.content} onChange={event => setDraft({ ...draft, content: event.target.value })} /></label>
          {error && <p className="ym-page-error" role="alert">{error}</p>}
          <div className="ym-inline-form"><button className="ym-button" type="button" onClick={cancel}>取消</button><button className="ym-button" type="submit" disabled={disabled}>{busy ? "正在保存…" : "保存"}</button></div>
        </fieldset>
      </form>}
      {!loading && profiles.length === 0 && <p className="ym-page-note">暂无用户设定。</p>}
      {!selected&&<CardBrowserControls browser={browser} label="用户设定" timeLabel="更新时间" disabled={busy}/>}
      {selected&&<div className="ym-detail-header"><button className="ym-button" disabled={busy||draft!==null} onClick={()=>setSelected(null)}>返回用户设定列表</button><span>用户设定详情</span></div>}
      <div className="ym-persona-grid">{(selected?profiles.filter(p=>p.id===selected):browser.items).map(profile => <article className="ym-persona-card" key={profile.id} aria-label={profile.name} onClick={event=>{if(!selected&&!disabled&&!draft&&!(event.target as HTMLElement).closest("button"))setSelected(profile.id);}}>
        <h3><button className="ym-card-open" disabled={busy||draft!==null} onClick={()=>setSelected(profile.id)}>{profile.name}</button></h3>
        {!selected&&<p className="ym-card-summary">{profile.content}</p>}
        <p className="ym-page-note">更新时间：{new Date(profile.updatedAt).toLocaleString()}</p>
        {selected===profile.id&&<>
        <details><summary>内容</summary><p className="ym-persona-prompt">{profile.content}</p></details>
        <div className="ym-inline-form"><button className="ym-button" aria-label={`编辑 ${profile.name}`} disabled={disabled || draft !== null || confirmation !== null} onClick={() => { setDraft({ id: profile.id, name: profile.name, content: profile.content }); setError(""); setNotice(""); }}>编辑</button><button className="ym-button" data-variant="danger" aria-label={`删除 ${profile.name}`} disabled={disabled || draft !== null || confirmation !== null} onClick={() => { setConfirmation(profile); setError(""); setNotice(""); }}>删除</button></div>
        </>}
      </article>)}</div>
    </section>
    <dialog ref={dialog} className="ym-persona-dialog" aria-labelledby="user-profile-confirm-title" onCancel={event => { if (busy) event.preventDefault(); else cancelDelete(); }}>
      {confirmation && <><h2 id="user-profile-confirm-title">删除 {confirmation.name}</h2>
        <p>确定删除此用户设定？绑定它的会话不会被删除，只会变为未绑定；聊天与记忆保留。</p>
        {error && <p className="ym-page-error" role="alert">{error}</p>}
        <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={cancelDelete}>取消</button><button className="ym-button" data-variant="danger" disabled={busy} onClick={remove}>{busy ? "正在删除…" : "确认删除"}</button></div>
      </>}
    </dialog>
  </>;
}
