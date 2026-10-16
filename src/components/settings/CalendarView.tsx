import { useEffect, useRef, useState } from "react";
import { errorMessage, festivalsApi } from "../../api";
import type { CalendarDay, CalendarMonth } from "../../types";
import "./CalendarView.css";

function localDate() {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth()+1).padStart(2,"0")}-${String(now.getDate()).padStart(2,"0")}`;
}
export default function CalendarView({ revision, onAdd, onEdit,canAdd=true }: {revision:string;onAdd:(date:string)=>void;onEdit:(id:string)=>void;canAdd?:boolean}) {
  const today = localDate();
  const [view, setView] = useState({year:new Date().getFullYear(),month:new Date().getMonth()+1});
  const [data, setData] = useState<CalendarMonth|null>(null);
  const [selected, setSelected] = useState(today);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [refresh, setRefresh] = useState(0);
  const generation = useRef(0);
  useEffect(() => {
    const token = ++generation.current;
    setLoading(true); setError(""); setData(null);
    void festivalsApi.month(view.year,view.month).then(value => {
      if (token===generation.current) setData(value);
    }).catch(cause => {if(token===generation.current)setError(errorMessage(cause));})
      .finally(()=>{if(token===generation.current)setLoading(false);});
    return ()=>{generation.current++;};
  }, [view,revision,refresh]);
  useEffect(() => {
    const reload = ()=>setRefresh(n=>n+1);
    const interval = window.setInterval(reload,60000);
    window.addEventListener("focus",reload);
    let disposed=false,stop:(()=>void)|undefined;
    void festivalsApi.listen(reload).then(fn=>{if(disposed)fn();else stop=fn;}).catch(()=>{});
    return ()=>{disposed=true;stop?.();window.clearInterval(interval);window.removeEventListener("focus",reload);};
  }, []);
  const shift=(amount:number)=>{
    const date=new Date(view.year,view.month-1+amount,1);
    if(date.getFullYear()<1950||date.getFullYear()>2100)return;
    setView({year:date.getFullYear(),month:date.getMonth()+1});
    setSelected(`${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,"0")}-01`);
  };
  const chosen=data?.days.find(d=>d.date===selected);
  const first=data?.days[0];
  const blanks=first?(new Date(`${first.date}T12:00:00`).getDay()+6)%7:0;
  return <section className="ym-page-card ym-calendar" aria-label="节日日历">
    <div className="ym-page-heading"><h2>日历</h2><button className="ym-button" onClick={()=>{setView({year:new Date().getFullYear(),month:new Date().getMonth()+1});setSelected(localDate());}}>回到今天</button></div>
    <div className="ym-calendar-toolbar"><button className="ym-button" aria-label="上个月" disabled={view.year===1950&&view.month===1} onClick={()=>shift(-1)}>‹</button>
      <label className="ym-calendar-month">月份<input aria-label="查看月份" type="month" min="1950-01" max="2100-12" value={`${view.year}-${String(view.month).padStart(2,"0")}`} onChange={e=>{const [year,month]=e.target.value.split("-").map(Number);if(year>=1950&&year<=2100&&month>=1&&month<=12){setView({year,month});setSelected(`${e.target.value}-01`);}}}/></label>
      <button className="ym-button" aria-label="下个月" disabled={view.year===2100&&view.month===12} onClick={()=>shift(1)}>›</button></div>
    {error&&<p role="alert" className="ym-page-error">{error}<button className="ym-button" onClick={()=>setRefresh(n=>n+1)}>重试</button></p>}
    <div className="ym-calendar-grid"><div className="ym-calendar-week">{["一","二","三","四","五","六","日"].map(day=><span key={day}>周{day}</span>)}</div>
      {loading?<p role="status">正在读取日历…</p>:<div className="ym-calendar-days">{Array.from({length:blanks},(_,i)=><span key={`blank-${i}`} />)}{data?.days.map(day=><button key={day.date} className="ym-calendar-day" aria-label={`${day.date} ${day.lunar} ${day.events.map(e=>e.name).join("、")}${day.solarTerm??""}`} aria-pressed={selected===day.date} data-today={day.date===today} data-holiday={day.events.length>0||!!day.solarTerm} onClick={()=>setSelected(day.date)}>
        <span className="ym-calendar-number">{Number(day.date.slice(-2))}</span><small className="ym-calendar-lunar">{day.lunar}</small>
        {(day.events.length>0||day.solarTerm)&&<span className="ym-calendar-marker" title={[...day.events.map(e=>e.name),day.solarTerm].filter(Boolean).join("、")}>{day.events[0]?.name??day.solarTerm}{day.events.length>1&&` +${day.events.length-1}`}</span>}
      </button>)}</div>}</div>
    {chosen&&<section className="ym-calendar-detail" aria-label="所选日期"><div className="ym-page-heading"><h3>{chosen.date} · {chosen.lunar}{chosen.solarTerm&&` · ${chosen.solarTerm}`}</h3><button className="ym-button" disabled={!canAdd} onClick={()=>onAdd(chosen.date)}>新增日期</button></div>
      {!chosen.events.length&&<p className="ym-page-note">这一天没有节日或会话纪念日。</p>}
      {chosen.events.map(e=><div className="ym-calendar-event" key={`${e.scope}:${e.id}`}><strong>{e.name}</strong><span className="ym-page-note">{e.scope==="catalog"?"内置节日":"自定义日期"}</span><p>{e.blessing}</p>{e.scope==="custom"&&<button className="ym-button" onClick={()=>onEdit(e.id)}>查看 / 编辑</button>}</div>)}
    </section>}
  </section>;
}
