import type {ChatImageInput} from "../types";
import { useEffect, useRef, useState } from "react";
import { chatApi, errorMessage, sessionApi } from "../api";
import type { ChatContextBudget, ChatReplyEvent, ChatRequestSnapshot, CurrentContext, Message, RoundSnapshot } from "../types";

type ChatView = {
  messages: Message[]; active: ChatRequestSnapshot | null; busy: boolean; cancelling: boolean;
  ready: boolean; loading: boolean; afterId: string | null; error: string; notice: string;
  lastRound: RoundSnapshot | null; budget: ChatContextBudget | null;
};
const empty: ChatView = { messages: [], active: null, busy: false, cancelling: false, ready: false, loading: false, afterId: null, error: "", notice: "", lastRound: null, budget: null };
const interruptedNotice = (round: RoundSnapshot) => {
  if (round.round.status !== "interrupted") return "";
  switch (round.round.stopReason) {
    case "length_limit": return "已达到本次输出上限，已收到的内容已保存。";
    case "repetition": return "检测到持续重复，回复已停止，已收到的内容已保存。";
    case "invalid_encoding": return "检测到异常编码，回复已停止，已收到的内容已保存。";
    case "timeout": return "回复超时，已收到的内容已保存。";
    case "tool_limit": return "工具调用达到上限或持续无进展，回复已停止，已收到的内容已保存。";
    default: return "回复已中断，已收到的内容已保存。";
  }
};

