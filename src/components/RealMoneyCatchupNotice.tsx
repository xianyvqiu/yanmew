import { useEffect,useState } from "react";
import { errorMessage } from "../api";
import { navigation } from "../platform/navigation";
import { realMoneyApi,type PendingSummary } from "../realMoneyApi";
export function useRealMoneyNotice(){
    const[pending,setPending]=useState<PendingSummary>();const[dismissed,setDismissed]=useState<string|null>();const[error,setError]=useState("");
    useEffect(()=>{let disposed=false;let stop:(()=>void)|undefined;const read=()=>{void realMoneyApi.pending().then(v=>{if(!disposed)setPending(v);}).catch(()=>{});};void realMoneyApi.listen(read).then(fn=>{if(disposed)fn();else{stop=fn;read();}}).catch(()=>{});read();return()=>{disposed=true;stop?.();};},[]);
    return {visible:!!pending?.count&&pending.throughId!==dismissed,pending,error,dismiss:()=>setDismissed(pending?.throughId??null),open:()=>{void navigation.openRealMoney().then(()=>setDismissed(pending?.throughId??null)).catch(e=>setError(errorMessage(e)));}};
}
export default function RealMoneyCatchupNotice({notice}:{notice:ReturnType<typeof useRealMoneyNotice>}){
    return <div className="pet-state-bubble real-pet-notice" data-pet-interactive><strong>现实记账有 {notice.pending?.count} 项漏期</strong><p>是否需要补记？先查看日期和金额，再选择补记或跳过。</p>{notice.error&&<p role="alert">{notice.error}</p>}<div className="pet-bubble-actions"><button className="ym-button" onClick={notice.open}>查看并选择</button><button className="ym-button" onClick={notice.dismiss}>稍后</button></div></div>;
}
