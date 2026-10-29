import { useEffect, useRef, useState } from "react";
import { useChat } from "../hooks/useChat";
import type { ChatRequestSnapshot, PetActivity, ChatImageInput } from "../types";

/** 只追踪宠窗实际提交成功的回合；消息、流式事件和保存均复用现有聊天服务。 */
export function usePetChat(view: PetActivity | null) {
  const context = { personaId: view?.personaId ?? "", sessionId: view?.sessionId ?? null, contextRevision: view?.contextRevision ?? -1 };
  const key = `${context.personaId}:${context.sessionId}:${context.contextRevision}`;
  const currentKey = useRef(key);
  currentKey.current = key;
  const [draft, setDraft] = useState("");
  const [images,setImages]=useState<ChatImageInput[]>([]);
  const [uploading,setUploading]=useState(false);
  const [target, setTarget] = useState<ChatRequestSnapshot | null>(null);
  const chat = useChat(context, { onSubmitted: receipt => setTarget(receipt) });
  useEffect(() => { setDraft(""); setImages([]); setUploading(false); setTarget(null); }, [key]);
  useEffect(() => {
    // 另一个入口开始新一轮后，不让上一轮的气泡继续占据头顶。
    if (target && chat.active && target.requestId !== chat.active.requestId) setTarget(null);
  }, [target, chat.active?.requestId]);
  const owned = target?.personaId === context.personaId && target.sessionId === context.sessionId && target.contextRevision === context.contextRevision ? target : null;
  const active = owned && chat.active?.requestId === owned.requestId ? chat.active : null;
  const saved = owned && chat.lastRound?.round.id === owned.roundId ? chat.lastRound : null;
  const message = saved?.assistantMessage ?? (owned ? chat.messages.find(item => item.roundId === owned.roundId && item.role === "assistant") : null);
  const send = async () => {
    if (!context.sessionId || view?.activity === "away") return;
    const submittedKey = key;
    if (await (images.length?chat.send(draft,images):chat.send(draft))) { if (currentKey.current === submittedKey) { setDraft(""); setImages([]); } }
  };
  return {
    ...chat, context, draft, setDraft, images, setImages, uploading,setUploading,attachmentOwnerKey:key, send,
    dismissReply: () => setTarget(null),
    reply: owned ? {
      images: message?.images?.filter(image=>image.sourceKind==="sticker") ?? [],
      text: active?.partialContent || message?.content || (message?.images?.some(image=>image.sourceKind==="sticker")?"":chat.busy ? "正在想怎么回答你…" : "这次回复暂未完成，请查看聊天详情。"),
      streaming: Boolean(active || chat.busy),
      error: chat.error,
      notice: chat.notice,
      expression: active?.expression ?? message?.expression,
    } : null,
  };
}
