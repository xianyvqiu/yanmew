import { useEffect, useRef, useState } from "react";
import { errorMessage, lettersApi } from "../api";
import { travelContextKey, useTravel } from "../hooks/useTravel";
import { navigation } from "../platform/navigation";
import { travelTime } from "./TravelPanel";
import type { CurrentContext, Letter, LetterTarget } from "../types";
import "./TravelPanel.css";

export default function LetterEditor({ context, target, value, onChange, onCancel }: {
  context: CurrentContext; target: LetterTarget; value: string; onChange: (value: string) => void; onCancel: () => Promise<void>;
}) {
  const travel = useTravel(context);
  const [original, setOriginal] = useState<Letter | null>(null);
  const [replyLoading, setReplyLoading] = useState(Boolean(target.replyToId));
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [sentReply, setSentReply] = useState(false);
  const [busy, setBusy] = useState(false);
  const locked = useRef(false);
  const alive = useRef(true);
  const activeTarget = useRef(target);
  activeTarget.current = target;
  const targetKey = `${travelContextKey(target)}:${target.travelId}:${target.replyToId ?? "new"}`;
  const currentTarget = travelContextKey(context) === travelContextKey(target);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  useEffect(() => { setOriginal(null); setError(""); setNotice(""); setSentReply(false); }, [targetKey]);
  useEffect(() => {
    let disposed = false;
    if (!currentTarget || !target.replyToId) { setReplyLoading(false); return; }
    setReplyLoading(true);
    void lettersApi.get(context, target.replyToId).then(letter => { if (!disposed) setOriginal(letter); })
      .catch(cause => { if (!disposed) { setOriginal(null); setError(errorMessage(cause)); } })
      .finally(() => { if (!disposed) setReplyLoading(false); });
    return () => { disposed = true; };
  }, [targetKey, currentTarget, travel.version]);
  const validReply = !target.replyToId || (original?.id === target.replyToId && original.sender === "ai" && original.travelId === target.travelId && !original.hasBeenReplied && !sentReply);
  const canSend = currentTarget && travel.current?.id === target.travelId && validReply && !travel.loading && !travel.error && !replyLoading && !busy && Boolean(value.trim());
  const send = async () => {
    if (locked.current || !canSend) return;
    locked.current = true; setBusy(true); setError(""); setNotice("");
    const captured = target;
    try {
      await lettersApi.send(context, {travelId:captured.travelId,replyToId:captured.replyToId,content:value});
      if (alive.current && activeTarget.current === captured) {
        onChange(""); setSentReply(Boolean(captured.replyToId)); setNotice("信件已寄出。回信时间已保存，旅行结束前会等待回复。");
        await travel.refresh();
      }
    } catch (cause) { if (alive.current && activeTarget.current === captured) setError(errorMessage(cause)); }
    finally { locked.current = false; if (alive.current) setBusy(false); }
  };
  const openMailbox = async () => {
    try { await navigation.open("mailbox"); } catch (cause) { if (alive.current) setError(errorMessage(cause)); }
  };
  return <section className="ym-page-card" aria-label="写信">
    <form className="ym-letter-editor" onSubmit={event => { event.preventDefault(); void send(); }}>
      <div className="ym-page-heading"><h2>{target.replyToId ? "回一封信" : "寄一封信"}</h2><button className="ym-button" type="button" onClick={() => { void openMailbox(); }}>跳转邮箱</button></div>
      {!currentTarget ? <p role="status" className="ym-page-note">当前人设或会话已变化，旧草稿已保留供复制，不能转投新会话。</p>
        : !travel.loading && travel.current?.id !== target.travelId ? <p role="status" className="ym-page-note">这次旅行已结束或取消，草稿仍保留，但不能再发送。</p>
          : travel.current && <p className="ym-page-note">本次旅行：{travel.current.destination ?? "行程准备中"} · 计划返回 {travelTime(travel.current.plannedReturnAt)}</p>}
      {replyLoading && <p role="status">正在读取回复目标…</p>}
      {original && <div className="ym-letter-target"><strong>回复的来信</strong><p className="ym-letter-body">{original.content}</p>
        {original.hasBeenReplied && <p className="ym-page-note">原信已回复，删除回复也不会恢复资格。</p>}</div>}
      <label htmlFor="letter-content">信件正文</label><textarea id="letter-content" aria-label="信件正文" value={value} onChange={event => onChange(event.target.value)} disabled={busy} rows={10} placeholder="写下想对它说的话…" />
      {(error || travel.error) && <p role="alert" className="ym-page-error">{error || travel.error}</p>}
      {notice && <p role="status" className="ym-page-note">{notice}</p>}
      <p className="ym-page-note">只有本次旅行未结束时可以寄信或回信；切换人设或会话会取消旅行和待回信。</p>
      <div className="ym-inline-form"><button className="ym-button" type="button" disabled={busy} onClick={() => { void onCancel().catch(cause => setError(errorMessage(cause))); }}>取消</button>
        <button className="ym-button" type="submit" disabled={!canSend}>{busy ? "正在寄出…" : "寄出"}</button>
        <button className="ym-button" type="button" disabled={busy || travel.loading} onClick={() => { void travel.refresh(); }}>刷新状态</button></div>
    </form>
  </section>;
}
