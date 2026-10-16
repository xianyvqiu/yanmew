import { useEffect, useRef, useState } from "react";

/** 只复制点击时的消息正文；不读取剪贴板，也不修改消息或输入草稿。 */
export default function CopyMessageButton({ content }: { content: string }) {
  const [copied, setCopied] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const alive = useRef(true);
  const locked = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; if (timer.current) clearTimeout(timer.current); };
  }, []);
  const copy = async () => {
    if (locked.current || !content.trim()) return;
    locked.current = true;
    if (timer.current) clearTimeout(timer.current);
    setPending(true); setCopied(false); setError("");
    try {
      await navigator.clipboard.writeText(content);
      if (!alive.current) return;
      setCopied(true);
      timer.current = setTimeout(() => { if (alive.current) setCopied(false); }, 2000);
    } catch {
      if (alive.current) setError("复制失败，请选中文字后手动复制。");
    } finally {
      locked.current = false;
      if (alive.current) setPending(false);
    }
  };
  return <span className="ym-message-copy">
    <button type="button" className="ym-button" disabled={pending || !content.trim()}
      title={content.trim() ? "复制消息正文" : "这条消息没有可复制的文字"}
      onClick={() => { void copy(); }}>{pending ? "复制中…" : copied ? "已复制" : "复制"}</button>
    {error && <span role="alert" className="ym-message-copy-error">{error}</span>}
  </span>;
}
