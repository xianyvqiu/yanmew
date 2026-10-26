import { useEffect, useRef, useState } from "react";
import { configurationApi, errorMessage, memoryManagementApi } from "../../api";
import type { CurrentContext, MemoryDetail, MemoryKind, MemoryPreview, MemoryRecord, Settings } from "../../types";
import NotebookPanel from "./NotebookPanel";
import InlineDetailCard from "../InlineDetailCard";
const PAGE_SIZE = 12;
import "./MemoryManagementPanel.css";

type Tab = "notebook" | MemoryKind;
type Target = { id: string; request: number };
type Props = { context: CurrentContext };

function validText(value: string) {
  return value.trim().length > 0;
}

function isConflict(cause: unknown) {
  return Boolean(cause && typeof cause === "object" && "code" in cause &&
    ["MEMORY_CONFLICT", "MEMORY_PREVIEW_EXPIRED"].includes(String(cause.code)));
}

export default function MemoryManagementPanel({ context }: Props) {
  const key = `${context.personaId}:${context.sessionId ?? "none"}:${context.contextRevision}`;
  return <MemoryManagementForContext key={key} context={context} />;
}

function MemoryManagementForContext(props: Props) {
  // 同一上下文凭据保持稳定，窗口补查产生的新对象不取消现有编辑或预览。
  const [context] = useState(props.context);
  const [tab, setTab] = useState<Tab>("notebook");
  const [target, setTarget] = useState<Target | null>(null);
  const [previewOwner, setPreviewOwner] = useState<MemoryKind | null>(null);
  const targetCounter = useRef(0);

  useEffect(() => () => { if (context.sessionId) void memoryManagementApi.cancel(context).catch(() => {}); }, [context]);

  const locateLong = (id: string) => {
    setTab("long");
    setTarget({ id, request: ++targetCounter.current });
  };

  return <div className="ym-memory-management">
    <div className="ym-memory-tabs" role="tablist" aria-label="当前会话内容">
      {(["notebook", "short", "long"] as const).map(kind => <button key={kind} type="button" className="ym-button"
        role="tab" id={`ym-memory-tab-${kind}`} aria-controls={`ym-memory-page-${kind}`} aria-selected={tab === kind}
        onClick={() => setTab(kind)}>{kind === "notebook" ? "记事本" : kind === "short" ? "短期记忆" : "长期记忆"}</button>)}
    </div>
    <div id="ym-memory-page-notebook" role="tabpanel" aria-labelledby="ym-memory-tab-notebook" hidden={tab !== "notebook"}>
      <NotebookPanel context={context} />
    </div>
    {(["short", "long"] as const).map(kind => <div key={kind} id={`ym-memory-page-${kind}`} role="tabpanel"
      aria-labelledby={`ym-memory-tab-${kind}`} hidden={tab !== kind}>
      <MemoryKindPanel context={context} kind={kind} active={tab === kind} target={kind === "long" ? target : null}
        previewOwner={previewOwner} onPreviewOwner={setPreviewOwner} onLocateLong={locateLong} />
    </div>)}
  </div>;
}

type KindProps = Props & {
  kind: MemoryKind; active: boolean; target: Target | null; previewOwner: MemoryKind | null;
  onPreviewOwner: (owner: MemoryKind | null) => void; onLocateLong: (id: string) => void;
};

