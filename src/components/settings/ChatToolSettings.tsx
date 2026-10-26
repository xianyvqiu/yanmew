import {useEffect,useId,useRef,useState} from "react";
import {configurationApi,errorMessage} from "../../api";

export default function ChatToolSettings(){
  const id=useId(),alive=useRef(false),locked=useRef(false);
  const [draft,setDraft]=useState<string|null>(null),[baseline,setBaseline]=useState("");
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[notice,setNotice]=useState("");
  const [attempt,setAttempt]=useState(0);
  useEffect(()=>{
    alive.current=true;let disposed=false;setError("");
    void configurationApi.getSettings().then(value=>{if(!disposed){setDraft(value.tool_call_rounds);setBaseline(value.tool_call_rounds);}}).catch(cause=>{if(!disposed)setError(errorMessage(cause));});
    return()=>{disposed=true;alive.current=false;};
  },[attempt]);
  const valid=draft!==null&&/^[1-9]\d*$/.test(draft)&&Number(draft)<=2147483647;
  const save=async()=>{
    if(!valid||draft===null||draft===baseline||locked.current)return;
    locked.current=true;setBusy(true);setError("");setNotice("");
    try{
      const saved=await configurationApi.saveSettings({tool_call_rounds:draft});
      if(alive.current){setDraft(saved.tool_call_rounds);setBaseline(saved.tool_call_rounds);setNotice("工具往返次数已保存，对之后开始的聊天生效。");}
    }catch(cause){if(alive.current)setError(errorMessage(cause));}
    finally{locked.current=false;if(alive.current)setBusy(false);}
  };
  return <section className="ym-page-card" aria-labelledby={`${id}-title`}>
    <h2 id={`${id}-title`}>聊天工具</h2>
    <p className="ym-page-note">允许 AI 按需查找资料、读取 Skill、查看天气、切换表情或执行已开放的操作。默认每次聊天最多30轮工具往返，不会为了用完次数而强制调用。</p>
    {draft!==null?<form onSubmit={event=>{event.preventDefault();void save();}}>
      <label className="ym-field" htmlFor={`${id}-rounds`}>每次聊天最多工具往返次数<input id={`${id}-rounds`} type="text" inputMode="numeric" value={draft} disabled={busy} aria-invalid={!valid} aria-describedby={`${id}-help`} onChange={event=>{setDraft(event.target.value);setNotice("");setError("");}}/></label>
      <p id={`${id}-help`} className="ym-page-note">AI 请求工具、颜喵返回结果，算一轮；同一轮可以调用多个工具。所有工具共用此次数，重复或持续无进展仍会停止。请输入1～2147483647的整数。</p>
      {!valid&&<p className="ym-page-error" role="alert">请输入范围内的正整数，不含小数、符号或前导零。</p>}
      <button className="ym-button" data-variant="primary" type="submit" disabled={busy||!valid||draft===baseline}>{busy?"正在保存…":"保存工具设置"}</button>
    </form>:!error&&<p role="status">正在读取工具设置…</p>}
    {error&&<p role="alert" className="ym-page-error">{error}{draft===null&&<button className="ym-button" onClick={()=>setAttempt(value=>value+1)}>重新读取</button>}</p>}
    {notice&&<p role="status">{notice}</p>}
  </section>;
}
