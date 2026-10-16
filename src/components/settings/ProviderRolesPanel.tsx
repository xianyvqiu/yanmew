import {useEffect,useRef,useState} from "react";
import {configurationApi,errorMessage} from "../../api";
import type {Provider,ProviderRole,ProviderRoles} from "../../types";
const purposes:ProviderRole["purpose"][]=["chat","memory","image","vision"];
const names={chat:"聊天",memory:"记忆",image:"生图",vision:"识图"};

export default function ProviderRolesPanel({providers}:{providers:Provider[]}) {
  const [config,setConfig]=useState<ProviderRoles|null>(null);
  const [editing,setEditing]=useState<ProviderRole["purpose"]|null>(null);
  const [id,setId]=useState("");const [model,setModel]=useState("");const [enabled,setEnabled]=useState(false);
  const [output,setOutput]=useState("32768");const [capacity,setCapacity]=useState("");
  const [expected,setExpected]=useState(0);const [busy,setBusy]=useState(false);const [error,setError]=useState("");
  const alive=useRef(false),generation=useRef(0),lock=useRef(false);
  useEffect(()=>{
    alive.current=true;let disposed=false,stop:(()=>void)|undefined;
    const reload=()=>{const token=++generation.current;void configurationApi.roles().then(value=>{if(!disposed&&token===generation.current)setConfig(value);}).catch(cause=>{if(!disposed)setError(errorMessage(cause));});};
    void configurationApi.listenChanges(reload).then(fn=>{if(disposed)fn();else stop=fn;}).catch(()=>{});reload();
    window.addEventListener("focus",reload);
    return()=>{disposed=true;alive.current=false;generation.current++;stop?.();window.removeEventListener("focus",reload);};
  },[]);
  const edit=(purpose:ProviderRole["purpose"])=>{
    if(!config||busy)return;
    const role=config.roles.find(r=>r.purpose===purpose);
    const provider=providers.find(p=>p.id===role?.providerId)||providers.find(p=>purpose==="chat"?p.isActive:purpose==="memory"&&p.memoryIsActive);
    const stored=config.models?.find(m=>m.purpose===purpose&&m.providerId===provider?.id);
    setId(role?.providerId??provider?.id??"");setModel(stored?.model??role?.model??(provider?.model==="未配置"?"":provider?.model)??"");setEnabled(role?.enabled??false);
    setOutput(String(stored?.maxOutputTokens??provider?.maxOutputTokens??32768));setCapacity((stored?stored.contextWindowOverride:provider?.contextWindowOverride)==null?"":String(stored?stored.contextWindowOverride:provider?.contextWindowOverride));
    setExpected(config.revision);setEditing(purpose);setError("");
  };
  const save=async()=>{
    if(lock.current||!editing)return;
    if(editing==="chat"&&(!id||!model.trim())){setError("请选择连接并填写聊天模型名。");return;}
    if(id&&!model.trim()){setError("请填写该用途的模型名。");return;}
    if(editing!=="image"&&id&&(!/^\d+$/.test(output)||Number(output)<1||!Number.isSafeInteger(Number(output))||(capacity!==""&&(!/^\d+$/.test(capacity)||Number(capacity)<1)))){setError("最大输出和手动容量须为正整数，容量可留空。");return;}
    if(editing==="image"&&enabled&&!window.confirm("启用后，饰品准备图片可能产生服务商费用；即使未购买饰品也可能计费。喵币与API费用独立。确认开启？"))return;
    lock.current=true;setBusy(true);setError("");
    try{const value=await configurationApi.saveRole(editing,id||null,model,enabled,expected,editing!=="image"&&id?Number(output):undefined,editing!=="image"&&id?(capacity?Number(capacity):null):undefined);if(alive.current){generation.current++;setConfig(value);setEditing(null);}}
    catch(cause){if(alive.current)setError(errorMessage(cause));}
    finally{lock.current=false;if(alive.current)setBusy(false);}
  };
  return <section className="ym-page-card" aria-label="模型用途配置"><h2>模型用途</h2><p className="ym-page-note">连接只添加一次，各用途分别选择模型。一个连接或模型可以用于多个用途。</p>
    {error&&<p role="alert" className="ym-page-error">{error}</p>}
    <div className="ym-object-grid">{purposes.map(purpose=>{
      const role=config?.roles.find(r=>r.purpose===purpose);
      const provider=providers.find(p=>p.id===role?.providerId)||providers.find(p=>purpose==="chat"?p.isActive:purpose==="memory"&&p.memoryIsActive);
      return <article className="ym-object-card" key={purpose}><div className="ym-page-heading"><h3>{names[purpose]}</h3><button className="ym-button" disabled={!config||busy||editing!==null} onClick={()=>edit(purpose)}>配置{names[purpose]}</button></div>
        <p>{provider?.name??(purpose==="memory"||purpose==="vision"?"跟随聊天":"尚未配置")}</p><p className="ym-page-note">{role?.model??provider?.model??""}</p>
        {purpose==="image"&&<p className="ym-page-note">{role?.enabled?"已开启":"已关闭"} · 饰品失败保留图标、Emoji、颜文字或符号组合；创作失败保留旧图</p>}
        {purpose==="vision"&&<p className="ym-page-note">独立模型先理解图片，再交聊天模型回答。</p>}
        {editing===purpose&&<form onSubmit={event=>{event.preventDefault();void save();}}><fieldset disabled={busy} className="ym-role-fields"><label className="ym-field">提供商<select value={id} onChange={event=>{const next=event.target.value;setId(next);const p=providers.find(p=>p.id===next);const stored=config?.models?.find(m=>m.purpose===purpose&&m.providerId===next);setModel(stored?.model??(p?.model==="未配置"?"":p?.model??""));setOutput(String(stored?.maxOutputTokens??p?.maxOutputTokens??32768));const savedCapacity=stored?stored.contextWindowOverride:p?.contextWindowOverride;setCapacity(savedCapacity==null?"":String(savedCapacity));}}><option value="">{purpose==="memory"||purpose==="vision"?"跟随聊天":purpose==="image"?"不配置":"请选择"}</option>{providers.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
          {id&&<label className="ym-field">{names[purpose]}模型<input required value={model} onChange={event=>setModel(event.target.value)}/></label>}
          {purpose==="image"&&id&&<label><input type="checkbox" checked={enabled} onChange={event=>setEnabled(event.target.checked)}/>开启图片生成（饰品 / 桌宠创作）</label>}
          {purpose!=="image"&&id&&<details><summary>高级设置</summary><label className="ym-field">最大输出（tokens）<input type="number" required min={1} value={output} onChange={e=>setOutput(e.target.value)}/></label><label className="ym-field">上下文容量（自动时留空）<input type="number" min={1} value={capacity} onChange={e=>setCapacity(e.target.value)}/></label><p className="ym-page-note">默认最大输出32768，包含模型可能使用的思考内容；实际服务商或剩余容量更小时采用其允许值。表情、标题等短内容由提示词和校验约束，不会要求模型凑满上限。</p></details>}
          <div className="ym-inline-form"><button className="ym-button" type="button" onClick={()=>setEditing(null)}>取消</button><button className="ym-button" type="submit">{busy?"保存中…":"保存用途"}</button></div></fieldset></form>}
      </article>;
    })}</div>
  </section>;
}