function MemoryKindPanel({ context, kind, active, target, previewOwner, onPreviewOwner, onLocateLong }: KindProps) {
  const [items, setItems] = useState<MemoryRecord[]>([]);
  const [nextAfterId, setNextAfterId] = useState<string | null>(null);
  const [listLoading, setListLoading] = useState(true);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detail, setDetail] = useState<MemoryDetail | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [preview, setPreview] = useState<MemoryPreview | null>(null);
  const [previewTitle, setPreviewTitle] = useState("");
  const [previewContent, setPreviewContent] = useState("");
  const [busy, setBusy] = useState<"generate" | "save" | "cancel" | null>(null);
  const [listError, setListError] = useState("");
  const [readError, setReadError] = useState("");
  const [operationError, setOperationError] = useState("");
  const [notice, setNotice] = useState("");
  const [remoteChanged, setRemoteChanged] = useState(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [settingsError, setSettingsError] = useState("");
  const mounted = useRef(false);
  const listGeneration = useRef(0);
  const detailGeneration = useRef(0);
  const operationGeneration = useRef(0);
  const selectedId = useRef<string | null>(null);
  const [page, setPage] = useState(0);
  const [expanded, setExpanded] = useState(false);
  const currentPage = useRef(0);
  const cursors = useRef<(string | null)[]>([null]);
  const busyRef = useRef(busy);
  const refreshPending = useRef(false);
  const state = useRef({ detail, title, content, preview });
  const refreshRef = useRef<(() => void) | null>(null);
  const loadDetailRef = useRef<((id: string, force?: boolean) => Promise<void>) | null>(null);
  const loadListRef = useRef<((page?: number, locateId?: string) => Promise<void>) | null>(null);
  busyRef.current = busy;
  state.current = { detail, title, content, preview };
  const label = kind === "short" ? "短期记忆" : "长期记忆";
  const actionLabel = kind === "short" ? "重新生成" : "重新整合";
  const dirty = detail !== null && (content !== detail.record.content || (kind === "long" && title !== (detail.record.title ?? "")));

  const acceptDetail = (next: MemoryDetail) => {
    state.current = { detail: next, title: next.record.title ?? "", content: next.record.content, preview: null };
    setDetail(next); setTitle(next.record.title ?? ""); setContent(next.record.content);
    setPreview(null); setRemoteChanged(false); setReadError(""); setOperationError("");
    setItems(current => current.map(item => item.id === next.record.id ? next.record : item));
  };

  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    const stops: (() => void)[] = [];

    const loadDetail = async (id: string, force = false) => {
      if (busyRef.current) { refreshPending.current = true; return; }
      const token = ++detailGeneration.current;
      if (!state.current.detail || force) setDetailLoading(true);
      setReadError("");
      try {
        const next = await memoryManagementApi.get(context, kind, id);
        if (disposed || token !== detailGeneration.current || selectedId.current !== id) return;
        const current = state.current;
        const changedDraft = current.detail && (current.content !== current.detail.record.content ||
          (kind === "long" && current.title !== (current.detail.record.title ?? "")));
        if (!force && (changedDraft || current.preview || busyRef.current)) {
          if (current.detail && next.memoryRevision !== current.detail.memoryRevision) setRemoteChanged(true);
        } else {
          acceptDetail(next);
          if (force) setNotice("已重新加载最新内容。");
        }
      } catch (cause) {
        if (!disposed && token === detailGeneration.current && selectedId.current === id) setReadError(errorMessage(cause));
      } finally {
        if (!disposed && token === detailGeneration.current) setDetailLoading(false);
      }
    };

    const loadList = async (requestedPage = currentPage.current, locateId?: string): Promise<void> => {
      if (!context.sessionId) { setListLoading(false); return; }
      const token = ++listGeneration.current;
      setListLoading(true); setListError("");
      try {
        let index = requestedPage;
        const positions = [...cursors.current];
        let cursor = positions[index];
        if (cursor === undefined) return;
        while (true) {
          const result = await memoryManagementApi.list(context, kind, cursor, PAGE_SIZE);
          if (disposed || token !== listGeneration.current) return;
          positions[index] = cursor;
          if (locateId && !result.items.some(item => item.id === locateId) && result.nextAfterId) {
            cursor = result.nextAfterId; index++; continue;
          }
          cursors.current = positions.slice(0, index + 1);
          if (result.nextAfterId) cursors.current[index + 1] = result.nextAfterId;
          currentPage.current = index; setPage(index);
          setItems(result.items); setNextAfterId(result.nextAfterId);
          break;
        }
      } catch (cause) {
        if (!disposed && token === listGeneration.current) setListError(errorMessage(cause));
      } finally {
        if (!disposed && token === listGeneration.current) setListLoading(false);
      }
    };

    const refresh = () => {
      if (busyRef.current) { refreshPending.current = true; return; }
      void loadList();
      if (selectedId.current) void loadDetail(selectedId.current);
    };
    const loadSettings = async () => {
      try {
        const next = await configurationApi.getSettings();
        if (!disposed) { setSettings(next); setSettingsError(""); }
      } catch (cause) { if (!disposed) setSettingsError(errorMessage(cause)); }
    };
    loadDetailRef.current = loadDetail;
    loadListRef.current = loadList;
    refreshRef.current = refresh;
    void loadSettings();
    if (context.sessionId) {
      void memoryManagementApi.listen(() => { if (!disposed) refresh(); }).then(stop => {
        if (disposed) stop(); else { stops.push(stop); refresh(); }
      }).catch(cause => {
        if (!disposed) { setOperationError(`订阅记忆更新失败：${errorMessage(cause)}`); refresh(); }
      });
      void configurationApi.listenChanges(() => { if (!disposed) void loadSettings(); }).then(stop => {
        if (disposed) stop(); else stops.push(stop);
      }).catch(cause => { if (!disposed) setSettingsError(errorMessage(cause)); });
      window.addEventListener("focus", refresh);
    } else setListLoading(false);
    return () => {
      disposed = true; mounted.current = false;
      listGeneration.current++; detailGeneration.current++; operationGeneration.current++;
      stops.forEach(stop => stop()); window.removeEventListener("focus", refresh);
      loadDetailRef.current = null; loadListRef.current = null; refreshRef.current = null;
    };
  }, [context, kind]);

  const select = (id: string, toggle = true) => {
    if (selectedId.current === id) { setExpanded(value => toggle ? !value : true); return true; }
    if (busyRef.current || state.current.preview || dirty) {
      setNotice("当前编辑仍保留。请先保存，或重新加载放弃草稿，再查看其他记忆。");
      return false;
    }
    selectedId.current = id; setExpanded(true);
    setDetail(null); setTitle(""); setContent(""); setOperationError(""); setNotice(""); setRemoteChanged(false);
    state.current = { detail: null, title: "", content: "", preview: null };
    void loadDetailRef.current?.(id);
    return true;
  };

  useEffect(() => {
    if (target && select(target.id, false)) void loadListRef.current?.(0, target.id);
  }, [target]);

  const finishOperation = (token: number) => {
    if (!mounted.current || token !== operationGeneration.current) return;
    busyRef.current = null; setBusy(null);
    if (refreshPending.current) { refreshPending.current = false; refreshRef.current?.(); }
  };

  const save = async () => {
    const current = state.current;
    if (!current.detail || current.preview || busyRef.current || remoteChanged || !dirty ||
      !validText(current.content) || (kind === "long" && !validText(current.title))) return;
    const token = ++operationGeneration.current;
    busyRef.current = "save"; setBusy("save"); setOperationError(""); setNotice("");
    detailGeneration.current++;
    try {
      const next = await memoryManagementApi.save(context, current.detail, kind === "long" ? current.title : null, current.content);
      if (!mounted.current || token !== operationGeneration.current) return;
      acceptDetail(next); setNotice(`${label}已保存。`);
    } catch (cause) {
      if (!mounted.current || token !== operationGeneration.current) return;
      if (isConflict(cause)) {
        setRemoteChanged(true); setOperationError("记忆或来源已更新。草稿仍保留；请重新加载最新内容后再编辑。");
      } else setOperationError(`保存失败：${errorMessage(cause)}`);
    } finally { finishOperation(token); }
  };

  const generate = async () => {
    if (!active || !detail || busyRef.current || previewOwner || dirty || remoteChanged) return;
    const token = ++operationGeneration.current;
    busyRef.current = "generate"; setBusy("generate"); onPreviewOwner(kind); setOperationError(""); setNotice("");
    detailGeneration.current++;
    try {
      const result = await memoryManagementApi.preview(context, kind, detail.record.id);
      if (!mounted.current || token !== operationGeneration.current) return;
      acceptDetail(result.detail);
      setPreview(result); setPreviewTitle(result.title ?? ""); setPreviewContent(result.content);
      state.current = { ...state.current, preview: result };
    } catch (cause) {
      if (!mounted.current || token !== operationGeneration.current) return;
      onPreviewOwner(null);
      if (isConflict(cause)) setRemoteChanged(true);
      setOperationError(`${actionLabel}失败：${errorMessage(cause)}`);
    } finally { finishOperation(token); }
  };

  const cancel = async () => {
    if (busyRef.current === "save" || busyRef.current === "cancel") return;
    const token = ++operationGeneration.current;
    busyRef.current = "cancel"; setBusy("cancel"); setOperationError("");
    try {
      await memoryManagementApi.cancel(context);
      if (!mounted.current || token !== operationGeneration.current) return false;
      setPreview(null); state.current = { ...state.current, preview: null }; onPreviewOwner(null);
      setNotice("已取消，原记忆未被替换。");
      return true;
    } catch (cause) {
      if (mounted.current && token === operationGeneration.current) {
        if (!state.current.preview) onPreviewOwner(null);
        setOperationError(`取消失败：${errorMessage(cause)}`);
      }
      return false;
    } finally { finishOperation(token); }
  };

  const reload = async () => {
    if (!selectedId.current || busyRef.current) return;
    if (preview && !await cancel()) return;
    void loadDetailRef.current?.(selectedId.current, true);
  };

  const confirm = async () => {
    if (!preview || busyRef.current || remoteChanged || !validText(previewContent) ||
      (kind === "long" && !validText(previewTitle))) return;
    const token = ++operationGeneration.current;
    busyRef.current = "save"; setBusy("save"); setOperationError("");
    detailGeneration.current++;
    try {
      const next = await memoryManagementApi.confirm(context, preview.previewId, kind === "long" ? previewTitle : null, previewContent);
      if (!mounted.current || token !== operationGeneration.current) return;
      acceptDetail(next); onPreviewOwner(null); setNotice(`${label}已替换。`);
    } catch (cause) {
      if (!mounted.current || token !== operationGeneration.current) return;
      if (isConflict(cause)) {
        setRemoteChanged(true); setOperationError("预览已失效。预览草稿仍保留；请重新加载最新内容后重新预览。");
      } else setOperationError(`确认失败：${errorMessage(cause)}`);
    } finally { finishOperation(token); }
  };

  if (!context.sessionId) return <section className="ym-notebook-panel">
    <h2>{label}</h2><div className="ym-notebook-empty" role="status"><strong>尚无当前会话</strong>
      <p>选择或创建会话后，这里会显示该会话的{label}。</p></div>
  </section>;

  const changePage = (index: number) => {
    if (dirty || preview || busy || detailLoading || listLoading) return;
    detailGeneration.current++; selectedId.current = null; setExpanded(false); setDetail(null);
    state.current = { detail: null, title: "", content: "", preview: null };
    setReadError(""); setOperationError(""); setNotice("");
    void loadListRef.current?.(index);
  };

  const editorInvalid = !validText(preview ? previewContent : content) || (kind === "long" && !validText(preview ? previewTitle : title));
  const minChars = settings?.[kind === "short" ? "short_memory_min_chars" : "long_memory_min_chars"];
  const maxChars = settings?.[kind === "short" ? "short_memory_max_chars" : "long_memory_max_chars"];
  const blocked = dirty || !!preview || !!busy || detailLoading;

  return <section className="ym-notebook-panel ym-memory-kind" aria-label={label} aria-busy={listLoading || detailLoading || !!busy}>
    <div className="ym-notebook-heading"><div><h2>{label}</h2>
      <p>{kind === "short" ? "每轮完整对话的中性事实记录。" : "保留原有来源的中性事实记录。"}</p></div>
      <button type="button" className="ym-button" disabled={listLoading || !!busy} onClick={() => refreshRef.current?.()}>刷新列表</button>
    </div>
    {minChars && maxChars && <p className="ym-page-note">AI {actionLabel}生成的正文须为 {minChars}–{maxChars} 个汉字。
      手动编辑与预览修改不受该字数范围限制。</p>}
    {settingsError && <p role="alert" className="ym-notebook-error">读取建议字数失败：{settingsError}</p>}
    <div className="ym-memory-grid" aria-label={`${label}列表`}>
      {(detail && !items.some(item => item.id === detail.record.id) ? [detail.record, ...items] : items).map(record =>
        <InlineDetailCard key={record.id}
          title={kind === "long" ? record.title : `回合 ${record.roundNumber ?? "未知"}`}
          summary={<span className="ym-memory-summary">{record.content}</span>}
          meta={kind === "short" ? <span className="ym-memory-state">{record.integrationState === "integrated" ? "已经整合" : "尚未整合"}</span>
            : `${record.sourceCount} 条来源 · 本会话回合 ${record.sourceStartRoundNumber ?? "未知"}–${record.sourceEndRoundNumber ?? "未知"}`}
          expanded={expanded && selectedId.current === record.id} onToggle={() => select(record.id)}
          disabled={blocked && selectedId.current !== record.id} detailLabel={`${label}详情`}>
          {selectedId.current === record.id && <div className="ym-memory-detail">
        {detailLoading && <p role="status">正在读取记忆详情…</p>}
        {readError && <div role="alert" className="ym-page-error">读取记忆失败：{readError}
          <button className="ym-button" type="button" onClick={() => selectedId.current && void loadDetailRef.current?.(selectedId.current)}>重试详情</button></div>}
        {detail && <>
          <div className="ym-memory-detail-heading"><h3>{kind === "short" ? `回合 ${detail.record.roundNumber ?? "未知"}` : detail.record.title}</h3>
            <span className="ym-notebook-updated">最近保存：{new Date(detail.record.updatedAt).toLocaleString()}</span></div>
          {kind === "short" && <p className="ym-memory-state">{detail.record.integrationState === "integrated" ? "已经整合" : "尚未整合"}</p>}
          {kind === "short" && detail.record.integrationState === "integrated" && <div className="ym-notebook-over-limit" role="status">
            现有长期记忆不会自动变化，可按需重新整合相关长期记忆。
            <div className="ym-memory-actions">{detail.relatedLongMemories.map(memory => <button className="ym-button" type="button" key={memory.id}
              onClick={() => onLocateLong(memory.id)}>查看长期记忆：{memory.title}</button>)}</div>
          </div>}
          {kind === "long" && <>
            <p className="ym-page-note">来源 {detail.record.sourceCount} 条 · 本会话回合 {detail.record.sourceStartRoundNumber ?? "未知"}–{detail.record.sourceEndRoundNumber ?? "未知"} · 触发于本会话回合 {detail.record.activatedByRoundNumber ?? "未知"}</p>
            <details className="ym-memory-sources"><summary>来源短期记忆（{detail.sources.length} 条）</summary>
              <ol>{[...detail.sources].sort((left, right) => left.sourceOrder - right.sourceOrder).map(source => <li key={source.shortMemoryId}>
                <strong>本会话回合 {source.roundNumber ?? "未知"}</strong><p>{source.content}</p>
              </li>)}</ol>
            </details>
          </>}
          {remoteChanged && <div className="ym-notebook-remote" role="status"><span>记忆或来源已有更新。当前草稿已保留，请重新加载后继续。</span>
            <button className="ym-button" type="button" disabled={!!busy || detailLoading} onClick={() => void reload()}>重新加载最新内容并放弃草稿</button></div>}
          {preview ? <div className="ym-memory-preview" aria-label="记忆预览">
            <h3>{actionLabel}预览</h3><p className="ym-page-note">确认替换前可编辑新内容，取消会保留原记忆。</p>
            <div className="ym-memory-compare"><div><h4>原内容</h4>{kind === "long" && <strong>{preview.detail.record.title}</strong>}
              <p className="ym-memory-original">{preview.detail.record.content}</p></div>
              <div>{kind === "long" && <label className="ym-notebook-field"><span>预览标题</span>
                <input aria-label="预览标题" value={previewTitle} disabled={!!busy} onChange={event => setPreviewTitle(event.target.value)} /></label>}
                <label className="ym-notebook-field"><span>预览正文</span><textarea aria-label="预览正文" value={previewContent}
                  disabled={!!busy} onChange={event => setPreviewContent(event.target.value)} /></label></div>
            </div>
            <div className="ym-memory-actions"><button className="ym-button" type="button" disabled={!!busy || remoteChanged || editorInvalid}
              onClick={() => void confirm()}>{busy === "save" ? "确认中…" : "确认替换"}</button>
              <button className="ym-button" type="button" disabled={!!busy} onClick={() => void cancel()}>取消预览</button></div>
          </div> : <>
            {kind === "long" && <label className="ym-notebook-field"><span>长期记忆标题</span>
              <input aria-label="长期记忆标题" value={title} disabled={!!busy || detailLoading} onChange={event => setTitle(event.target.value)} /></label>}
            <label className="ym-notebook-field"><span>{label}正文</span><textarea aria-label={`${label}正文`} value={content}
              disabled={!!busy || detailLoading} onChange={event => { setContent(event.target.value); setNotice(""); }} /></label>
            <div className="ym-memory-actions"><button className="ym-button" type="button" disabled={!dirty || !!busy || detailLoading || remoteChanged || editorInvalid}
              onClick={() => void save()}>{busy === "save" ? "保存中…" : "保存编辑"}</button>
              <button className="ym-button" type="button" disabled={!!busy || !!previewOwner || dirty || remoteChanged || detailLoading || !active}
                onClick={() => void generate()}>{busy === "generate" ? "正在生成预览…" : actionLabel}</button>
              {busy === "generate" && <button className="ym-button" type="button" onClick={() => void cancel()}>取消生成</button>}
              {dirty && !remoteChanged && <button className="ym-button" type="button" disabled={!!busy || detailLoading} onClick={() => void reload()}>重新加载并放弃草稿</button>}
            </div>
            {dirty && <p className="ym-page-note">当前有未保存的编辑。保存或放弃草稿后，可选择其他记忆和使用 AI 预览。</p>}
          </>}
          {editorInvalid && <p className="ym-notebook-error" role="status">正文{kind === "long" ? "和标题" : ""}不能为空。</p>}
        </>}
          </div>}
        </InlineDetailCard>)}
    </div>
    {listLoading && <p role="status">正在读取{label}…</p>}
    {!items.length && !listLoading && !listError && <div className="ym-notebook-empty" role="status">
      <strong>暂无{label}</strong><p>{kind === "short" ? "完成对话后会自动生成短期记忆。" : "待整合短期记忆达到条件后会自动生成长期记忆。"}</p>
    </div>}
    {listError && <div role="alert" className="ym-page-error">读取列表失败：{listError}
      <button className="ym-button" type="button" disabled={listLoading} onClick={() => void loadListRef.current?.()}>重试列表</button></div>}
    <nav className="ym-memory-pagination" aria-label={`${label}分页`}>
      <button className="ym-button" type="button" disabled={page === 0 || listLoading || blocked} onClick={() => changePage(page - 1)}>上一页</button>
      <span aria-live="polite">第 {page + 1} 页 · 每页 {PAGE_SIZE} 条</span>
      <button className="ym-button" type="button" disabled={!nextAfterId || listLoading || blocked} onClick={() => changePage(page + 1)}>下一页</button>
    </nav>
    {previewOwner && previewOwner !== kind && <p role="status" className="ym-page-note">另一页签有 AI 生成或预览，请先确认或取消后再生成。</p>}
    {operationError && <p role="alert" className="ym-notebook-error">{operationError}</p>}
    {notice && <p role="status" className="ym-notebook-notice">{notice}</p>}
  </section>;
}
