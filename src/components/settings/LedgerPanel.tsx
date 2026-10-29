import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { errorMessage, walletApi } from "../../api";
import { coinText, ledgerApi, ledgerRange, ledgerReasons, type LedgerQuery, type LedgerReport } from "../../ledgerApi";
import LedgerCharts, { LedgerOverview } from "./LedgerCharts";
import MotionRegion from "../MotionRegion";
import "./LedgerPanel.css";

const presets: Record<string,string>={all:"全部时间",yesterday:"昨日",week:"最近7天",month:"最近1个月",halfYear:"最近6个月",year:"最近1年",custom:"自定义"};
export default function LedgerPanel() {
    const [query,setQuery]=useState<LedgerQuery>(()=>({...ledgerRange("all"),direction:"all",reason:"",pluginId:"",search:"",ascending:false,page:1}));
    const [preset,setPreset]=useState("all");
    const [dates,setDates]=useState({start:"",end:""});
    const [keyword,setKeyword]=useState("");
    const [report,setReport]=useState<LedgerReport>();
    const [view,setCurrentView]=useState<"home"|"bills"|"charts">("home");
    const dashboard=view==="charts";
    const [filtersOpen,setFiltersOpen]=useState(false);
    const [expanded,setExpanded]=useState<string>();
    const [loading,setLoading]=useState(true);
    const [error,setError]=useState("");
    const [refresh,setRefresh]=useState(0);
    const listScroll=useRef(0);
    const region=useRef<HTMLDivElement>(null);
    useLayoutEffect(()=>{
        const container=region.current?.closest(".ym-window-shell__content");
        if(container)container.scrollTop=view==="bills"?listScroll.current:0;
    },[view]);
    useEffect(()=>{
        let disposed=false;
        const reload=()=>{if(!disposed){
            if(preset!=="custom"){
                const range=ledgerRange(preset);
                setQuery(q=>q.start===range.start&&q.end===range.end?q:{...q,...range,page:1});
                setDates({start:range.start??"",end:range.end??""});
            }
            setRefresh(v=>v+1);
        }};
        let stop:(()=>void)|undefined;
        void walletApi.listen(reload).then(fn=>{if(disposed)fn();else{stop=fn;reload();}}).catch(()=>{});
        window.addEventListener("focus",reload);
        return()=>{disposed=true;stop?.();window.removeEventListener("focus",reload);};
    },[preset]);
    useEffect(()=>{
        let disposed=false;
        setLoading(true);setError("");
        void ledgerApi.query(query).then(value=>{
            if(disposed)return;
            if(query.page>value.totalPages){setQuery(q=>({...q,page:value.totalPages}));return;}
            setReport(value);setLoading(false);
        }).catch(cause=>{if(!disposed){setError(errorMessage(cause));setLoading(false);}});
        return()=>{disposed=true;};
    },[query,refresh]);
    const filter=(patch:Partial<LedgerQuery>)=>{setExpanded(undefined);setQuery(q=>({...q,...patch,page:1}));};
    const setView=(next:typeof view)=>{
        const container=region.current?.closest(".ym-window-shell__content");
        if(container&&view==="bills")listScroll.current=container.scrollTop;
        setFiltersOpen(false);setCurrentView(next);
    };
    const drill=(patch:Partial<LedgerQuery>)=>{filter(patch);listScroll.current=0;setView("bills");};
    const selectedRange=report?`${report.rangeStart} — ${report.rangeEnd}`:"正在读取日期范围";
    return <div ref={region} className="ym-ledger" aria-label="全局喵币账单">
        <div className="ym-page-heading"><div><h2>{view==="home"?"喵币账单":dashboard?"账单数据面板":"账单明细"}</h2><p className="ym-page-note">全局喵币{view!=="home"&&report&&<>{!dashboard&&<> · 当前余额 <span>{coinText(report.balance)}</span></>} · {report.summary.count} 笔符合条件的流水</>}</p></div><div className="ym-inline-form"><button className="ym-button" onClick={()=>setRefresh(v=>v+1)} disabled={loading}>刷新</button>{view!=="home"&&<><button className="ym-button" onClick={()=>setView("home")}>返回喵币账单</button><button className="ym-button" data-variant="primary" onClick={()=>setView(dashboard?"bills":"charts")}>{dashboard?"返回账单":"账单数据面板"}</button></>}</div></div>
        {view==="home"&&<MotionRegion motionKey="ledger-home"><div className="ym-finance-home"><section className="ym-finance-balance" aria-label="喵币钱包"><span>当前余额</span><strong>{report?coinText(report.balance):"—"}<small> 喵币</small></strong><p className="ym-page-note">所有人设共用的钱包</p></section><nav className="ym-finance-modules" aria-label="喵币账单模块">{[["bills","账单明细","查找每一笔收入与支出"],["charts","账单数据面板","查看收支趋势、分类占比和小游戏排行"]].map(([key,title,note])=><button key={key} className="ym-finance-module" aria-label={title} onClick={()=>setView(key as "bills"|"charts")}><span><strong>{title}</strong><small>{note}</small></span><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true"><path d="m9 5 7 7-7 7"/></svg></button>)}</nav></div></MotionRegion>}
        {view!=="home"&&<><div className="ym-finance-query-heading"><p className="ym-page-note">{selectedRange} · {presets[preset]}</p><button className="ym-button" aria-expanded={filtersOpen} aria-controls="coin-ledger-filters" onClick={()=>setFiltersOpen(v=>!v)}>{filtersOpen?"收起筛选":"筛选账单"}</button></div>
        <div id="coin-ledger-filters" hidden={!filtersOpen}><section className="ym-page-card ym-ledger-filters" aria-label="账单筛选"><div className="ym-ledger-presets">{Object.entries(presets).map(([key,name])=><button className="ym-button" aria-pressed={preset===key} key={key} onClick={()=>{
            setPreset(key);if(key!=="custom"){const range=ledgerRange(key);setDates({start:range.start??"",end:range.end??""});filter(range);}
        }}>{name}</button>)}</div>
        {preset==="custom"&&<form className="ym-inline-form" onSubmit={e=>{e.preventDefault();if(dates.start&&dates.end&&dates.start>dates.end){setError("开始日期不能晚于结束日期");return;}filter({start:dates.start||null,end:dates.end||null});}}><label className="ym-field">开始日期<input type="date" required value={dates.start} onChange={e=>setDates(d=>({...d,start:e.target.value}))}/></label><label className="ym-field">结束日期<input type="date" required value={dates.end} onChange={e=>setDates(d=>({...d,end:e.target.value}))}/></label><button className="ym-button" type="submit">应用日期</button></form>}
        <div className="ym-ledger-filter-grid"><label className="ym-field">收支<select value={query.direction} onChange={e=>filter({direction:e.target.value as LedgerQuery["direction"]})}><option value="all">全部流水</option><option value="income">收入</option><option value="expense">支出</option></select></label>
        <label className="ym-field">分类<select value={query.reason} onChange={e=>filter({reason:e.target.value,pluginId:""})}><option value="">全部分类</option>{Object.entries(ledgerReasons).map(([key,name])=><option key={key} value={key}>{name}</option>)}</select></label>
        <label className="ym-field">小游戏<select disabled={!!query.reason&&query.reason!=="plugin_game"} value={query.pluginId} onChange={e=>filter({pluginId:e.target.value,reason:e.target.value?"plugin_game":query.reason})}><option value="">全部小游戏</option>{report?.plugins.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></label>
        <label className="ym-field">时间排序<select value={query.ascending?"asc":"desc"} onChange={e=>filter({ascending:e.target.value==="asc"})}><option value="desc">最新优先</option><option value="asc">最早优先</option></select></label></div>
        <form className="ym-inline-form" onSubmit={e=>{e.preventDefault();filter({search:keyword});}}><label className="ym-field">查询账单<input type="search" value={keyword} placeholder="分类、游戏名称、说明或金额" onChange={e=>setKeyword(e.target.value)}/></label><button type="submit" className="ym-button">查询</button>{query.search&&<button type="button" className="ym-button" onClick={()=>{setKeyword("");filter({search:""});}}>清除查询</button>}</form>
        <p className="ym-page-note">{preset==="all"?"全部时间 · 已有流水范围：":"统计日期："}{selectedRange} · 按电脑当前时区 · 零变动只计笔数</p></section></div></>}
        <div role="status" className="ym-ledger-status">{loading?"正在更新，保留上次查询内容…":error?"本次查询未成功，下方保留上次结果。":"查询完成"}</div>
        {error&&<p className="ym-page-error" role="alert">{error}<button className="ym-button" onClick={()=>setRefresh(v=>v+1)}>重试</button></p>}
        {report&&view!=="home"&&<div ref={element=>{element?.toggleAttribute("inert",loading||!!error);}} className="ym-ledger-results" aria-busy={loading}><MotionRegion motionKey={`${view}:${report.page}:${report.rangeStart}:${report.rangeEnd}:${report.summary.count}:${report.summary.net}`}>
            {dashboard?<div className="ym-ledger-dashboard"><LedgerOverview report={report}/><LedgerCharts report={report} onSelect={(reason,direction)=>drill({reason,direction})} onGame={pluginId=>drill({pluginId,reason:"plugin_game"})}/></div>:<>

            <div className="ym-ledger-list">{report.entries.length===0?<p className="ym-page-empty">没有符合条件的账单，请调整筛选。</p>:report.entries.map(entry=>{
                const delta=BigInt(entry.delta),kind=delta>0n?"income":delta<0n?"expense":"zero";
                return <article className="ym-page-card ym-ledger-entry" data-kind={kind} key={entry.id}><button className="ym-ledger-entry-main" aria-expanded={expanded===entry.id} onClick={()=>setExpanded(expanded===entry.id?undefined:entry.id)}><span className="ym-ledger-entry-icon" aria-hidden="true">{kind==="income"?"↗":kind==="expense"?"↘":"—"}</span><span className="ym-ledger-entry-label"><strong>{entry.pluginName??ledgerReasons[entry.reason]??"其他"}</strong><span>{entry.description??ledgerReasons[entry.reason]} · {new Date(entry.createdAt).toLocaleString("zh-CN")}</span></span><span className="ym-ledger-entry-amount"><strong>{delta>0n?"+":""}{coinText(entry.delta)}</strong><small>{kind==="income"?"收入":kind==="expense"?"支出":"无金额变动"}</small></span></button>
                {expanded===entry.id&&<div className="ym-ledger-entry-detail"><span>账单编号：{entry.id}</span><span>分类：{ledgerReasons[entry.reason]}</span>{entry.pluginId&&<span>游戏：{entry.pluginId} · {entry.pluginVersion}</span>}{entry.refId&&<span>历史饰品编号：{entry.refId}（原饰品可能已出售或删除）</span>}<span>时间：{new Date(entry.createdAt).toLocaleString("zh-CN")}</span></div>}</article>;
            })}</div>
            <nav className="ym-ledger-pagination" aria-label="账单分页"><span>每页12条 · 第 {report.page} / {report.totalPages} 页</span><button className="ym-button" disabled={loading||query.page<=1} onClick={()=>setQuery(q=>({...q,page:q.page-1}))}>上一页</button><button className="ym-button" disabled={loading||query.page>=report.totalPages} onClick={()=>setQuery(q=>({...q,page:q.page+1}))}>下一页</button></nav></>}
        </MotionRegion></div>}
    </div>;
}
