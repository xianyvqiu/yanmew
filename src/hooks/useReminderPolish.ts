import { useEffect, useState } from "react";
import { reminderApi } from "../api";
import type { CurrentContext, ReminderOccurrence } from "../types";

/** 默认词立即显示；润色结果只属于相同原记录和当前人设，不影响投递或完成操作。 */
export function useReminderPolish(occurrence: ReminderOccurrence, context?: Pick<CurrentContext, "personaId" | "contextRevision">) {
  const personaId = context?.personaId;
  const revision = context?.contextRevision;
  const key = JSON.stringify([occurrence.id, occurrence.name, occurrence.content, occurrence.scheduledAt, personaId, revision]);
  const [result, setResult] = useState<{ key: string; text: string } | null>(null);
  useEffect(() => {
    let disposed = false;
    if (!personaId || revision === undefined) return;
    void reminderApi.polish(occurrence.id, personaId, revision).then(value => {
      if (!disposed && value?.occurrenceId === occurrence.id && value.personaId === personaId && value.contextRevision === revision && value.text) {
        setResult({ key, text: value.text });
      }
    }).catch(() => { /* 后台润色失败保持默认词，不把显示辅助错误当作投递失败。 */ });
    return () => { disposed = true; };
  }, [key, personaId, revision, occurrence.id]);
  return result?.key === key ? result.text : null;
}
