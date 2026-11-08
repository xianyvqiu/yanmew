import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { petCreationApi, errorMessage } from "../../api";
import type { PetArtwork, PetArtworkInput, PetArtCategory, PetCreation } from "../../types";
import CardBrowserControls, {CardBrowserPagination} from "../CardBrowserControls";
import { useCardBrowser } from "../../hooks/useCardBrowser";
import "./PetCreationPanel.css";
type ArtworkStatus = { kind: "queued" | "generating" | "stopping" | "success" | "error" | "stopped"; text: string };
const emptyExpression = (): PetArtworkInput => ({ kind: "expression", name: "", prompt: "", description: "", category: "chat", enabled: true });
function ArtworkFields({ draft, onChange, disabled = false }: { draft: PetArtworkInput; onChange: (value: PetArtworkInput) => void; disabled?: boolean }) {
    return <div className="ym-art-fields">
        <label className="ym-field">表情名称<input required maxLength={80} disabled={disabled} value={draft.name} placeholder="例如：开心挥手" onChange={e => onChange({ ...draft, name: e.target.value })}/></label>
        <label className="ym-field">适用类别<select disabled={disabled} value={draft.category} onChange={e => onChange({ ...draft, category: e.target.value as PetArtCategory })}>{categories.map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select></label>
        <label className="ym-field ym-art-fields__wide">什么时候使用<textarea rows={2} maxLength={1000} disabled={disabled} value={draft.description} placeholder="例如：打招呼、欢迎用户回来时使用。供 AI 判断何时选择这张表情，可不填。" onChange={e => onChange({ ...draft, description: e.target.value })}/></label>
        <label className="ym-field ym-art-fields__wide">具体动作与画面要求<textarea required rows={4} maxLength={8000} disabled={disabled} value={draft.prompt} placeholder="例如：面向画面，抬起一只手轻轻挥动，微笑、眼神明亮；保持默认形象的服装和头身比例。" onChange={e => onChange({ ...draft, prompt: e.target.value })}/></label>
        <label className="ym-art-selection ym-art-fields__wide"><input type="checkbox" disabled={disabled} checked={draft.enabled} onChange={e => onChange({ ...draft, enabled: e.target.checked })}/>允许桌宠使用</label>
    </div>;
}
function ArtworkProgress({ status, onStop }: { status?: ArtworkStatus; onStop?: () => void }) {
    if (!status) return null;
    const active = status.kind === "generating" || status.kind === "stopping";
    return <div className="ym-art-progress" data-state={status.kind} role={status.kind === "error" ? "alert" : "status"} aria-live="polite">
        <div className="ym-art-progress__message">{active && <span className="ym-art-spinner" aria-hidden="true"/>}<span>{status.text}</span></div>
        {active && onStop && <button className="ym-button" disabled={status.kind === "stopping"} onClick={onStop}>{status.kind === "stopping" ? "正在停止…" : "停止生成"}</button>}
    </div>;
}
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
function ArtworkEditor({ item, base, personaId, busy, status, selected, onSelect, run, onBaseAccepted, onStop }: {
    item: PetArtwork;
    base?: PetArtwork;
    personaId: string;
    busy: boolean;
    status?: ArtworkStatus;
    onStop?: () => void;
    selected: boolean;
    onSelect: () => void;
    run: (job: () => Promise<unknown>, generating?: boolean) => Promise<unknown>;
    onBaseAccepted?:()=>void;
}) {
    const [draft, setDraft] = useState<PetArtworkInput>(item);
    const [opened, setOpened] = useState(false);
    const dialog = useRef<HTMLDialogElement>(null);
    useEffect(() => { if (opened) { if (dialog.current?.showModal) dialog.current.showModal(); else dialog.current?.setAttribute("open", ""); } }, [opened]);
    const [editing, setEditing] = useState(false);
    const [editRevision, setEditRevision] = useState(item.revision);
    const [confirmDelete, setConfirmDelete] = useState(false);
    useEffect(() => { if (!editing)
        setDraft(item); }, [item, editing]);
    const upload = async (file: File) => { const data = await new Promise<string>((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = () => reject(new Error("图片读取失败")); reader.readAsDataURL(file); }); await petCreationApi.upload(personaId, item.id, item.revision, data); };
    return <article className="ym-page-card ym-pet-art-card" aria-busy={status?.kind === "generating" || status?.kind === "stopping"}>
      <div className="ym-page-heading"><h3>{item.name}</h3></div>
      <p className="ym-page-note">{item.kind === "base" ? "初始形象 · 默认显示" : `${categories.find(([key]) => key === item.category)?.[1]} · ${item.enabled ? "启用" : "停用"}`}</p>
      <PetArtworkImage personaId={personaId} item={item} preview={Boolean(item.previewFile)}/>
      {item.previewFile && <small>待确认预览</small>}
      <p className="ym-card-summary">{item.description || "尚未填写使用说明"}</p>
      <ArtworkProgress status={status} onStop={onStop}/>
      <div className="ym-art-card-footer">{item.kind === "expression" && <label className="ym-art-selection"><input type="checkbox" checked={selected} disabled={busy} onChange={onSelect}/>加入批量生成</label>}
      <button className="ym-button" onClick={() => setOpened(true)}>查看与操作</button></div>
      {opened && createPortal(<dialog ref={dialog} className="ym-session-dialog ym-art-dialog" aria-label={`${item.name} · 表情操作`} onCancel={event => {event.preventDefault();setOpened(false);}}>
  <div className="ym-page-heading"><h2>{item.name}</h2></div>
  <div className="ym-session-dialog-body">
  <p className="ym-page-note">{item.kind === "base" ? "初始形象 · 默认显示" : `${categories.find(([key]) => key === item.category)?.[1]} · ${item.enabled ? "启用" : "停用"}`}</p>
  <div className="ym-art-previews">{base && item.kind === "expression" && <div><small>本次默认参考图</small><PetArtworkImage personaId={personaId} item={base}/></div>}<div><small>已接受图片</small><PetArtworkImage personaId={personaId} item={item}/></div>{item.previewFile && <div><small>待确认预览</small><PetArtworkImage personaId={personaId} item={item} preview/></div>}</div>
  <p className="ym-card-summary">{item.description || "尚未填写使用说明"}</p><ArtworkProgress status={status} onStop={onStop}/>
  {editing ? <div className="ym-art-editor">{item.kind === "expression" ? <ArtworkFields draft={draft} onChange={setDraft} disabled={busy}/> : <label className="ym-field">名称<input value={draft.name} maxLength={80} disabled={busy} onChange={e => setDraft({ ...draft, name: e.target.value })}/></label>}
   <div className="ym-inline-form"><button className="ym-button" disabled={busy || !draft.name.trim() || (item.kind === "expression" && !draft.prompt.trim())} onClick={() => { void run(() => petCreationApi.save(personaId, item.id, editRevision, { kind: draft.kind, name: draft.name, prompt: draft.prompt, description: draft.description, category: draft.category, enabled: draft.enabled }).then(value => { setDraft(value); setEditing(false); })); }}>保存条目</button><button className="ym-button" disabled={busy} onClick={() => { setDraft(item); setEditing(false); }}>取消修改</button></div>
  </div> : <div className="ym-inline-form"><button className="ym-button" disabled={busy} onClick={() => { setDraft(item); setEditRevision(item.revision); setEditing(true); }}>编辑要求与说明</button><label className="ym-button">上传透明图片<input type="file" accept="image/png,image/webp" disabled={busy} onChange={e => { const file = e.target.files?.[0]; e.target.value = ""; if (file)
            void run(() => upload(file)); }}/></label>
   {item.kind === "expression" && <button className="ym-button" disabled={busy || !item.prompt.trim()} onClick={() => { void run(() => petCreationApi.generate(personaId, item.id, item.revision), true); }}>{item.previewFile || item.imageFile ? "重新生成" : "生成此表情"}</button>}
   {item.previewFile && <><button className="ym-button" disabled={busy} onClick={() => { void run(async () => {await petCreationApi.finish(personaId, item.id, item.revision, "accept");if(item.kind==="base")onBaseAccepted?.();}); }}>接受预览</button><button className="ym-button" disabled={busy} onClick={() => { void run(() => petCreationApi.finish(personaId, item.id, item.revision, "discard")); }}>放弃预览</button></>}
   <button className="ym-button" data-variant="danger" disabled={busy} onClick={() => setConfirmDelete(true)}>删除条目</button>
  </div>}
  {confirmDelete && <div role="alert" className="ym-page-error"><p>{item.kind === "base" ? "删除初始形象会关闭图片模式，其他表情保留。" : "删除条目及其图片，此操作不可恢复。"}</p><button className="ym-button" disabled={busy} onClick={() => setConfirmDelete(false)}>取消</button><button className="ym-button" data-variant="danger" disabled={busy} onClick={() => { void run(() => petCreationApi.finish(personaId, item.id, item.revision, "delete")); }}>确认删除</button></div>}
  </div>
  <div className="ym-session-dialog-actions ym-art-dialog-actions"><button className="ym-button" aria-label="关闭表情操作" onClick={() => setOpened(false)}>关闭</button></div>
      </dialog>, document.body)}
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
    const [newExpression, setNewExpression] = useState<PetArtworkInput>(emptyExpression);
    const [creating, setCreating] = useState(false);
    const [selected, setSelected] = useState<string[]>([]);
    const [statuses, setStatuses] = useState<Record<string, ArtworkStatus>>({});
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
    const run = async (job: () => Promise<unknown>, generating = false, itemId?: string) => {
        if (locked.current) return false;
        locked.current = true;
        stopped.current = false;
        if (generating) current.current = itemId ?? null;
        if (!generating && itemId) setStatuses(old => { const next = { ...old }; delete next[itemId]; return next; });
        if (generating && itemId) setStatuses(old => ({ ...old, [itemId]: { kind: "generating", text: "正在生成：已附带默认参考图，等待服务出图并处理透明背景…" } }));
        setBusy(true); setGenerating(generating); setError("");
        let failed = false;
        try {
            await job();
            if (generating && itemId && alive.current) setStatuses(old => ({ ...old, [itemId]: { kind: "success", text: "生成完成，请对照默认图检查预览后接受。" } }));
        }
        catch (error) {
            failed = true;
            if (alive.current) {
                if (itemId) setStatuses(old => ({ ...old, [itemId]: { kind: stopped.current ? "stopped" : "error", text: errorMessage(error) } }));
                else setError(errorMessage(error));
            }
        }
        finally {
            // 失败前可能已有成功写入，仍补查真实条目，保留操作本身的错误。
            try { await reload(); }
            catch (error) { if (alive.current && !failed) { if (itemId) setStatuses(old => ({ ...old, [itemId]: { kind: "error", text: `操作已结束，但刷新卡片失败：${errorMessage(error)}；请重新打开页面核对。` } })); else setError(errorMessage(error)); } }
            locked.current = false; current.current = null;
            if (alive.current) { setBusy(false); setGenerating(false); }
        }
        return !failed;
    };
    const stopGeneration = () => {
        stopped.current = true;
        setStatuses(old => Object.fromEntries(Object.entries(old).map(([id, status]) => [id, status.kind === "queued" ? { kind: "stopped", text: "已停止，尚未请求生图服务。" } : status.kind === "generating" ? { kind: "stopping", text: "正在停止当前生成，请稍候…" } : status])));
        if (current.current) { const id = current.current; void petCreationApi.cancel(personaId, id).catch(e => { if (alive.current) setStatuses(old => ({ ...old, [id]: { kind: "error", text: `停止请求失败：${errorMessage(e)}；当前任务可能仍在处理。` } })); }); }
    };
    const expressions = library?.items.filter(a => a.kind === "expression") ?? [];
    const browser = useCardBrowser(expressions, a => `${a.name} ${a.prompt} ${a.description}`, a => a.createdAt, a => a.id);
    const base = library?.items.find(a => a.kind === "base");
    const standbyReady = expressions.some(a => a.category === "standby" && a.enabled && a.imageFile);
    const canEnable = Boolean(base?.imageFile && standbyReady);
    const editor = (item: PetArtwork) => <ArtworkEditor key={item.id} item={item} base={base} personaId={personaId} busy={busy} status={statuses[item.id]} onBaseAccepted={onStartSetup} onStop={stopGeneration} selected={selected.includes(item.id)} onSelect={() => setSelected(old => old.includes(item.id) ? old.filter(id => id !== item.id) : [...old, item.id])} run={(job, generating) => run(job, generating, item.id)}/>;
    const batch = () => {
        if (locked.current) return;
        const items = expressions.filter(a => selected.includes(a.id));
        setStatuses(old => ({ ...old, ...Object.fromEntries(items.map(a => [a.id, { kind: "queued", text: "等待生成…" } as ArtworkStatus])) }));
        return run(async () => {
            for (const item of items) {
                if (stopped.current || !alive.current) break;
                current.current = item.id;
                setStatuses(old => ({ ...old, [item.id]: { kind: "generating", text: "正在生成：已附带默认参考图，等待服务出图并处理透明背景…" } }));
                try {
                    const saved = await petCreationApi.generate(personaId, item.id, item.revision);
                    if (alive.current) {
                        setLibrary(old => old && ({ ...old, items: old.items.map(a => a.id === saved.id ? saved : a) }));
                        setStatuses(old => ({ ...old, [item.id]: { kind: "success", text: "生成完成，请对照默认图检查预览后接受。" } }));
                    }
                } catch (e) {
                    if (alive.current) setStatuses(old => ({ ...old, [item.id]: { kind: stopped.current ? "stopped" : "error", text: errorMessage(e) } }));
                }
            }
        }, true);
    };
    return <div className="ym-pet-creation"><section className="ym-page-card"><div className="ym-page-heading"><h2>桌宠自定义创作</h2><label className="ym-art-selection"><input type="checkbox" role="switch" checked={library?.enabled ?? false} disabled={busy || !library || (!library.enabled && !canEnable)} onChange={e => { if (library)
        void run(() => petCreationApi.enable(personaId, e.target.checked, library.revision)); }}/>图片模式{library?.enabled ? "已开启" : "已关闭"}</label></div><p>当前人设：{personaName}。关闭后恢复颜文字模式，保留图片和表情库。</p><p className="ym-page-note">开启图片模式需要已接受的初始形象和至少一张允许使用的待机图。待机图用于无互动达到设置时间后的姿态，与日常默认形象分开。</p>{library && !standbyReady && <p role="status">尚缺可用待机图：请添加待机表情，上传或生成透明图片并接受预览。旧版已开启的图片模式保留，补图前待机暂用初始形象。</p>}<p className="ym-page-note">上传静态透明 PNG/WebP；图片和预览会保存。生图使用你在提供商中配置的服务，初始参考图会发送给该服务，可能产生 API 费用。服务须支持参考图编辑；生成结果不透明时会尝试本地抠图，不会自动付费重试。</p>{error && <p role="alert" className="ym-page-error">{error}</p>}
 {!expressions.some(a => a.category === "standby") && <button className="ym-button" disabled={busy || !library} onClick={() => { void run(() => petCreationApi.save(personaId, null, null, { kind: "expression", name: "待机姿态", prompt: "保持初始形象，安静等待陪伴的待机姿态，区别于睡觉和无聊，透明背景", description: "没有互动达到待机等待时间时使用", category: "standby", enabled: true })); }}>添加待机表情</button>}
 {onStartSetup&&<button className="ym-button" disabled={busy} onClick={onStartSetup}>上传初始图并生成基础表情</button>}
 {!base && <button className="ym-button" disabled={busy || !library} onClick={() => { void run(() => petCreationApi.save(personaId, null, null, { kind: "base", name: "初始形象", prompt: "", description: "日常默认形象", category: "normal", enabled: true })); }}>添加初始形象</button>}</section>
 {base && editor(base)}
 <div><button type="button" className="ym-button" disabled={busy} aria-expanded={creating} onClick={() => setCreating(value => !value)}>{creating ? "收起创建表单" : "创建新表情"}</button></div>
 {creating && <section className="ym-page-card ym-art-create"><h3>新建表情要求</h3><p className="ym-page-note">每张表情分别填写。生成时会自动参考当前人设和已接受的默认形象，先预览再决定是否使用。</p>
 <form onSubmit={e => { e.preventDefault(); if (!newExpression.name.trim() || !newExpression.prompt.trim()) return; void run(async () => {
     const saved = await petCreationApi.save(personaId, null, null, newExpression);
     if (alive.current) { setSelected(old => [...old.filter(id => id !== saved.id), saved.id]); setNewExpression(emptyExpression()); setCreating(false); browser.setSearch(""); browser.setOrder("newest"); }
 }, false, "new"); }}>
 <ArtworkFields draft={newExpression} onChange={setNewExpression} disabled={busy}/><ArtworkProgress status={statuses.new}/>
 <div className="ym-inline-form"><button type="submit" className="ym-button" disabled={busy || !library || !newExpression.name.trim() || !newExpression.prompt.trim()}>添加表情</button></div></form></section>}
 <div className="ym-art-library-heading"><h3>图片表情库</h3><div className="ym-inline-form"><button className="ym-button" disabled={busy || !base?.imageFile || !expressions.some(a => selected.includes(a.id))} onClick={() => { void batch(); }}>生成选中表情 · {selected.length} 项</button>{busy && generating && <button className="ym-button" onClick={stopGeneration}>停止全部生成</button>}</div></div>
 <CardBrowserControls browser={browser} label="图片表情" disabled={busy}/><div className="ym-object-grid ym-art-library-grid">{browser.items.map(editor)}</div>
      <CardBrowserPagination browser={browser} label="图片表情" disabled={busy}/></div>;
}
