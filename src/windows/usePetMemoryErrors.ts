import { useEffect, useState } from "react";
import { errorMessage, memoryGenerationApi } from "../api";
import type { PetActivity } from "../types";

/** 只读当前会话的后台失败；旧上下文和迟到查询不覆盖新会话，不改变桌宠健康状态。 */
export function usePetMemoryErrors(activity: PetActivity | null) {
  const personaId = activity?.personaId;
  const sessionId = activity?.sessionId;
  const contextRevision = activity?.contextRevision;
  const owner = `${personaId}:${sessionId}:${contextRevision}`;
  const [state, setState] = useState<{ owner: string; messages: string[] }>({ owner: "", messages: [] });
  useEffect(() => {
    if (!personaId || !sessionId || contextRevision === undefined) return;
    let disposed = false;
    let generation = 0;
    let unlisten: (() => void) | undefined;
    const read = async () => {
      const token = ++generation;
      try {
        const view = await memoryGenerationApi.get({ personaId, sessionId, contextRevision });
        if (disposed || token !== generation || !view || view.sessionId !== sessionId) return;
        const messages = [];
        if (view.error) messages.push(`短期记忆生成失败：${view.error.message}`);
        if (view.longError) messages.push(`长期记忆整合失败：${view.longError.message}`);
        setState({ owner, messages });
      } catch (cause) {
        if (disposed || token !== generation || (cause as { code?: string })?.code === "CONTEXT_CHANGED") return;
        setState({ owner, messages: [`记忆状态读取失败：${errorMessage(cause)}`] });
      }
    };
    void memoryGenerationApi.listen(() => { if (!disposed) void read(); }).then(stop => {
      if (disposed) stop();
      else { unlisten = stop; void read(); }
    }).catch(cause => {
      if (!disposed) { setState({ owner, messages: [`记忆状态监听失败：${errorMessage(cause)}`] }); void read(); }
    });
    window.addEventListener("focus", read);
    return () => { disposed = true; generation++; unlisten?.(); window.removeEventListener("focus", read); };
  }, [personaId, sessionId, contextRevision, owner]);
  return state.owner === owner && sessionId ? state.messages : [];
}
