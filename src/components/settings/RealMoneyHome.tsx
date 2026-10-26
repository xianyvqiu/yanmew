import {useEffect,useState} from "react";
import {emptyRealQuery,moneyText,realMoneyApi,type RealEntry,type RealOverview,type RealReport} from "../../realMoneyApi";
import RealMoneyBudgets from "./RealMoneyBudgets";
import type {RealRun} from "./RealMoneyForms";
export type MoneyModule="overview"|"bills"|"charts"|"assets"|"cycles"|"categories"|"calendar";
const modules:{id:MoneyModule;name:string;note:string;path:string}[]=[
  {id:"overview",name:"总览",note:"本月收支、预算与近期账单",path:"m3 11 9-8 9 8M5 10v11h14V10M9 21v-7h6v7"},
  {id:"bills",name:"账单",note:"查找和管理全部现实收支",path:"M6 3h12v18l-3-2-3 2-3-2-3 2V3Zm3 5h6M9 12h6"},
  {id:"charts",name:"统计",note:"打开账单数据面板",path:"M4 20h17M7 16v-5m5 5V4m5 12V8"},
  {id:"assets",name:"资产",note:"管理资金池与校准余额",path:"M3 6h17v15H3V6Zm0 0 14-3v3m-3 7h7v5h-7v-5Z"},
  {id:"cycles",name:"周期",note:"管理周期记账和漏期确认",path:"M20 8a8 8 0 0 0-14-3L3 8m0-5v5h5m-4 8a8 8 0 0 0 14 3l3-3m0 5v-5h-5"},
  {id:"categories",name:"分类",note:"整理收入与支出分类",path:"M3 3h7v7H3V3Zm11 0h7v7h-7V3ZM3 14h7v7H3v-7Zm11 0h7v7h-7v-7Z"},
  {id:"calendar",name:"日历",note:"按日期查看收支与计划",path:"M4 5h16v16H4V5ZM8 2v6m8-6v6M4 10h16"},
];
export default function RealMoneyHome({overview,busy,run,revision,onNavigate,onEntry}:{overview:RealOverview;busy:boolean;run:RealRun;revision:number;onNavigate:(tab:MoneyModule)=>void;onEntry:(entry:RealEntry)=>void}){
  const[recent,setRecent]=useState<RealReport>();const[error,setError]=useState("");
  useEffect(()=>{let disposed=false;const end=overview.today,start=new Date(`${end}T12:00:00`);start.setDate(start.getDate()-6);const pad=(n:number)=>String(n).padStart(2,"0");const from=`${start.getFullYear()}-${pad(start.getMonth()+1)}-${pad(start.getDate())}`;setError("");void realMoneyApi.query({...emptyRealQuery(),start:from,end}).then(v=>{if(!disposed)setRecent(v);}).catch(()=>{if(!disposed)setError("近期账单暂时无法读取，请到全部账单重试。");});return()=>{disposed=true;};},[revision,overview.today]);
  return <div className="real-home">
    <section className="real-home-hero" aria-label="本月收支"><span>{overview.today.slice(0,7).replace("-","年")}月 · 支出</span><strong>¥{moneyText(overview.monthSummary.expense)}</strong><div><span>收入 <b>¥{moneyText(overview.monthSummary.income)}</b></span><span>结余 <b>¥{moneyText(overview.monthSummary.net)}</b></span></div><small>总资产 ¥{moneyText(overview.totalAssetsCents)}</small></section>
    <RealMoneyBudgets overview={overview} busy={busy} run={run}/>
    <section className="ym-page-card real-today"><h3>今日收支</h3><strong>结余 ¥{moneyText(overview.todaySummary.net)}</strong><p className="ym-page-note">收入 ¥{moneyText(overview.todaySummary.income)} · 支出 ¥{moneyText(overview.todaySummary.expense)}</p></section>
    <section className="ym-page-card real-recent"><div className="ym-page-heading"><h3>近7天账单</h3><button type="button" className="ym-button" disabled={busy} onClick={()=>onNavigate("bills")}>全部账单</button></div>{error?<p role="alert">{error}</p>:!recent?<p role="status">正在读取近期账单…</p>:!recent.entries.length?<p className="ym-page-note">最近7天还没有记录。</p>:<ul>{recent.entries.slice(0,7).map(entry=><li key={entry.id}><button type="button" disabled={busy} onClick={()=>onEntry(entry)}><span><strong>{entry.title}</strong><small>{entry.occurredLocal.replace("T"," ")} · {entry.categoryName} · {entry.assetName}</small></span><b data-kind={entry.kind}>{entry.kind==="expense"?"−":entry.kind==="income"?"+":""}¥{moneyText(entry.amountCents)}</b></button></li>)}</ul>}</section>

  </div>;
}

export function RealMoneyNavigation({active,busy,pendingCount,onNavigate}:{active:MoneyModule;busy:boolean;pendingCount:number;onNavigate:(tab:MoneyModule)=>void}){
  return <nav className="real-home-dock" aria-label="现实记账模块">{modules.map(item=><div className="real-dock-item" key={item.id}><button type="button" className="ym-button" aria-label={item.name==="统计"?"数据面板":item.name==="周期"?"周期任务":item.name} aria-describedby={`money-tip-${item.id}`} aria-current={active===item.id?"page":undefined} disabled={busy} onClick={()=>onNavigate(item.id)}><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={item.path}/></svg><span>{item.name}</span>{item.id==="cycles"&&pendingCount>0&&<i aria-label={`${pendingCount}项待确认`}/>}</button><span className="real-dock-tooltip" role="tooltip" id={`money-tip-${item.id}`}>{item.note}</span></div>)}</nav>;
}