/** 页面只渲染后端保存结果；未保存增量单独展示，不另发消息写入命令。 */
export function useChat(context: CurrentContext, options: { onSubmitted?: (receipt: ChatRequestSnapshot) => void } = {}) {
  const submittedCallback = useRef(options.onSubmitted);
  submittedCallback.current = options.onSubmitted;
  const [view, setView] = useState<ChatView>(empty);
  const actions = useRef({ send: async (_input: string,_images?:ChatImageInput[]): Promise<boolean> => false, continueReply: async (): Promise<boolean> => false,
    regenerateReply: async (): Promise<boolean> => false, editMessage: async (_message:Message,_content:string): Promise<boolean> => false,
    deleteTail: async (_message:Message): Promise<boolean> => false, cancel: async () => {}, reload: async () => {}, more: async () => {} });
  useEffect(() => {
    let disposed = false;
    let state = { ...empty };
    let submitting = false;
    let syncing = false;
    let connecting = false;
    let paging = false;
    let mutating = false;
    let loadedPages = 1;
    let historyGeneration = 0;
    let cancelWanted = false;
    let buffered: ChatReplyEvent[] = [];
    let unlisten: (() => void) | undefined;
    let unlistenMessages: (() => void) | undefined;
    const rows = new Map<string, Message>();
    const ended = new Set<string>();
    const ownedRequests = new Set<string>();
    const publish = () => {
      if (disposed) return;
      state = { ...state, messages: [...rows.values()].sort((a, b) => {
        if (/^\d+$/.test(a.id) && /^\d+$/.test(b.id)) return BigInt(a.id) < BigInt(b.id) ? -1 : a.id === b.id ? 0 : 1;
        return a.createdAt.localeCompare(b.createdAt) || a.id.localeCompare(b.id);
      }), busy: submitting || mutating || state.active !== null };
      setView(state);
    };
    const save = (round: RoundSnapshot) => {
      historyGeneration++;
      rows.set(round.userMessage.id, round.userMessage);
      if (round.assistantMessage) rows.set(round.assistantMessage.id, round.assistantMessage);
    };
    const matches = (event: ChatReplyEvent) => event.personaId === context.personaId
      && event.sessionId === context.sessionId && event.contextRevision === context.contextRevision;
    const refreshMessages = async () => {
      const token = ++historyGeneration;
      const fresh = new Map<string,Message>();
      let cursor:string|null=null;
      try {
        for(let index=0;index<loadedPages;index++) {
          const page: import("../types").MessagePage=await (cursor ? sessionApi.messages(context,cursor) : sessionApi.messages(context));
          if(disposed || token !== historyGeneration) return null;
          page.items.forEach(item=>fresh.set(item.id,item));
          cursor=page.nextAfterId;
          if(!cursor) break;
        }
      } catch (cause) {
        if (disposed || token !== historyGeneration) return null;
        throw cause;
      }
      // 消息写入或较新查询使本次快照失效，不能覆盖已编辑/删除的内容。
      if (disposed || token !== historyGeneration) return null;
      rows.clear();fresh.forEach((item,id)=>rows.set(id,item));
      state.afterId=cursor;
      publish();
      return token;
    };
    const apply = (event: ChatReplyEvent) => {
      const active = state.active;
      if (ended.has(event.requestId)) return;
      if (active && (event.requestId !== active.requestId || event.roundId !== active.roundId || event.seq <= active.seq)) return;
      if (!active && (event.type === "text" || event.type === "expression" || event.type === "context_notice" || event.type === "web_activity")) return;
      if (active) state.active = { ...active, seq: event.seq };
      if (event.type === "text" && state.active) state.active.partialContent += event.text;
      else if (event.type === "expression" && state.active) state.active.expression = event.expression;
      else if (event.type === "web_activity" && state.active) state.active.webActivity = event.phase;
      else if (event.type === "context_notice" && state.active) {
        state.active = { ...state.active, notice: event.message, budget: event.budget };
        state.notice = event.message; state.budget = event.budget;
      }
      else {
        ended.add(event.requestId);
        if (event.saved) save(event.saved);
        state.lastRound = event.saved;
        state.active = null; state.cancelling = false; cancelWanted = false;
        if (event.type === "failed") state.error = event.message;
        state.notice = [active?.notice, event.saved ? interruptedNotice(event.saved) : ""].filter(Boolean).join(" ");
        if(active?.regenerating) state.notice=event.type==="finished" ? "重说回复和记忆已一起保存。" : "重说已停止，原回复和记忆保留。";
        if (!event.saved) state.notice = "回复未能确认保存，请重新加载查看记录。";
        const generation = historyGeneration + 1;
        void (async () => {
          const token = await refreshMessages();
          if (token === null) return;
          const lastRound = await chatApi.lastRound(context);
          if (disposed || token !== historyGeneration || state.active || submitting || mutating) return;
          state.lastRound = lastRound;
          if (lastRound) save(lastRound);
          publish();
        })().catch(cause => { if (!disposed && generation === historyGeneration) { state.error = errorMessage(cause); publish(); } });
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
      if (disposed || syncing || submitting || mutating || !context.sessionId) return;
      syncing = true; state.loading = true; publish();
      try {
        // 查询期间事件暂存；快照 seq 之前的增量在重放时自动忽略。
        const previous = state.active;
        const active = await chatApi.current(context);
        const completed = !active && previous ? await chatApi.cancel(previous) : null;
        const lastRound = active ? active.round : await chatApi.lastRound(context);
        const token = await refreshMessages();
        if (disposed || token === null || token !== historyGeneration) return;
        state.active = active;
        state.lastRound = lastRound;
        if (lastRound) save(lastRound);
        if (active) {
          state.notice = active.notice ?? "";
          state.budget = active.budget ?? null;
        }
        else {
          state.cancelling = false;
          if (completed) {
            if (previous) ended.add(previous.requestId);
            save(completed);
            state.notice = [previous?.notice, interruptedNotice(completed)].filter(Boolean).join(" ");
          } else if (lastRound) {
            state.notice = interruptedNotice(lastRound);
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
        if (saved.round.status !== "pending" && !active.regenerating) {
          ended.add(active.requestId);
          save(saved); state.lastRound = saved; state.active = null; state.cancelling = false;
          state.notice = [active.notice, interruptedNotice(saved)].filter(Boolean).join(" ");
        }
      } catch (cause) { if (!disposed) { state.error = errorMessage(cause); state.cancelling = false; } }
      publish();
    };
    const submitRequest = async (request: () => Promise<ChatRequestSnapshot>) => {
        if (disposed || !state.ready || submitting || syncing || mutating || paging || state.active) return false;
        historyGeneration++;
        submitting = true; cancelWanted = false; state.error = ""; state.notice = ""; state.budget = null; publish();
        try {
          const receipt = await request();
          if (disposed) { void chatApi.cancel(receipt).catch(() => {}); return true; }
          ownedRequests.add(receipt.requestId);
          submittedCallback.current?.(receipt);
          state.active = receipt; state.lastRound = receipt.round; save(receipt.round);
          state.notice = receipt.notice ?? ""; state.budget = receipt.budget ?? null;
          submitting = false; replay(); publish();
          if (cancelWanted && state.active) await cancel();
          return true;
        } catch (cause) {
          if (!disposed) { state.error = errorMessage(cause); state.cancelling = false; }
          return false;
        } finally { submitting = false; buffered = []; publish(); }
    };
    actions.current = {
      send: async (input,images=[]) => input.trim() || images.length ? submitRequest(() => images.length?chatApi.submit(context,input,images):chatApi.submit(context,input)) : false,
      continueReply: async () => {
        const lastRound = state.lastRound;
        if (lastRound?.round.status !== "interrupted" || lastRound.round.stopReason !== "length_limit" || !lastRound.assistantMessage) return false;
        return submitRequest(() => chatApi.continue(context, lastRound.round.id));
      },
      regenerateReply: async () => {
        if(state.afterId || state.lastRound?.round.status!=="completed") return false;
        return submitRequest(()=>chatApi.regenerate(context,state.lastRound!.round.id));
      },
      editMessage: async (message,content) => {
        if(disposed || !state.ready || syncing || submitting || mutating || paging || state.active || !content.trim()) return false;
        historyGeneration++;
        mutating=true;state.error="";publish();
        let saved=false;
        try {
          const result=await chatApi.edit(context,message,content);
          saved=true;
          if(disposed) return true;
          historyGeneration++;
          rows.set(result.id,result);
          state.lastRound=await chatApi.lastRound(context);
          if(disposed) return true;
          if(state.lastRound) save(state.lastRound);
          state.notice="消息已保存；已有记忆不自动变化，可在记忆窗口手动修改。";
        } catch(cause) {if(!disposed) state.error=saved ? `消息已保存，但刷新失败：${errorMessage(cause)}` : errorMessage(cause);}
        finally {mutating=false;replay();publish();}
        return saved;
      },
      deleteTail: async message => {
        if(disposed || !state.ready || syncing || submitting || mutating || paging || state.afterId) return false;
        historyGeneration++;
        mutating=true;state.error="";
        const active=state.active;
        if(active) ended.add(active.requestId);
        publish();
        let saved=false;
        try {
          await chatApi.deleteTail(context,message);
          saved=true;
          if(disposed) return true;
          if(active && state.active?.requestId===active.requestId) {state.active=null;state.cancelling=false;cancelWanted=false;}
          state.lastRound=null;
          await refreshMessages();
          const last=await chatApi.lastRound(context);
          if(disposed) return true;
          state.lastRound=last;if(last) save(last);
          state.notice="底部消息已删除。";
        } catch(cause) {if(!disposed) {state.error=saved ? `消息已删除，但刷新失败：${errorMessage(cause)}` : errorMessage(cause);if(!saved && active) ended.delete(active.requestId);}}
        finally {mutating=false;replay();publish();}
        return saved;
      },
      cancel,
      reload: async () => { if (!unlisten) await connect(); else await synchronize(); },
      more: async () => {
        if (disposed || paging || !state.afterId) return;
        const token = ++historyGeneration;
        paging = true; state.loading = true; publish();
        try {
          const page = await sessionApi.messages(context, state.afterId);
          if (disposed || token !== historyGeneration) return;
          page.items.forEach(item => rows.set(item.id, item)); state.afterId = page.nextAfterId;loadedPages++;
        } catch (cause) { if (!disposed && token === historyGeneration) state.error = errorMessage(cause); }
        finally { paging = false; state.loading = false; publish(); }
      },
    };
    const receive = (event: ChatReplyEvent) => {
      if (disposed || !matches(event) || ended.has(event.requestId)) return;
      if (submitting || syncing || mutating) buffered.push(event);
      else if (state.active && event.requestId === state.active.requestId && event.seq > state.active.seq + 1) {
        buffered.push(event); void synchronize();
      } else if (!state.active && ["text", "expression"].includes(event.type)) {
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
          const stopMessages=await chatApi.listenMessages(change=>{if(change.sessionId===context.sessionId) void synchronize();});
          if(disposed) {stopMessages();return;}
          unlistenMessages=stopMessages;
          window.addEventListener("focus", focus);
          await synchronize();
        } catch (cause) { if (!disposed) { state.error = errorMessage(cause); publish(); } }
        finally { connecting = false; }
    };
    void connect();
    return () => {
      disposed = true; unlisten?.(); unlistenMessages?.(); window.removeEventListener("focus", focus);
      // 旁观窗口卸载不取消其他入口发起的回复。
      if (state.active && ownedRequests.has(state.active.requestId)) void chatApi.cancel(state.active).catch(() => {});
    };
  }, [context.personaId, context.sessionId, context.contextRevision]);
  return { ...view, send: (input: string,images?:ChatImageInput[]) => actions.current.send(input,images), continueReply: () => actions.current.continueReply(), cancel: () => actions.current.cancel(),
    regenerateReply: () => actions.current.regenerateReply(),editMessage: (message:Message,content:string)=>actions.current.editMessage(message,content),deleteTail:(message:Message)=>actions.current.deleteTail(message),
    reload: () => actions.current.reload(), loadMore: () => actions.current.more() };
}
