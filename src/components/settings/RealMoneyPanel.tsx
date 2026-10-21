import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { errorMessage } from "../../api";
import { ledgerRange } from "../../ledgerApi";
import { chartReport, emptyRealQuery, moneyText, realKindNames, realMoneyApi, type RealAction, type RealAsset, type RealCategory, type RealEntry, type RealOverview, type RealPlan, type RealQuery, type RealReport } from "../../realMoneyApi";
import MotionRegion from "../MotionRegion";
import LedgerCharts,{LedgerDisplayContext,LedgerOverview} from "./LedgerCharts";
import RealMoneyBills,{RealMoneyFilters} from "./RealMoneyBills";
import RealMoneyBudgets from "./RealMoneyBudgets";
import RealMoneyEntities from "./RealMoneyEntities";
import RealMoneyTasks from "./RealMoneyTasks";
import RealMoneyCalendar from "./RealMoneyCalendar";
import { RealAssetEditor,RealCategoryEditor,RealConfirm,RealEntryEditor,RealFormErrorContext,RealModal,RealTotalEditor,type RealRun } from "./RealMoneyForms";
import "./LedgerPanel.css";
import "./RealMoneyPanel.css";
const tabs={overview:"总览",bills:"账单",charts:"数据面板",assets:"资产",budgets:"预算",cycles:"周期任务",categories:"分类",calendar:"日历"};
const moduleNotes={bills:"查询、编辑与管理每一笔收支",charts:"收支趋势、分类占比和资产排行",assets:"管理资金池与校准余额",budgets:"设置每日、每月和每年预算",cycles:"管理自动记账与漏期确认",categories:"整理收入与支出分类",calendar:"按日期查看收支和未来计划"};
type Tab=keyof typeof tabs;
type Editor={kind:"entry";existing?:RealEntry|RealPlan;recurring?:boolean}|{kind:"asset";asset?:RealAsset}|{kind:"category";category?:RealCategory}|{kind:"total"};
const display={format:moneyText,unit:"元",scale:100,sourceName:"资产"};
export default function RealMoneyPanel({request}:{request?:{date:string|null}}){
    const[tab,setTab]=useState<Tab>("overview");const[overview,setOverview]=useState<RealOverview>();const[report,setReport]=useState<RealReport>();
    const[query,setQuery]=useState<RealQuery>(emptyRealQuery);const[preset,setPreset]=useState("all");const[refresh,setRefresh]=useState(0);
    const[filtersOpen,setFiltersOpen]=useState(false);
    const[readError,setReadError]=useState("");const[reportError,setReportError]=useState("");const[writeError,setWriteError]=useState("");const[notice,setNotice]=useState("");
    const[loading,setLoading]=useState(true);const[working,setWorking]=useState(false);const[uncertain,setUncertain]=useState(false);
    const[editor,setEditor]=useState<Editor>();const[detail,setDetail]=useState<RealEntry>();const[confirmation,setConfirmation]=useState<{title:string;text:string;action:RealAction}>();
    const lock=useRef(false);const operation=useRef<{id:string;action:RealAction;done?:()=>void}>();const root=useRef<HTMLDivElement>(null);const billScroll=useRef(0);
    const busy=working||uncertain;
    useEffect(()=>{if(request){if(request.date){setPreset("custom");setQuery(q=>({...q,start:request.date,end:request.date,page:1}));setTab("bills");}else setTab("cycles");}},[request]);
    useEffect(()=>{let disposed=false;const stops:Array<()=>void>=[];const changed=()=>{if(!disposed)setRefresh(v=>v+1);};
        void realMoneyApi.listen(changed).then(stop=>{if(disposed)stop();else{stops.push(stop);changed();}}).catch(()=>{});
        void realMoneyApi.listenError(message=>{if(!disposed)setReadError(message);}).then(stop=>{if(disposed)stop();else stops.push(stop);}).catch(()=>{});
        const focus=()=>{if(preset!=="custom"){const range=ledgerRange(preset);setQuery(q=>q.start===range.start&&q.end===range.end?q:{...q,...range,page:1});}changed();};
        window.addEventListener("focus",focus);return()=>{disposed=true;stops.forEach(stop=>stop());window.removeEventListener("focus",focus);};
    },[preset]);
    useEffect(()=>{let disposed=false;void realMoneyApi.overview().then(v=>{if(!disposed){setOverview(v);setReadError("");}}).catch(e=>{if(!disposed)setReadError(errorMessage(e));});return()=>{disposed=true;};},[refresh]);
    useEffect(()=>{if(tab!=="bills"&&tab!=="charts")return;let disposed=false;setLoading(true);setReportError("");void realMoneyApi.query(query).then(v=>{if(disposed)return;if(query.page>v.totalPages){setQuery(q=>({...q,page:v.totalPages}));return;}setReport(v);setLoading(false);}).catch(e=>{if(!disposed){setReportError(errorMessage(e));setLoading(false);}});return()=>{disposed=true;};},[query,refresh,tab]);
    useLayoutEffect(()=>{const container=root.current?.closest(".ym-window-shell__content");if(container)container.scrollTop=tab==="bills"?billScroll.current:0;},[tab]);
    const navigate=(next:Tab)=>{const container=root.current?.closest(".ym-window-shell__content");if(tab==="bills"&&container)billScroll.current=container.scrollTop;setFiltersOpen(false);setTab(next);};
    const filter=(patch:Partial<RealQuery>)=>{setQuery(q=>({...q,...patch,page:1}));billScroll.current=0;};
    const run:RealRun=async(action,done)=>{
        if(lock.current)return;if(operation.current&&operation.current.action!==action){setWriteError("请先核对上一笔操作，再执行新的修改。");return;}
        lock.current=true;setWorking(true);setWriteError("");setNotice("");const op=operation.current??{id:crypto.randomUUID(),action,done};operation.current=op;
        try{
            try{await realMoneyApi.mutate(op.id,op.action);}catch(cause){
                if(cause&&typeof cause==="object"&&"code"in cause&&cause.code!=="STORAGE_ERROR"){operation.current=undefined;throw cause;}
                const receipt=await realMoneyApi.receipt(op.id);if(!receipt){operation.current=undefined;throw cause;}
            }
            operation.current=undefined;setUncertain(false);setNotice("已保存。资产和统计按已提交记录重新计算。");op.done?.();setRefresh(v=>v+1);
        }catch(cause){setUncertain(!!operation.current);setWriteError(operation.current?`暂时无法确认结果，请核对并重试同一笔操作，避免重复记账。${errorMessage(cause)}`:errorMessage(cause));}
        finally{lock.current=false;setWorking(false);}
    };
    const open=(value:Editor)=>{setWriteError("");setEditor(value);};
    const confirm=(title:string,text:string,action:RealAction)=>{setWriteError("");setDetail(undefined);setConfirmation({title,text,action});};
    const deleteEntity=(entity:RealAsset|RealCategory)=>confirm("删除这项资料？",`将删除“${entity.name}”的可选入口，保留历史账单。${"balanceCents"in entity?"资产需先清零，并处理未结束的周期。":"分类仍被周期使用时，需要先修改或结束相关周期。"}`,"balanceCents"in entity?{type:"delete_asset",id:entity.id,revision:entity.revision}:{type:"delete_category",id:entity.id,revision:entity.revision});
    return <RealFormErrorContext.Provider value={{message:writeError,working,retry:uncertain?()=>{const op=operation.current;if(op)void run(op.action,op.done);}:undefined}}><div ref={root} className="real-money ym-ledger" aria-label="现实人民币记账"><div className="ym-page-heading"><div><h2>{tab==="overview"?"现实记账":tabs[tab]}</h2><p className="ym-page-note">全局人民币账本 · 与喵币分别保存</p></div><div className="ym-inline-form">{tab!=="overview"&&<button className="ym-button" disabled={busy} onClick={()=>navigate("overview")}>返回现实记账</button>}<button className="ym-button" disabled={working} onClick={()=>setRefresh(v=>v+1)}>刷新</button>{(tab==="overview"||tab==="bills")&&<button className="ym-button" data-variant="primary" disabled={busy||!overview} onClick={()=>open({kind:"entry"})}>＋ 记一笔</button>}</div></div>
        {readError&&<p role="alert" className="ym-page-error">读取失败：{readError}<button className="ym-button" onClick={()=>setRefresh(v=>v+1)}>重试</button></p>}{writeError&&!editor&&!confirmation&&<p role="alert" className="ym-page-error">{writeError}</p>}{uncertain&&<button className="ym-button" disabled={working} onClick={()=>{const op=operation.current;if(op)void run(op.action,op.done);}}>核对并重试原操作</button>}{notice&&<p className="ym-page-note" role="status">{notice}</p>}
        {!overview?<p role="status">正在读取现实账本…</p>:<><MotionRegion motionKey={tab}><div className="real-content">
        {tab==="overview"&&<div className="ym-finance-home"><div className="real-hero"><div><span>总资产</span><strong>¥{moneyText(overview.totalAssetsCents)}</strong><small>各资金池余额合计</small></div><div><span>本月支出</span><strong>¥{moneyText(overview.monthSummary.expense)}</strong><small>收入 ¥{moneyText(overview.monthSummary.income)} · 结余 ¥{moneyText(overview.monthSummary.net)}</small></div></div><nav className="ym-finance-modules" aria-label="现实记账模块">{Object.entries(moduleNotes).map(([key,note])=><button className="ym-finance-module" aria-label={tabs[key as Tab]} key={key} disabled={busy} onClick={()=>navigate(key as Tab)}><span><strong>{tabs[key as Tab]}{key==="cycles"&&overview.pending.count>0&&<span className="real-badge">{overview.pending.count} 项待确认</span>}</strong><small>{key==="cycles"?`${overview.activeSchedules} 项运行中 · ${note}`:note}</small></span><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" aria-hidden="true"><path d="m9 5 7 7-7 7"/></svg></button>)}</nav></div>}
        {(tab==="bills"||tab==="charts")&&<><div className="ym-finance-query-heading"><p className="ym-page-note">{report?`${report.rangeStart} — ${report.rangeEnd}`:"正在读取日期范围"}</p><div className="ym-inline-form"><button className="ym-button" aria-expanded={filtersOpen} aria-controls="real-money-filters" onClick={()=>setFiltersOpen(v=>!v)}>{filtersOpen?"收起筛选":"筛选账单"}</button><button className="ym-button" disabled={busy} onClick={()=>navigate(tab==="bills"?"charts":"bills")}>{tab==="bills"?"账单数据面板":"返回账单"}</button></div></div><div id="real-money-filters" hidden={!filtersOpen}><RealMoneyFilters key={query.start+":"+query.end} overview={overview} query={query} preset={preset} onPreset={setPreset} onChange={filter} busy={busy}/></div><p className="ym-page-note" role="status">{loading?"正在读取，保留上次结果…":report?`${report.summary.count} 条记录（含校准）`:""}</p>{reportError&&<p className="ym-page-error" role="alert">{reportError}<button className="ym-button" onClick={()=>setRefresh(v=>v+1)}>重试</button></p>}{report&&<div ref={el=>{el?.toggleAttribute("inert",loading||busy||!!reportError);}} className="real-results" aria-busy={loading}><LedgerDisplayContext.Provider value={display}>{tab==="bills"?<RealMoneyBills report={report} query={query} busy={busy||loading} onPage={page=>setQuery(q=>({...q,page}))} onOpen={entry=>{setWriteError("");setDetail(entry);}}/>:<><LedgerOverview report={chartReport(report)}/><LedgerCharts report={chartReport(report)} onSelect={(categoryId,direction)=>{filter({categoryId,direction});navigate("bills");}} onGame={assetId=>{filter({assetId});navigate("bills");}}/></>}</LedgerDisplayContext.Provider></div>}</>}
        {(tab==="assets"||tab==="categories")&&<RealMoneyEntities mode={tab} overview={overview} busy={busy} run={run} onAsset={asset=>open({kind:"asset",asset})} onCategory={category=>open({kind:"category",category})} onTotal={()=>open({kind:"total"})} onDelete={deleteEntity}/>}
        {tab==="budgets"&&<RealMoneyBudgets overview={overview} busy={busy} run={run}/>}
        {tab==="cycles"&&<RealMoneyTasks overview={overview} refresh={refresh} busy={busy} run={run} onEdit={existing=>open({kind:"entry",existing})} onNew={()=>open({kind:"entry",recurring:true})} onConfirm={confirm}/>}
        {tab==="calendar"&&<RealMoneyCalendar revision={String(refresh)} showInitially onDateDetails={date=>{setPreset("custom");filter({start:date,end:date});navigate("bills");}}/>}
        </div></MotionRegion></>}
        {overview&&editor?.kind==="entry"&&<RealEntryEditor overview={overview} existing={editor.existing} recurring={editor.recurring} busy={busy} run={run} onClose={()=>setEditor(undefined)}/>}
        {editor?.kind==="asset"&&<RealAssetEditor asset={editor.asset} busy={busy} run={run} onClose={()=>setEditor(undefined)}/>}
        {editor?.kind==="category"&&<RealCategoryEditor category={editor.category} busy={busy} run={run} onClose={()=>setEditor(undefined)}/>}
        {overview&&editor?.kind==="total"&&<RealTotalEditor overview={overview} busy={busy} run={run} onClose={()=>setEditor(undefined)}/>}
        {detail&&<RealModal title="现实账单详情" busy={busy} onClose={()=>setDetail(undefined)}><dl className="real-entry-details"><dt>标题</dt><dd>{detail.title}</dd><dt>金额</dt><dd>{realKindNames[detail.kind]} ¥{moneyText(detail.amountCents)}</dd><dt>分类与资产</dt><dd>{detail.categoryName} · {detail.assetName}</dd><dt>发生时间</dt><dd>{detail.occurredLocal.replace("T"," ")}</dd><dt>简介</dt><dd>{detail.description||"无"}</dd><dt>记账者</dt><dd>{detail.recorder}</dd><dt>来源</dt><dd>{({manual:"手动记账",ai:"AI代记",cycle:"周期生成",adjustment:"余额校准"}as Record<string,string>)[detail.source]??detail.source}</dd><dt>记录时间</dt><dd>{new Date(detail.createdAt).toLocaleString("zh-CN")}</dd></dl><div className="ym-inline-form"><button className="ym-button" disabled={busy||detail.kind==="adjustment"} onClick={()=>{setDetail(undefined);open({kind:"entry",existing:detail});}}>编辑</button><button className="ym-button" data-variant="danger" disabled={busy} onClick={()=>confirm("删除这笔现实账单？","删除后会重新计算资产、收支和预算。若由周期生成，删除这一笔不会让同一期重新生成。",{type:"delete_entry",id:detail.id,revision:detail.revision})}>删除</button></div></RealModal>}
        {confirmation&&<RealConfirm {...confirmation} busy={busy} onClose={()=>setConfirmation(undefined)} onConfirm={()=>void run(confirmation.action,()=>setConfirmation(undefined))}/>}
    </div></RealFormErrorContext.Provider>;
}
