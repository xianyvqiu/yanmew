import { useEffect, useRef, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { configurationApi, errorMessage, notebookApi } from "../../api";
import type { CurrentContext, NotebookView } from "../../types";
import "./NotebookPanel.css";

type NotebookPanelProps = { context: CurrentContext };

function isNotebookConflict(cause: unknown) {
  return Boolean(cause && typeof cause === "object" && "code" in cause && cause.code === "NOTEBOOK_CONFLICT");
}

function sameNotebook(left: NotebookView["notebook"], right: NotebookView["notebook"]) {
  return left === null ? right === null : right !== null && left.sessionId === right.sessionId &&
    left.content === right.content && left.updatedAt === right.updatedAt;
}

export default function NotebookPanel(props: NotebookPanelProps) {
  const contextKey = `${props.context.personaId}:${props.context.sessionId ?? "none"}:${props.context.contextRevision}`;
  return <NotebookPanelForContext key={contextKey} {...props} />;
}

function NotebookPanelForContext({ context }: NotebookPanelProps) {
  const [view, setView] = useState<NotebookView | null>(null);
  const [content, setContent] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [readError, setReadError] = useState("");
  const [operationError, setOperationError] = useState("");
  const [remoteChanged, setRemoteChanged] = useState(false);
  const [notice, setNotice] = useState("");
  const mounted = useRef(false);
  const saveLocked = useRef(false);
  const refreshPending = useRef(false);
  const contentRef = useRef(content);
  const viewRef = useRef(view);
  const activeContextKey = useRef("");
  const requestGeneration = useRef(0);
  const refreshRef = useRef<((force?: boolean) => void) | null>(null);

  contentRef.current = content;
  viewRef.current = view;

  const sessionId = context.sessionId;
  const contextKey = `${context.personaId}:${sessionId ?? "none"}:${context.contextRevision}`;

  activeContextKey.current = contextKey;

  useEffect(() => {
    mounted.current = true;
    activeContextKey.current = contextKey;
    requestGeneration.current++;
    let disposed = false;
    const stops: UnlistenFn[] = [];

    const readSettings = async () => {
      const settings = await configurationApi.getSettings();
      if (!disposed) {
        const limitChars = Number(settings.notebook_max_chars);
        const current = viewRef.current;
        if (current) {
          const next = { ...current, limitChars };
          viewRef.current = next;
          setView(next);
        }
      }
    };

    const load = async (force = false) => {
      if (!sessionId) return;
      if (saveLocked.current) {
        refreshPending.current = true;
        return;
      }
      const token = ++requestGeneration.current;
      const targetContext = contextKey;
      if (!viewRef.current || force) setLoading(true);
      setReadError("");
      try {
        const [nextView, settings] = await Promise.all([notebookApi.get(context), configurationApi.getSettings()]);
        if (disposed || requestGeneration.current !== token || activeContextKey.current !== targetContext) return;
        const next = { ...nextView, limitChars: Number(settings.notebook_max_chars) };
        const current = viewRef.current;
        const baselineContent = current?.notebook?.content ?? "";
        const dirty = current !== null && contentRef.current !== baselineContent;
        if (force || !dirty || current === null) {
          viewRef.current = next;
          contentRef.current = next.notebook?.content ?? "";
          setView(next);
          setContent(contentRef.current);
          setRemoteChanged(false);
          setOperationError("");
          setNotice(force ? "已重新加载最新记事本。" : "");
        } else {
          const withLatestLimit = { ...current, limitChars: next.limitChars };
          viewRef.current = withLatestLimit;
          setView(withLatestLimit);
          setRemoteChanged(!sameNotebook(current.notebook, next.notebook));
        }
      } catch (cause) {
        if (!disposed && requestGeneration.current === token && activeContextKey.current === targetContext) setReadError(errorMessage(cause));
      } finally {
        if (!disposed && requestGeneration.current === token && activeContextKey.current === targetContext) setLoading(false);
      }
    };
    const refresh = (force = false) => { void load(force); };
    refreshRef.current = refresh;

    const refreshLimit = async () => {
      try { await readSettings(); }
      catch (cause) { if (!disposed) setOperationError(`读取记事本上限失败：${errorMessage(cause)}`); }
    };

    const onFocus = () => { refresh(); };
    void (async () => {
      if (!sessionId) { setLoading(false); return; }
      try {
        const stopNotebook = await notebookApi.listen(() => { if (!disposed) refresh(); });
        if (disposed) { stopNotebook(); return; }
        stops.push(stopNotebook);
        const stopConfiguration = await configurationApi.listenChanges(() => { if (!disposed) void refreshLimit(); });
        if (disposed) { stopConfiguration(); return; }
        stops.push(stopConfiguration);
        window.addEventListener("focus", onFocus);
        await load();
      } catch (cause) {
        if (!disposed) {
          setOperationError(`订阅记事本更新失败：${errorMessage(cause)}`);
          await load();
        }
      }
    })();

    return () => {
      disposed = true;
      mounted.current = false;
      requestGeneration.current++;
      refreshPending.current = false;
      if (refreshRef.current === refresh) refreshRef.current = null;
      stops.forEach(stop => stop());
      window.removeEventListener("focus", onFocus);
    };
  }, [context, contextKey, sessionId]);

  const edit = (next: string) => {
    contentRef.current = next;
    setContent(next);
    setOperationError("");
    setNotice("");
  };

  const reloadRemote = async () => {
    if (!sessionId || loading) return;
    refreshRef.current?.(true);
  };

  const save = async () => {
    if (!sessionId || !view || saveLocked.current || saving) return;
    saveLocked.current = true;
    refreshPending.current = false;
    setSaving(true);
    const token = ++requestGeneration.current;
    const targetContext = contextKey;
    setOperationError("");
    setNotice("");
    try {
      const saved = await notebookApi.save(context, view.notebook, content);
      if (!mounted.current || requestGeneration.current !== token || activeContextKey.current !== targetContext) return;
      const next = { ...saved, limitChars: viewRef.current?.limitChars ?? saved.limitChars };
      viewRef.current = next;
      contentRef.current = saved.notebook?.content ?? content;
      setView(next);
      setContent(contentRef.current);
      setRemoteChanged(false);
      setNotice("记事本已保存。");
    } catch (cause) {
      if (!mounted.current || requestGeneration.current !== token || activeContextKey.current !== targetContext) return;
      if (isNotebookConflict(cause)) {
        setRemoteChanged(true);
        setOperationError("记事本已在其他位置更新。草稿仍保留；重新加载最新内容后再继续编辑。");
      } else {
        setOperationError(`保存失败：${errorMessage(cause)}`);
      }
    } finally {
      saveLocked.current = false;
      const canRefresh = mounted.current && requestGeneration.current === token && activeContextKey.current === targetContext;
      if (canRefresh) {
        requestGeneration.current++;
        setSaving(false);
      }
      const shouldRefresh = refreshPending.current;
      refreshPending.current = false;
      if (canRefresh && shouldRefresh) refreshRef.current?.();
    }
  };

  const baselineContent = view?.notebook?.content ?? "";
  const dirty = content !== baselineContent;
  const charCount = Array.from(content).length;

  if (!sessionId) {
    return <section className="ym-notebook-panel" aria-labelledby="ym-notebook-title">
      <h2 id="ym-notebook-title">记事本</h2>
      <div className="ym-notebook-empty" role="status">
        <strong>尚无当前会话</strong>
        <p>选择或创建会话后，这里会显示该会话的记事本。</p>
      </div>
    </section>;
  }

  return <section className="ym-notebook-panel" aria-labelledby="ym-notebook-title" aria-busy={loading || saving}>
    <div className="ym-notebook-heading">
      <div>
        <h2 id="ym-notebook-title">记事本</h2>
        <p>记录当前会话的重要内容。</p>
      </div>
      <button className="ym-button ym-notebook-save" type="button" onClick={() => void save()}
        disabled={loading || saving || !view || !dirty}>
        {saving ? "保存中…" : "保存"}
      </button>
    </div>

    {view && <div className="ym-notebook-meta">
      <span aria-live="polite">{charCount} / {view.limitChars} 字</span>
      <span className="ym-notebook-soft-limit">建议上限为软限制，超出后仍会完整保存，不会截断正文。</span>
    </div>}

    {charCount > (view?.limitChars ?? 0) && view && <p className="ym-notebook-over-limit" role="status">
      已超过建议上限，建议后续整理；超限不影响保存。
    </p>}

    {remoteChanged && <div className="ym-notebook-remote" role="status">
      <span>检测到记事本在其他位置更新。当前草稿已保留。</span>
      <button className="ym-button" type="button" onClick={() => void reloadRemote()} disabled={loading || saving}>重新加载最新内容</button>
    </div>}

    {readError && <div className="ym-page-error" role="alert">
      <span>读取记事本失败：{readError}</span>
      <button className="ym-button" type="button" onClick={() => void reloadRemote()}>重试</button>
    </div>}
    {loading && !view ? <p role="status">正在读取记事本…</p> : view ? <label className="ym-notebook-field">
      <span>记事本全文</span>
      <textarea value={content} onChange={event => edit(event.target.value)} disabled={!view || saving}
        aria-label="记事本全文" placeholder="写下这个会话中希望保留的内容…" />
    </label> : !readError && <p role="status">正在读取记事本…</p>}

    {operationError && <p className="ym-notebook-error" role="alert">{operationError}</p>}
    {notice && <p className="ym-notebook-notice" role="status">{notice}</p>}
    {view?.notebook && <p className="ym-notebook-updated">最近保存：{new Date(view.notebook.updatedAt).toLocaleString()}</p>}
  </section>;
}
