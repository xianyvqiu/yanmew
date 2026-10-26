import { useEffect, useState } from "react";
import { reminderApi } from "../api";
import type { CurrentContext, ReminderOccurrence } from "../types";

/** 默认词立即显示；只读订阅后台已保存的文案，不由页面调用模型。 */
export function useReminderPolish(occurrence: ReminderOccurrence, context?: Pick<CurrentContext, "personaId" | "contextRevision">) {
  const personaId = context?.personaId;
  const revision = context?.contextRevision;
  const key = JSON.stringify([occurrence.id, occurrence.name, occurrence.content, occurrence.scheduledAt, personaId, revision]);
  const [result, setResult] = useState<{ key: string; text: string } | null>(null);
  useEffect(() => {
    let disposed=false,sequence=0;let stop:(()=>void)|undefined;
    if(!personaId||revision===undefined)return;
    const read=()=>{const request=++sequence;void reminderApi.polish(occurrence.id,personaId,revision).then(value=>{
      if(!disposed&&request===sequence&&value?.occurrenceId===occurrence.id&&value.personaId===personaId&&value.contextRevision===revision&&value.text)setResult({key,text:value.text});
    }).catch(()=>{});};
    read();void reminderApi.listen(read).then(fn=>{if(disposed)fn();else stop=fn;}).catch(()=>{});
    return()=>{disposed=true;sequence++;stop?.();};
  }, [key, personaId, revision, occurrence.id]);
  return result?.key === key ? result.text : null;
}
