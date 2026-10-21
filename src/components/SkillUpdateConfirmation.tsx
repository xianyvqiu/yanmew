import type {useSkillUpdate} from "../hooks/useSkillUpdate";
import "./SkillUpdateConfirmation.css";
export default function SkillUpdateConfirmation({update,pet=false}:{update:ReturnType<typeof useSkillUpdate>;pet?:boolean}){
 const {preview,error,busy}=update;
 if(!preview&&!error)return null;
 return <section className={`ym-skill-confirmation${pet?" pet-chat-bubble":""}`} aria-label="共享Skill修改确认" data-pet-interactive>
  <div className="ym-skill-confirmation__body">
   {preview&&<><h3>确认修改共享 Skill</h3><p>将影响：{preview.before.personas.map(p=>p.name).join("、")}。尚未保存。</p>
    <details><summary>查看原文：{preview.before.title}</summary><p>{preview.before.summary}</p><pre>{preview.before.body}</pre></details>
    <h4>拟修改为：{preview.proposed.title}</h4><p>{preview.proposed.summary}</p><pre>{preview.proposed.body}</pre></>}
   {error&&<p role="alert">{error}<button className="ym-button" disabled={busy} onClick={()=>void update.refresh()}>刷新预览</button></p>}
  </div>
  {preview&&<div className="ym-skill-confirmation__actions"><button className="ym-button" disabled={busy} onClick={()=>void update.resolve(preview.id,true)}>确认修改共享原文</button><button className="ym-button" disabled={busy} onClick={()=>void update.resolve(preview.id,false)}>取消</button></div>}
 </section>;
}
