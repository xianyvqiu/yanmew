import { createContext, useContext, useState } from "react";
import { coinText, type LedgerGroup, type LedgerReport } from "../../ledgerApi";

export const LedgerDisplayContext=createContext({format:coinText,unit:"喵币",scale:1,sourceName:"小游戏"});
function useDisplay(){const display=useContext(LedgerDisplayContext);return {...display,signed:(value:string)=>`${BigInt(value)>0n?"+":""}${display.format(value)}`};}
export function LedgerOverview({report}: {report:LedgerReport}) {
    const {format,unit,signed}=useDisplay();
    return <div className="ym-ledger-overview">{[
        ["当前余额",format(report.balance),"wallet"], ["区间收入",`+${format(report.summary.income)}`,"income"],
        ["区间支出",`−${format(report.summary.expense)}`,"expense"], ["区间净变化",signed(report.summary.net),"net"],
    ].map(([label,value,kind])=><div className="ym-ledger-metric" key={kind} data-kind={kind}><span>{label}</span><strong>{value}<small> {unit}</small></strong></div>)}</div>;
}
function BarChart({report}:{report:LedgerReport}) {
    const {format,unit,signed,scale}=useDisplay();
    const [active,setActive]=useState<string>();
    const data=report.timeline;
    const peak=Math.max(1,...data.flatMap(item=>[Number(item.summary.income),Number(item.summary.expense)]));
    const count=data.length;
    const width=640, left=64, right=16, plot=width-left-right, step=plot/Math.max(1,count);
    const selected=data.find(item=>item.key===active);
    return <section className="ym-page-card ym-ledger-chart"><div className="ym-page-heading"><h3>收支趋势</h3><span className="ym-page-note">按{({day:"日",week:"周",month:"月",year:"年"} as Record<string,string>)[report.bucket]}汇总</span></div>
        <div className="ym-ledger-legend"><span data-kind="income">↑ 收入</span><span data-kind="expense">↓ 支出</span></div>
        {report.bucket!=="day"&&<p className="ym-page-note">首尾时段只统计所选日期范围内的流水。</p>}
        <svg viewBox="0 0 640 300" role="img" aria-label="收支柱状图：收入向上，支出向下"><title>收支趋势，收入向上、支出向下；下方提供分类金额明细。</title>
            {[50,150,250].map((y,i)=><g key={y}><line x1={left} x2={width-right} y1={y} y2={y} className="ym-ledger-gridline"/><text x={left-8} y={y+4} textAnchor="end" className="ym-ledger-axis">{i===1 ? "0" : `${i===0?"+":"−"}${(peak/scale).toLocaleString("zh-CN",{notation:"compact",maximumFractionDigits:scale===1?1:2})}`}</text></g>)}
            {data.map((item,index)=>{
                const x=left+step*index; const bar=Math.max(.5,Math.min(16,step*.32));
                return <g key={item.key} tabIndex={0} role="img" aria-label={`${item.name}起，收入${format(item.summary.income)}${unit}，支出${format(item.summary.expense)}${unit}`} onFocus={()=>setActive(item.key)} onBlur={()=>setActive(undefined)} onMouseEnter={()=>setActive(item.key)} onMouseLeave={()=>setActive(undefined)}>
                    <title>{item.name}起：收入 {format(item.summary.income)}，支出 {format(item.summary.expense)}，净变化 {signed(item.summary.net)}</title>
                    <rect x={x} y={35} width={step} height={220} fill="transparent"/>
                    <rect className="ym-ledger-bar ym-ledger-income" x={x+step/2-bar} y={150-Number(item.summary.income)/peak*100} width={bar} height={Number(item.summary.income)/peak*100} rx={Math.min(3,bar/2)}/>
                    <rect className="ym-ledger-bar ym-ledger-expense" x={x+step/2+1} y={150} width={bar} height={Number(item.summary.expense)/peak*100} rx={Math.min(3,bar/2)}/>
                    {(index===0||index===count-1||index%Math.max(1,Math.ceil(count/6))===0)&&<text x={x+step/2} y={280} textAnchor="middle" className="ym-ledger-axis">{report.bucket==="year"?item.name.slice(0,4):item.name.slice(5)}</text>}
                </g>;
            })}
        </svg><p className="ym-ledger-chart-hint" aria-live="polite">{selected?`${selected.name}起 · 收入 +${format(selected.summary.income)} · 支出 −${format(selected.summary.expense)} · 净变化 ${signed(selected.summary.net)}`:"悬停或用键盘选择柱子，查看该时段金额。"}</p>
    </section>;
}
function Donut({groups,direction,onSelect}:{groups:LedgerGroup[];direction:"income"|"expense";onSelect:(reason:string,direction:"income"|"expense")=>void}) {
    const {format,unit}=useDisplay();
    const values=groups.filter(g=>BigInt(g.summary[direction])>0n);
    const total=values.reduce((sum,g)=>sum+BigInt(g.summary[direction]),0n);
    let offset=0;
    const name=direction==="income"?"收入":"支出";
    return <section className="ym-page-card ym-ledger-chart"><h3>{name}构成</h3>{total===0n?<p className="ym-page-empty">这个范围还没有{name}记录</p>:<>
        <div className="ym-ledger-donut"><svg viewBox="0 0 180 180" role="img" aria-label={`${name}分类占比`}><circle cx="90" cy="90" r="64" className="ym-ledger-ring-track"/>{values.map((group,index)=>{
            const share=Number(group.summary[direction])/Number(total)*100; const at=offset;offset+=share;
            return <circle key={group.key} cx="90" cy="90" r="64" pathLength="100" strokeDasharray={`${share} ${100-share}`} strokeDashoffset={-at} className={`ym-ledger-ring ym-ledger-color-${index%6}`} transform="rotate(-90 90 90)"><title>{group.name}：{format(group.summary[direction])} ${unit}，占 {share.toFixed(1)}%</title></circle>;
        })}</svg><div><span>总{name}</span><strong>{format(total.toString())}</strong><small>{unit}</small></div></div>
        <div className="ym-ledger-breakdown">{values.map((g,i)=><button className="ym-ledger-category" key={g.key} aria-label={`${g.name}，${name}${format(g.summary[direction])}${unit}，占${(Number(g.summary[direction])/Number(total)*100).toFixed(1)}%，查看账单`} onClick={()=>onSelect(g.key,direction)}><i className={`ym-ledger-color-${i%6}`}/><span>{g.name}</span><strong>{format(g.summary[direction])}</strong><small>{(Number(g.summary[direction])/Number(total)*100).toFixed(1)}%</small></button>)}</div>
    </>}</section>;
}
export default function LedgerCharts({report,onSelect,onGame}:{report:LedgerReport;onSelect:(reason:string,direction:"income"|"expense")=>void;onGame:(id:string)=>void}) {
    const {format,signed,sourceName}=useDisplay();
    return <div className="ym-ledger-dashboard"><BarChart report={report}/><div className="ym-ledger-chart-pair"><Donut groups={report.categories} direction="income" onSelect={onSelect}/><Donut groups={report.categories} direction="expense" onSelect={onSelect}/></div>
        <section className="ym-page-card"><h3>分类汇总 <small>· {report.summary.count} 笔</small></h3><div className="ym-ledger-summary-list">{report.categories.length===0?<p className="ym-page-empty">没有符合条件的流水</p>:report.categories.map(g=><div className="ym-ledger-summary-row" key={g.key}><strong>{g.name}</strong><span>收入 +{format(g.summary.income)}</span><span>支出 −{format(g.summary.expense)}</span><small>{g.summary.count} 笔</small></div>)}</div></section>
        <section className="ym-page-card"><h3>{sourceName}收支排行</h3><p className="ym-page-note">按收入与支出总额排序；点击查看该{sourceName}账单。</p>{report.games.length===0?<p className="ym-page-empty">这个范围没有{sourceName}流水</p>:[...report.games].sort((a,b)=>{const x=BigInt(a.summary.income)+BigInt(a.summary.expense),y=BigInt(b.summary.income)+BigInt(b.summary.expense);return x===y?0:x>y?-1:1;}).map((g,i)=><button className="ym-ledger-game" key={g.key} onClick={()=>onGame(g.key)}><span>{i+1}</span><strong>{g.name}</strong><span>收入 +{format(g.summary.income)} · 支出 −{format(g.summary.expense)}</span><small>净变化 {signed(g.summary.net)}</small></button>)}</section>
    </div>;
}
