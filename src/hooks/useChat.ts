import { useEffect, useRef, useState } from "react";
import { chatApi, errorMessage, sessionApi } from "../api";
import type { ChatReplyEvent, ChatRequestSnapshot, CurrentContext, Message, RoundSnapshot } from "../types";

type ChatView = {
  messages: Message[]; active: ChatRequestSnapshot | null; busy: boolean; cancelling: boolean;
  ready: boolean; loading: boolean; afterId: string | null; error: string; notice: string;
};
const empty: ChatView = { messages: [], active: null, busy: false, cancelling: false, ready: false, loading: false, afterId: null, error: "", notice: "" };

/** 页面只渲染后端保存结果；未保存增量单独展示，不另发消息写入命令。 */
export function useChat(context: CurrentContext) {
  const [view, setView] = useState<ChatView>(empty);
  const actions = useRef({ send: async (_input: string): Promise<boolean> => false, cancel: async () => {}, reload: async () => {}, more: async () => {} });
  useEffect(() => {
    let disposed = false;
    let state = { ...empty };
    let submitting = false;
    let syncing = false;
    let connecting = false;
    let paging = false;
    let cancelWanted = false;
    let buffered: ChatReplyEvent[] = [];
    let unlisten: (() => void) | undefined;
    const rows = new Map<string, Message>();
    const ended = new Set<string>();
    const publish = () => {
      if (disposed) return;
      state = { ...state, messages: [...rows.values()].sort((a, b) => {
        if (/^\d+$/.test(a.id) && /^\d+$/.test(b.id)) return BigInt(a.id) < BigInt(b.id) ? -1 : a.id === b.id ? 0 : 1;
        return a.createdAt.localeCompare(b.createdAt) || a.id.localeCompare(b.id);
      }), busy: submitting || state.active !== null };
      setView(state);
    };
    const save = (round: RoundSnapshot) => {
      rows.set(round.userMessage.id, round.userMessage);
      if (round.assistantMessage) rows.set(round.assistantMessage.id, round.assistantMessage);
    };
    const matches = (event: ChatReplyEvent) => event.personaId === context.personaId
      && event.sessionId === context.sessionId && event.contextRevision === context.contextRevision;
    const refreshMessages = async () => {
      const page = await sessionApi.messages(context);
      if (disposed) return;
      page.items.forEach(item => rows.set(item.id, item));
      // 保留已经翻页的位置；首次加载才设置游标。
      if (!state.ready) state.afterId = page.nextAfterId;
      publish();
    };
    const apply = (event: ChatReplyEvent) => {
      const active = state.active;
      if (ended.has(event.requestId)) return;
      if (active && (event.requestId !== active.requestId || event.roundId !== active.roundId || event.seq <= active.seq)) return;
      if (!active && (event.type === "text" || event.type === "expression")) return;
      if (active) state.active = { ...active, seq: event.seq };
      if (event.type === "text" && state.active) state.active.partialContent += event.text;
      else if (event.type === "expression" && state.active) state.active.expression = event.expression;
      else {
        ended.add(event.requestId);
        if (event.saved) save(event.saved);
        state.active = null; state.cancelling = false; cancelWanted = false;
        if (event.type === "failed") state.error = event.message;
        state.notice = event.saved?.round.status === "interrupted" ? "回复已中断，已收到的内容已保存。" : "";
        if (!event.saved) state.notice = "回复未能确认保存，请重新加载查看记录。";
        void refreshMessages().catch(cause => { if (!disposed) { state.error = errorMessage(cause); publish(); } });
      }
      publish();
    };
    const replay = () => {
      const events = buffered; buffered = [];
      events.sort((a, b) => a.seq - b.seq);
      for (let index = 0; index < events.length; index++) {
        const event = events[index];
        if (state.active && event.requestId === state.active.requestId && event.seq > state.active.seq + 1) {
          buffered.push(...events.slice(index)); void synchronize(); return;
        }
        apply(event);
      }
    };
    const synchronize = async () => {
      if (disposed || syncing || submitting || !context.sessionId) return;
      syncing = true; state.loading = true; publish();
      try {
        // 查询期间事件暂存；快照 seq 之前的增量在重放时自动忽略。
        const previous = state.active;
        const active = await chatApi.current(context);
        const completed = !active && previous ? await chatApi.cancel(previous) : null;
        await refreshMessages();
        if (disposed) return;
        state.active = active;
        if (active) save(active.round);
        else {
          state.cancelling = false;
          if (completed) {
            save(completed);
            state.notice = completed.round.status === "interrupted" ? "回复已中断，已收到的内容已保存。" : "";
          }
        }
        state.ready = true; state.error = "";
      } catch (cause) { if (!disposed) state.error = errorMessage(cause); }
      finally {
        syncing = false;
        if (!disposed) { state.loading = false; replay(); publish(); }
      }
    };
    const cancel = async () => {
      cancelWanted = true; state.cancelling = true; publish();
      if (submitting) return;
      const active = state.active;
      if (!active) { state.cancelling = false; publish(); return; }
      try {
        const saved = await chatApi.cancel(active);
        if (disposed || state.active?.requestId !== active.requestId) return;
        if (saved.round.status !== "pending") {
          save(saved); state.active = null; state.cancelling = false;
          state.notice = saved.round.status === "interrupted" ? "回复已中断，已收到的内容已保存。" : "";
        }
      } catch (cause) { if (!disposed) { state.error = errorMessage(cause); state.cancelling = false; } }
      publish();
    };
    actions.current = {
      send: async input => {
        if (disposed || !state.ready || submitting || syncing || state.active || !input.trim()) return false;
        submitting = true; cancelWanted = false; state.error = ""; state.notice = ""; publish();
        try {
          const receipt = await chatApi.submit(context, input);
          if (disposed) { void chatApi.cancel(receipt).catch(() => {}); return true; }
          state.active = receipt; save(receipt.round);
          submitting = false; replay(); publish();
          if (cancelWanted && state.active) await cancel();
          return true;
        } catch (cause) {
          if (!disposed) { state.error = errorMessage(cause); state.cancelling = false; }
          return false;
        } finally { submitting = false; buffered = []; publish(); }
      },
      cancel,
      reload: async () => { if (!unlisten) await connect(); else await synchronize(); },
      more: async () => {
        if (disposed || paging || !state.afterId) return;
        paging = true; state.loading = true; publish();
        try {
          const page = await sessionApi.messages(context, state.afterId);
          if (disposed) return;
          page.items.forEach(item => rows.set(item.id, item)); state.afterId = page.nextAfterId;
        } catch (cause) { if (!disposed) state.error = errorMessage(cause); }
        finally { paging = false; state.loading = false; publish(); }
      },
    };
    const receive = (event: ChatReplyEvent) => {
      if (disposed || !matches(event)) return;
      if (submitting || syncing) buffered.push(event);
      else if (state.active && event.requestId === state.active.requestId && event.seq > state.active.seq + 1) {
        buffered.push(event); void synchronize();
      } else apply(event);
    };
    const focus = () => { void synchronize(); };
    setView({ ...empty });
    const connect = async () => {
      if (!context.sessionId || connecting || disposed) return;
      connecting = true;
        try {
          const stop = await chatApi.listen(receive);
          if (disposed) { stop(); return; }
          unlisten = stop;
          window.addEventListener("focus", focus);
          await synchronize();
        } catch (cause) { if (!disposed) { state.error = errorMessage(cause); publish(); } }
        finally { connecting = false; }
    };
    void connect();
    return () => {
      disposed = true; unlisten?.(); window.removeEventListener("focus", focus);
      if (state.active) void chatApi.cancel(state.active).catch(() => {});
    };
  }, [context.personaId, context.sessionId, context.contextRevision]);
  return { ...view, send: (input: string) => actions.current.send(input), cancel: () => actions.current.cancel(),
    reload: () => actions.current.reload(), loadMore: () => actions.current.more() };
}
