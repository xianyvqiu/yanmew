import { useEffect, useRef, useState } from "react";
import { petCreationApi, errorMessage } from "../../api";
import type { PetArtwork, PetArtworkInput, PetArtCategory, PetCreation } from "../../types";
import CardBrowserControls from "../CardBrowserControls";
import { useCardBrowser } from "../../hooks/useCardBrowser";
import "./PetCreationPanel.css";
const categories: [
    PetArtCategory,
    string
][] = [["normal", "日常"], ["standby", "待机"], ["click", "点击 / 摸摸"], ["hungry", "饥饿"], ["eating", "吃东西"], ["craving", "等待投喂"], ["craving_favorite", "期待最爱"], ["eating_favorite", "喜欢地品尝"], ["peek_left", "左侧探头"], ["peek_right", "右侧探头"], ["rest_sit", "底部坐下"], ["rest_lie", "底部趴下"], ["walk_left", "向左行走"], ["walk_right", "向右行走"], ["lifted", "被拎后颈"], ["error", "报错 / 超载"], ["toss_up", "往上抛"], ["toss_down", "往下掉"], ["landing", "落地"], ["wait_pet", "等摸摸"], ["sleepy", "睡觉"], ["bored", "无聊"], ["thinking", "思考"], ["chat", "聊天"]];
export function PetArtworkImage({ personaId, item, preview = false }: {
    personaId: string;
    item: PetArtwork;
    preview?: boolean;
}) {
    const [url, setUrl] = useState("");
    const [error, setError] = useState("");
    useEffect(() => { let alive = true; setUrl(""); setError(""); if (preview ? item.previewFile : item.imageFile)
        void petCreationApi.image(personaId, item.id, preview).then(url => { if (alive)
            setUrl(url); }).catch(e => { if (alive)
            setError(errorMessage(e)); }); return () => { alive = false; }; }, [personaId, item.id, item.imageFile, item.previewFile, preview]);
    return <div className="ym-pet-art-image">{url ? <img src={url} alt={`${item.name}${preview ? "预览" : "正式形象"}`} draggable={false}/> : <span>{error || "尚无图片"}</span>}</div>;
}
function ArtworkEditor({ item, personaId, busy, status, selected, onSelect, run, onBaseAccepted }: {
    item: PetArtwork;
    personaId: string;
    busy: boolean;
    status?: string;
    selected: boolean;
    onSelect: () => void;
    run: (job: () => Promise<unknown>, generating?: boolean) => Promise<void>;
    onBaseAccepted?:()=>void;
}) {
    const [draft, setDraft] = useState<PetArtworkInput>(item);
    const [editing, setEditing] = useState(false);
    const [editRevision, setEditRevision] = useState(item.revision);
    const [confirmDelete, setConfirmDelete] = useState(false);
    useEffect(() => { if (!editing)
        setDraft(item); }, [item, editing]);
    const upload = async (file: File) => { const data = await new Promise<string>((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = () => reject(new Error("图片读取失败")); reader.readAsDataURL(file); }); await petCreationApi.upload(personaId, item.id, item.revision, data); };
    return <article className="ym-page-card ym-pet-art-card">
  <div className="ym-page-heading"><h3>{item.name}</h3>{item.kind === "expression" && <label className="ym-art-selection"><input className="ym-selection-checkbox" type="checkbox" checked={selected} disabled={busy} onChange={onSelect}/>加入批量生成</label>}</div>
  <p className="ym-page-note">{item.kind === "base" ? "初始形象 · 默认显示" : `${categories.find(([key]) => key === item.category)?.[1]} · ${item.enabled ? "启用" : "停用"}`}</p>
  <div className="ym-art-previews"><div><small>已接受图片</small><PetArtworkImage personaId={personaId} item={item}/></div>{item.previewFile && <div><small>待确认预览</small><PetArtworkImage personaId={personaId} item={item} preview/></div>}</div>
  <p className="ym-card-summary">{item.description || "尚未填写使用说明"}</p>{status && <p role="status">{status}</p>}
  {editing ? <div className="ym-art-editor"><label className="ym-field">名称<input value={draft.name} maxLength={80} onChange={e => setDraft({ ...draft, name: e.target.value })}/></label>
   {item.kind === "expression" && <><label className="ym-field">生成要求<textarea value={draft.prompt} maxLength={8000} onChange={e => setDraft({ ...draft, prompt: e.target.value })}/></label><label className="ym-field">使用说明<textarea value={draft.description} maxLength={1000} onChange={e => setDraft({ ...draft, description: e.target.value })}/></label><label className="ym-field">适用类别<select value={draft.category} onChange={e => setDraft({ ...draft, category: e.target.value as PetArtCategory })}>{categories.map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></label><label className="ym-art-selection"><input type="checkbox" checked={draft.enabled} onChange={e => setDraft({ ...draft, enabled: e.target.checked })}/>允许使用</label></>}
   <div className="ym-inline-form"><button className="ym-button" disabled={busy || !draft.name.trim()} onClick={() => { void run(() => petCreationApi.save(personaId, item.id, editRevision, { kind: draft.kind, name: draft.name, prompt: draft.prompt, description: draft.description, category: draft.category, enabled: draft.enabled }).then(value => { setDraft(value); setEditing(false); })); }}>保存条目</button><button className="ym-button" disabled={busy} onClick={() => { setDraft(item); setEditing(false); }}>取消修改</button></div>
  </div> : <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => { setDraft(item); setEditRevision(item.revision); setEditing(true); }}>编辑要求与说明</button><label className="ym-button">上传透明图片<input type="file" accept="image/png,image/webp" disabled={busy} onChange={e => { const file = e.target.files?.[0]; e.target.value = ""; if (file)
            void run(() => upload(file)); }}/></label>
   {item.kind === "expression" && <button className="ym-button" disabled={busy || !item.prompt.trim()} onClick={() => { void run(() => petCreationApi.generate(personaId, item.id, item.revision), true); }}>{item.previewFile || item.imageFile ? "重新生成" : "生成此表情"}</button>}
   {item.previewFile && <><button className="ym-button" disabled={busy} onClick={() => { void run(async () => {await petCreationApi.finish(personaId, item.id, item.revision, "accept");if(item.kind==="base")onBaseAccepted?.();}); }}>接受预览</button><button className="ym-button" disabled={busy} onClick={() => { void run(() => petCreationApi.finish(personaId, item.id, item.revision, "discard")); }}>放弃预览</button></>}
   <button className="ym-button" data-variant="danger" disabled={busy} onClick={() => setConfirmDelete(true)}>删除条目</button>
  </div>}
  {confirmDelete && <div role="alert" className="ym-page-error"><p>{item.kind === "base" ? "删除初始形象会关闭图片模式，其他表情保留。" : "删除条目及其图片，此操作不可恢复。"}</p><button className="ym-button" disabled={busy} onClick={() => setConfirmDelete(false)}>取消</button><button className="ym-button" data-variant="danger" disabled={busy} onClick={() => { void run(() => petCreationApi.finish(personaId, item.id, item.revision, "delete")); }}>确认删除</button></div>}
 </article>;
}
export default function PetCreationPanel({ personaId, personaName, onStartSetup }: {
    personaId: string;
    personaName: string;
    onStartSetup?:()=>void;
}) {
    const [library, setLibrary] = useState<PetCreation | null>(null);
    const [error, setError] = useState("");
    const [busy, setBusy] = useState(false);
    const [generating, setGenerating] = useState(false);
    const [requirements, setRequirements] = useState("");
    const [selected, setSelected] = useState<string[]>([]);
    const [statuses, setStatuses] = useState<Record<string, string>>({});
    const alive = useRef(true);
    const locked = useRef(false);
    const stopped = useRef(false);
    const current = useRef<string | null>(null);
    const reload = async () => { const value = await petCreationApi.get(personaId); if (alive.current) {
        setLibrary(previous => previous && previous.revision > value.revision ? previous : value);
        setSelected(old => old.filter(id => value.items.some(item => item.id === id)));
    } return value; };
    useEffect(() => { alive.current = true; let stop: (() => void) | undefined; void reload().catch(e => { if (alive.current)
        setError(errorMessage(e)); }); void petCreationApi.listen(() => { if (!locked.current)
        void reload().catch(e => { if (alive.current)
            setError(errorMessage(e)); }); }).then(fn => { if (alive.current)
        stop = fn;
    else
        fn(); }).catch(e => { if (alive.current) setError(errorMessage(e)); }); return () => { alive.current = false; stopped.current = true; stop?.(); if (current.current)
        void petCreationApi.cancel(personaId, current.current).catch(() => { }); }; }, [personaId]);
    const run = async (job: () => Promise<unknown>, generating = false) => {
        if (locked.current) return;
        locked.current = true;
        setBusy(true); setGenerating(generating); setError("");
        let failed = false;
        try { await job(); }
        catch (error) { failed = true; if (alive.current) setError(errorMessage(error)); }
        finally {
            // 失败前可能已有成功写入，仍补查真实条目，保留操作本身的错误。
            try { await reload(); }
            catch (error) { if (alive.current && !failed) setError(errorMessage(error)); }
            locked.current = false; current.current = null;
            if (alive.current) { setBusy(false); setGenerating(false); }
        }
    };
    const expressions = library?.items.filter(a => a.kind === "expression") ?? [];
    const browser = useCardBrowser(expressions, a => `${a.name} ${a.prompt} ${a.description}`, a => a.createdAt, a => a.id);
    const base = library?.items.find(a => a.kind === "base");
    const standbyReady = expressions.some(a => a.category === "standby" && a.enabled && a.imageFile);
    const canEnable = Boolean(base?.imageFile && standbyReady);
    const editor = (item: PetArtwork) => <ArtworkEditor key={item.id} item={item} personaId={personaId} busy={busy} status={statuses[item.id]} onBaseAccepted={onStartSetup} selected={selected.includes(item.id)} onSelect={() => setSelected(old => old.includes(item.id) ? old.filter(id => id !== item.id) : [...old, item.id])} run={(job, generating) => { current.current = item.id; return run(job, generating); }}/>;
    const batch = () => run(async () => { stopped.current = false; for (const item of expressions.filter(a => selected.includes(a.id))) {
        if (stopped.current || !alive.current)
            break;
        current.current = item.id;
        setStatuses(old => ({ ...old, [item.id]: "正在生成…" }));
        try {
            await petCreationApi.generate(personaId, item.id, item.revision);
            if (alive.current)
                setStatuses(old => ({ ...old, [item.id]: "生成成功，等待接受" }));
        }
        catch (e) {
            if (alive.current)
                setStatuses(old => ({ ...old, [item.id]: errorMessage(e) }));
        }
    } }, true);
    return <div className="ym-pet-creation"><section className="ym-page-card"><div className="ym-page-heading"><h2>桌宠自定义创作</h2><label className="ym-art-selection"><input type="checkbox" role="switch" checked={library?.enabled ?? false} disabled={busy || !library || (!library.enabled && !canEnable)} onChange={e => { if (library)
        void run(() => petCreationApi.enable(personaId, e.target.checked, library.revision)); }}/>图片模式{library?.enabled ? "已开启" : "已关闭"}</label></div><p>当前人设：{personaName}。关闭后恢复颜文字模式，保留图片和表情库。</p><p className="ym-page-note">开启图片模式需要已接受的初始形象和至少一张允许使用的待机图。待机图用于无互动达到设置时间后的姿态，与日常默认形象分开。</p>{library && !standbyReady && <p role="status">尚缺可用待机图：请添加待机表情，上传或生成透明图片并接受预览。旧版已开启的图片模式保留，补图前待机暂用初始形象。</p>}<p className="ym-page-note">上传静态透明 PNG/WebP；图片和预览会保存。生图使用你在提供商中配置的服务，初始参考图会发送给该服务，可能产生 API 费用。服务须支持参考图编辑及透明输出；不会自动付费重试。</p>{error && <p role="alert" className="ym-page-error">{error}</p>}{busy && generating && <button className="ym-button" onClick={() => { stopped.current = true; if (current.current)
        void petCreationApi.cancel(personaId, current.current).catch(e => setError(errorMessage(e))); }}>停止生成</button>}
 {!expressions.some(a => a.category === "standby") && <button className="ym-button" disabled={busy || !library} onClick={() => { void run(() => petCreationApi.save(personaId, null, null, { kind: "expression", name: "待机姿态", prompt: "保持初始形象，安静等待陪伴的待机姿态，区别于睡觉和无聊，透明背景", description: "没有互动达到待机等待时间时使用", category: "standby", enabled: true })); }}>添加待机表情</button>}
 {onStartSetup&&<button className="ym-button" disabled={busy} onClick={onStartSetup}>上传初始图并生成基础表情</button>}
 {!base && <button className="ym-button" disabled={busy || !library} onClick={() => { void run(() => petCreationApi.save(personaId, null, null, { kind: "base", name: "初始形象", prompt: "", description: "日常默认形象", category: "normal", enabled: true })); }}>添加初始形象</button>}</section>
 {base && editor(base)}
 <section className="ym-page-card"><h3>新建表情要求</h3><label className="ym-field">每行一个要求，可一次添加多个<textarea rows={4} value={requirements} disabled={busy} placeholder={"开心地挥手\n抱着枕头睡觉\n委屈地看着我"} onChange={e => setRequirements(e.target.value)}/></label><div className="ym-inline-form"><button className="ym-button" disabled={busy || !library || !requirements.trim()} onClick={() => { void run(async () => { const lines = requirements.split(/\r?\n/).map(s => s.trim()).filter(Boolean); if (lines.some(line => [...line].length > 8000))
        throw new Error("每条生成要求最多8000字，请修改后再添加"); for (let index = 0; index < lines.length; index++) {
        const line = lines[index];
        const a = await petCreationApi.save(personaId, null, null, { kind: "expression", name: line.slice(0, 80), prompt: line, description: [...line].length <= 1000 ? line : "", category: "chat", enabled: true });
        if (alive.current) {
            setSelected(previous => [...previous.filter(id => id !== a.id), a.id]);
            setRequirements(lines.slice(index + 1).join("\n"));
        }
    } if (alive.current) {
        setRequirements("");
    } }); }}>添加要求</button><button className="ym-button" disabled={busy || !base?.imageFile || !expressions.some(a => selected.includes(a.id))} onClick={() => { void batch(); }}>生成选中表情 · {selected.length} 项</button></div></section>
 <CardBrowserControls browser={browser} label="图片表情" disabled={busy}/><div className="ym-object-grid">{browser.items.map(editor)}</div></div>;
}
