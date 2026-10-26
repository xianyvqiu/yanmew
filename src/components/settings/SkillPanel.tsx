import {useRevealForm} from "../../hooks/useRevealForm";
import { useCallback, useEffect, useRef, useState } from "react";
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
    } | null>(null), [error, setError] = useState(""), [saving, setBusy] = useState(false), [deleting, setDeleting] = useState(false);
    const [importing,setImporting]=useState(false),[importNote,setImportNote]=useState("");
    const busy=saving||importing;
    const form=useRevealForm(!!draft,draft?.id??undefined);
    const upload=useRef<HTMLInputElement>(null),reader=useRef<FileReader|null>(null),importGeneration=useRef(0),pendingImport=useRef(false);
    const cancelImport=()=>{importGeneration.current++;reader.current?.abort();reader.current=null;if(pendingImport.current)void companionApi.cancelSkillMarkdownImport().catch(()=>{});pendingImport.current=false;setImporting(false);setImportNote("已停止解析，文件原文仍在草稿中，可手动编辑后保存。");};
    useEffect(()=>()=>{importGeneration.current++;reader.current?.abort();if(pendingImport.current)void companionApi.cancelSkillMarkdownImport().catch(()=>{});pendingImport.current=false;},[]);
    const importMarkdown=async(file:File)=>{
        if(lock.current||pendingImport.current)return;
        setError("");setImportNote("");
        if(!/\.md$/i.test(file.name)){setError("请选择 .md 文件。");return;}
        if(file.size>1024*1024){setError("Markdown 文件不能超过 1 MB，请拆分后导入。");return;}
        const generation=++importGeneration.current;pendingImport.current=true;setImporting(true);
        try{
            const body=await new Promise<string>((resolve,reject)=>{const current=new FileReader();reader.current=current;current.onload=()=>resolve(String(current.result));current.onerror=()=>reject(new Error("文件读取失败"));current.onabort=()=>reject(new Error("文件读取已取消"));current.readAsText(file,"UTF-8");});
            if(generation!==importGeneration.current)return;
            reader.current=null;
            if(!body.trim()||body.includes("\uFFFD")||body.includes("\0"))throw new Error("文件为空或编码无效，请使用 UTF-8 Markdown 文件。");
            setSelected(null);setDraft({id:null,revision:null,title:file.name,summary:"",body});setImportNote("正在解析简介；文件名和正文保持原样，检查后保存。");
            const parsed=await companionApi.parseSkillMarkdown(file.name,body);
            if(generation!==importGeneration.current)return;
            setDraft({id:null,revision:null,title:file.name,summary:parsed.summary,body});setImportNote("已填入文件名、简介和完整正文，请检查后保存。保存后可绑定人设。");
        }catch(e){if(generation===importGeneration.current){setError(`导入解析未完成：${errorMessage(e)}`);setImportNote("若文件已读取，原文仍在草稿中，可手动填写简介后保存。");}}
        finally{if(generation===importGeneration.current){pendingImport.current=false;setImporting(false);}}
    };
    const lock = useRef(false);
    const [personaFilter, setPersonaFilter] = useState("all");
    const mounted=useRef(false),readGeneration=useRef(0);
    const reload = useCallback(async () => {
        const token=++readGeneration.current;
        try{const [s,p]=await Promise.all([companionApi.skills(),configurationApi.listPersonas()]);
            if(mounted.current&&token===readGeneration.current){setItems(s);setPeople(p);}
        }catch(cause){if(mounted.current&&token===readGeneration.current)throw cause;}
    },[]);
    useEffect(() => {
        let disposed=false,stop:(()=>void)|undefined;mounted.current=true;
        const refresh=()=>{if(!disposed)void reload().catch(e=>{if(!disposed)setError(errorMessage(e));});};
        void companionApi.listenSkills(refresh).then(fn=>{if(disposed)fn();else{stop=fn;refresh();}})
            .catch(e=>{if(!disposed){setError(errorMessage(e));refresh();}});
        window.addEventListener("focus",refresh);
        return()=>{disposed=true;mounted.current=false;readGeneration.current++;stop?.();window.removeEventListener("focus",refresh);};
    }, [reload]);
    useEffect(() => { if (requestedId) {
        if(pendingImport.current)cancelImport();
        setSelected(requestedId);
        setDraft(null);
    } }, [requestedId]);
    const run = async (fn: () => Promise<unknown>) => { if (lock.current||pendingImport.current)
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
    return <section className="ym-page-card" aria-label="Skill仓库"><div className="ym-page-heading"><h2>Skill 仓库</h2><div className="ym-inline-form"><button className="ym-button" disabled={busy || !!draft} onClick={() => { setImportNote("");setDraft({ id: null, revision: null, title: "", summary: "", body: "" }); setSelected(null); }}>新建 Skill</button><button className="ym-button" disabled={busy||!!draft} onClick={()=>upload.current?.click()}>上传 Markdown</button><input ref={upload} type="file" accept=".md,text/markdown" aria-label="上传 Markdown 文件" hidden disabled={busy||!!draft} onChange={e=>{const file=e.target.files?.[0];e.target.value="";if(file)void importMarkdown(file);}}/></div></div><p className="ym-page-note">用于聊天、无聊主动消息、信件和游记。上传 .md 会使用当前聊天模型解析简介，文件名和正文完整保留，检查后再保存。绑定多个人设的 Skill 修改后，会影响这些人设的后续请求。</p>
 <p className="ym-page-note">标题为 <code>SKILL_BASE</code>、<code>AGENTS</code>、<code>SKILL_BASE.md</code> 或 <code>AGENTS.md</code> 且已绑定人设的 Skill，AI 会每次必读其完整正文；标题区分大小写，不要求必须带 .md。其他绑定 Skill 由 AI 按需检索标题、简介，再读取正文。未绑定的基础项不会生效。</p>
 {importNote&&<p className="ym-page-note" role="status" aria-live="polite">{importNote}{importing&&<button type="button" className="ym-button" onClick={cancelImport}>停止解析</button>}</p>}
 {error && <p className="ym-page-error" role="alert">{error}</p>}
 {draft ? <form ref={form} className="ym-skill-editor" onSubmit={e => { e.preventDefault(); void run(() => companionApi.saveSkill(draft)).then(ok => { if (ok)
            setDraft(null); }); }}><label>标题<input value={draft.title} onChange={e => setDraft({ ...draft, title: e.target.value })} required disabled={busy}/></label><label>简介<textarea value={draft.summary} onChange={e => setDraft({ ...draft, summary: e.target.value })} disabled={busy}/></label><label>Skill 正文<textarea rows={12} value={draft.body} onChange={e => setDraft({ ...draft, body: e.target.value })} required disabled={busy}/></label><div className="ym-inline-form"><button className="ym-button" disabled={busy || !draft.title.trim() || !draft.body.trim()}>保存</button><button className="ym-button" type="button" disabled={busy} onClick={() => setDraft(null)}>取消</button></div></form> :
            current ? <><button className="ym-button" onClick={() => { setSelected(null); setDeleting(false); }}>返回 Skill 列表</button><h3>{current.title}</h3><p>{current.summary}</p><div className="ym-skill-body">{current.body}</div><div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => setDraft({ id: current.id, revision: current.revision, title: current.title, summary: current.summary, body: current.body })}>编辑</button><button className="ym-button" data-variant="danger" onClick={() => setDeleting(true)}>删除</button></div>{deleting && <div role="alert"><p>删除会解除全部人设绑定，不删除人设。确认删除“{current.title}”？</p><button className="ym-button" disabled={busy} onClick={() => void run(() => companionApi.deleteSkill(current.id, current.revision)).then(ok => { if (ok) {
                setSelected(null);
                setDeleting(false);
            } })}>确认删除</button><button className="ym-button" disabled={busy} onClick={() => setDeleting(false)}>取消</button></div>}<h3>绑定人设</h3><CardBrowserControls browser={peopleBrowser} label="绑定人设" disabled={busy}/><div className="ym-companion-grid">{peopleBrowser.items.map(p => <article className="ym-page-card" key={p.id}><strong>{p.name}</strong><div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => void run(() => companionApi.bind(current.id, p.id, !current.personas.some(b => b.id === p.id)))}>{current.personas.some(b => b.id === p.id) ? "解除绑定" : "绑定"}</button><button className="ym-button" onClick={() => void navigation.openSettingsDetail("persona", p.id).catch(e => setError(errorMessage(e)))}>人设详情</button></div></article>)}</div></> :
                <><label className="ym-field">按人设筛选<select aria-label="按人设筛选Skill" value={personaFilter} onChange={e => { setPersonaFilter(e.target.value); browser.setPage(1); }}><option value="all">全部</option><option value="unbound">未绑定</option>{people.map(p => <option key={p.id} value={p.id}>{p.name}</option>)}</select></label><CardBrowserControls browser={browser} label="Skill" timeLabel="更新时间"/><div className="ym-companion-grid">{browser.items.map(s => <button className="ym-object-card ym-skill-card" key={s.id} onClick={() => setSelected(s.id)}><strong>{s.title}</strong><p>{s.summary || "暂无简介"}</p><span className="ym-page-note">绑定 {s.personas.length} 个人设</span></button>)}</div>{!browser.total && <p className="ym-page-empty">{items.length ? "没有符合条件的 Skill。" : "写下可复用的表达方式或做事规则，再绑定到人设。"}</p>}</>}
 </section>;
}
