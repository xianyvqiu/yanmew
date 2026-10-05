import { useCallback, useEffect, useRef, useState } from "react";
import { errorMessage, memoryGenerationApi } from "../../api";
import type { CurrentContext, MemoryGenerationView } from "../../types";

/** 当前会话短期生成和长期整合状态；正文管理由后续页面接入。 */
export default function MemoryGenerationPanel({ context }: { context: CurrentContext }) {
  const [view, setView] = useState<MemoryGenerationView | null>(null);
  const [error, setError] = useState("");
  const [retrying, setRetrying] = useState(false);
  const generation = useRef(0);
  const mounted = useRef(false);
  const read = useCallback(async () => {
    if (!context.sessionId) return;
    const current = ++generation.current;
    try {
      const result = await memoryGenerationApi.get(context);
      if (mounted.current && current === generation.current) { setView(result); setError(""); }
    } catch (cause) {
      if (mounted.current && current === generation.current) setError(errorMessage(cause));
    }
  }, [context]);
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void memoryGenerationApi.listen(() => { if (!disposed) void read(); }).then(stop => {
      if (disposed) stop();
      else { unlisten = stop; void read(); }
    }).catch(cause => {
      if (!disposed) { setError(errorMessage(cause)); void read(); }
    });
    const focus = () => { void read(); };
    window.addEventListener("focus", focus);
    return () => {
      disposed = true; mounted.current = false; generation.current++;
      unlisten?.(); window.removeEventListener("focus", focus);
    };
  }, [read]);
  const retry = async () => {
    setRetrying(true); setError("");
    try { await memoryGenerationApi.retry(context); if (mounted.current) await read(); }
    catch (cause) { if (mounted.current) setError(errorMessage(cause)); }
    finally { if (mounted.current) setRetrying(false); }
  };
  if (!context.sessionId) return null;
  return <section className="ym-page-card" aria-label="记忆生成与整合状态">
    <h2>短期记忆生成</h2>
    <p className="ym-page-note">完整对话结束后自动生成；中断回合和主动消息不生成。混合检索将在后续接入。</p>
    {view ? <p role="status">已生成 {view.completedCount} 条，待生成 {view.pendingCount} 轮。
      {view.activeRoundId && "正在生成…"}</p> : !error && <p role="status">正在读取生成状态…</p>}
    {view?.error && <div role="alert" className="ym-page-error">短期记忆生成失败：{view.error.message}
      <p>{view.autoRetryPending ? "后台将有限重试；聊天记录仍完整保留。" : "自动重试已暂停。可检查记忆提供商后重试；聊天记录仍完整保留。"}</p>
    </div>}
    <h2>长期记忆整合</h2>
    <p className="ym-page-note">未整合短期记忆达到设置的 k+1 条时，整合前 k 条；触发条保留，已有记忆不会删除。</p>
    {view && <p role="status">已整合 {view.longCompletedCount} 条，待整合 {view.longPendingCount} 组。
      {view.longActiveRoundId && "正在整合…"}</p>}
    {view?.longError && <div role="alert" className="ym-page-error">长期记忆整合失败：{view.longError.message}
      <p>{view.longAutoRetryPending ? "后台将有限重试；来源短期记忆仍保留。" : "自动重试已暂停。可检查记忆提供商后重试；来源短期记忆仍保留。"}</p>
    </div>}
    {error && <div role="alert" className="ym-page-error">状态操作失败：{error}
      <button type="button" className="ym-button" onClick={() => void read()}>重新读取状态</button>
    </div>}
    {!!view && (view.pendingCount > 0 || view.longPendingCount > 0) && <button type="button" className="ym-button"
      disabled={retrying || !!view.activeRoundId || !!view.longActiveRoundId}
      onClick={() => void retry()}>{retrying ? "正在重试…" : "重试待处理记忆"}</button>}
  </section>;
}
