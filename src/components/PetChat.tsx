import { useEffect, useRef } from "react";
import ChatComposer from "./ChatComposer";
import ChatMessageText from "./ChatMessageText";
import type { usePetChat } from "../windows/usePetChat";

type PetChatState = ReturnType<typeof usePetChat>;
export function PetChatBubble({ chat, onDetails }: { chat: PetChatState; onDetails: () => void }) {
  const body = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  useEffect(() => {
    if (follow.current && body.current) body.current.scrollTop = body.current.scrollHeight;
  }, [chat.reply?.text]);
  if (!chat.reply) return null;
  return <section className="pet-chat-bubble" data-pet-interactive aria-label="桌宠聊天回复">
    <div ref={body} className="pet-chat-bubble__body" aria-live="polite" aria-busy={chat.reply.streaming}
      onScroll={() => { const element = body.current; if (element) follow.current = element.scrollHeight - element.scrollTop - element.clientHeight < 24; }}>
      <ChatMessageText content={chat.reply.text} />
      {chat.reply.error && <p className="pet-chat-message-error" role="alert">{chat.reply.error}</p>}
      {chat.reply.notice && !chat.reply.streaming && <p className="pet-chat-note">{chat.reply.notice}</p>}
    </div>
    <div className="pet-chat-bubble__actions">
      <button className="ym-button" onClick={onDetails}>聊天详情</button>
      <button className="ym-button" aria-label="收起聊天回复" onClick={chat.dismissReply}>收起</button>
    </div>
  </section>;
}

export function PetChatInput({ chat, hasSession, away, pending, onDetails, onLetter }: {
  chat: PetChatState; hasSession: boolean; away: boolean; pending: boolean;
  onDetails: () => void; onLetter: () => void;
}) {
  return <section className="pet-chat-input" data-pet-interactive aria-label="宠窗快捷聊天">
    {!hasSession ? <div className="pet-chat-empty"><span>先创建一个会话，再来聊聊吧。</span><button className="ym-button" onClick={onDetails}>聊天详情</button></div>
      : away ? <div className="pet-chat-empty"><span>出行期间只能写信。</span><button className="ym-button" onClick={onLetter}>写信</button></div>
        : <>
          {chat.error && <p className="pet-chat-message-error" role="alert">{chat.error}<button className="ym-button" onClick={() => { void chat.reload(); }}>重试连接</button></p>}
          <ChatComposer compact value={chat.draft} onChange={chat.setDraft} onSend={() => { void chat.send(); }} onCancel={() => { void chat.cancel(); }}
            disabled={!chat.ready || pending} busy={chat.busy} cancelling={chat.cancelling} />
        </>}
  </section>;
}
