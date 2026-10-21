import { useEffect, useRef, useState } from "react";
import { companionApi, type Skill } from "../../companionApi";
import { configurationApi, errorMessage } from "../../api";
import { navigation } from "../../platform/navigation";
import type { Persona } from "../../types";
import { useCardBrowser } from "../../hooks/useCardBrowser";
import CardBrowserControls from "../CardBrowserControls";
import "./CompanionPanels.css";
export default function SkillPanel({ requestedId }: {
    requestedId?: string;
}) {
    const [items, setItems] = useState<Skill[]>([]), [people, setPeople] = useState<Persona[]>([]), [selected, setSelected] = useState<string | null>(requestedId ?? null), [draft, setDraft] = useState<{
        id: string | null;
        revision: number | null;
        title: string;
        summary: string;
        body: string;
    } | null>(null), [error, setError] = useState(""), [busy, setBusy] = useState(false), [deleting, setDeleting] = useState(false);
    const lock = useRef(false);
    const [personaFilter, setPersonaFilter] = useState("all");
    const reload = async () => { const [s, p] = await Promise.all([companionApi.skills(), configurationApi.listPersonas()]); setItems(s); setPeople(p); };
    useEffect(() => { void reload().catch(e => setError(errorMessage(e))); }, []);
    useEffect(() => {
        let alive=true;
        const stop=companionApi.listenSkills(()=>{if(alive)void reload().catch(e=>setError(errorMessage(e)));}).catch(e=>{if(alive)setError(errorMessage(e));return()=>{};});
        return()=>{alive=false;void stop.then(f=>f());};
    }, []);
    useEffect(() => { if (requestedId) {
        setSelected(requestedId);
        setDraft(null);
    } }, [requestedId]);
    const run = async (fn: () => Promise<unknown>) => { if (lock.current)
        return; lock.current = true; setBusy(true); setError(""); try {
        await fn();
        await reload().catch(e => setError(`操作已成功，暂时无法刷新列表：${errorMessage(e)}`));
        return true;
    }
    catch (e) {
        setError(errorMessage(e));
        return false;
    }
    finally {
        lock.current = false;
        setBusy(false);
    } };
    const peopleBrowser = useCardBrowser(people, p => p.name, p => p.updatedAt ?? "", p => p.id);
    const filtered = items.filter(s => personaFilter === "all" || (personaFilter === "unbound" ? s.personas.length === 0 : s.personas.some(p => p.id === personaFilter)));
    const browser = useCardBrowser(filtered, s => `${s.title} ${s.summary} ${s.body}`, s => s.updatedAt, s => s.id);
    const current = items.find(s => s.id === selected);
    return <section className="ym-page-card" aria-label="Skill仓库"><div className="ym-page-heading"><h2>Skill 仓库</h2><button className="ym-button" disabled={busy || !!draft} onClick={() => { setDraft({ id: null, revision: null, title: "", summary: "", body: "" }); setSelected(null); }}>新建 Skill</button></div><p className="ym-page-note">用于聊天、无聊主动消息、信件和游记。绑定多个人设的 Skill 修改后，会影响这些人设的后续请求。</p>
 {error && <p className="ym-page-error" role="alert">{error}</p>}
 {draft ? <form className="ym-skill-editor" onSubmit={e => { e.preventDefault(); void run(() => companionApi.saveSkill(draft)).then(ok => { if (ok)
            setDraft(null); }); }}><label>标题<input value={draft.title} onChange={e => setDraft({ ...draft, title: e.target.value })} required disabled={busy}/></label><label>简介<textarea value={draft.summary} onChange={e => setDraft({ ...draft, summary: e.target.value })} disabled={busy}/></label><label>Skill 正文<textarea rows={12} value={draft.body} onChange={e => setDraft({ ...draft, body: e.target.value })} required disabled={busy}/></label><div className="ym-inline-form"><button className="ym-button" disabled={busy || !draft.title.trim() || !draft.body.trim()}>保存</button><button className="ym-button" type="button" disabled={busy} onClick={() => setDraft(null)}>取消</button></div></form> :
            current ? <><button className="ym-button" onClick={() => { setSelected(null); setDeleting(false); }}>返回 Skill 列表</button><h3>{current.title}</h3><p>{current.summary}</p><div className="ym-skill-body">{current.body}</div><div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDraft({ id: current.id, revision: current.revision, title: current.title, summary: current.summary, body: current.body })}>编辑</button><button className="ym-button" data-variant="danger" onClick={() => setDeleting(true)}>删除</button></div>{deleting && <div role="alert"><p>删除会解除全部人设绑定，不删除人设。确认删除“{current.title}”？</p><button className="ym-button" disabled={busy} onClick={() => void run(() => companionApi.deleteSkill(current.id, current.revision)).then(ok => { if (ok) {
                setSelected(null);
                setDeleting(false);
            } })}>确认删除</button><button className="ym-button" disabled={busy} onClick={() => setDeleting(false)}>取消</button></div>}<h3>绑定人设</h3><CardBrowserControls browser={peopleBrowser} label="绑定人设" disabled={busy}/><div className="ym-companion-grid">{peopleBrowser.items.map(p => <article className="ym-page-card" key={p.id}><strong>{p.name}</strong><div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => void run(() => companionApi.bind(current.id, p.id, !current.personas.some(b => b.id === p.id)))}>{current.personas.some(b => b.id === p.id) ? "解除绑定" : "绑定"}</button><button className="ym-button" onClick={() => void navigation.openSettingsDetail("persona", p.id).catch(e => setError(errorMessage(e)))}>人设详情</button></div></article>)}</div></> :
                <><label className="ym-field">按人设筛选<select aria-label="按人设筛选Skill" value={personaFilter} onChange={e => { setPersonaFilter(e.target.value); browser.setPage(1); }}><option value="all">全部</option><option value="unbound">未绑定</option>{people.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label><CardBrowserControls browser={browser} label="Skill" timeLabel="更新时间"/><div className="ym-companion-grid">{browser.items.map(s => <button className="ym-object-card ym-skill-card" key={s.id} onClick={() => setSelected(s.id)}><strong>{s.title}</strong><p>{s.summary || "暂无简介"}</p><span className="ym-page-note">绑定 {s.personas.length} 个人设</span></button>)}</div>{!browser.total && <p className="ym-page-empty">{items.length ? "没有符合条件的 Skill。" : "写下可复用的表达方式或做事规则，再绑定到人设。"}</p>}</>}
 </section>;
}
