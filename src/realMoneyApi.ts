import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { LedgerGroup, LedgerSummary, LedgerReport } from "./ledgerApi";
export type RealEntryInput={title:string;kind:"income"|"expense";categoryId:string;assetId:string;description:string;amount:string;occurredLocal:string|null;recorder:string|null};
export type RealRule={unit:"day"|"week"|"month"|"year";interval:number};
export type RealAsset={id:string;name:string;note:string;isDefault:boolean;archived:boolean;revision:number;balanceCents:string;createdAt:string;updatedAt:string};
export type RealCategory={id:string;name:string;icon:string;direction:"income"|"expense"|"both";archived:boolean;revision:number;createdAt:string;updatedAt:string};
export type RealEntry={id:string;title:string;kind:"income"|"expense"|"adjustment";categoryId:string|null;categoryName:string;assetId:string;assetName:string;description:string;amountCents:string;occurredLocal:string;source:string;recorder:string;scheduleId:string|null;revision:number;createdAt:string;updatedAt:string};
export type RealPlan={id:string;title:string;kind:"income"|"expense";categoryId:string;assetId:string;description:string;amountCents:string;anchorLocal:string;unit:RealRule["unit"];interval:number;generation:number;nextSequence:number;status:"active"|"paused"|"ended";reviewRequired:boolean;recorder:string;revision:number;createdAt:string;updatedAt:string;nextLocal:string|null};
export type RealPending={id:string;scheduleId:string;scheduledLocal:string;entry:RealEntryInput};
export type RealBudget={period:"day"|"month"|"year";amountCents:string|null;spentCents:string;remainingCents:string|null;averageDailyCents:string;remainingDailyCents:string|null;elapsedDays:number;remainingDays:number};
export type PendingSummary={count:number;throughId:string|null;incomeCents?:string;expenseCents?:string};
export type RealOverview={assets:RealAsset[];categories:RealCategory[];totalAssetsCents:string;today:string;todaySummary:LedgerSummary;monthSummary:LedgerSummary;budgets:RealBudget[];pending:PendingSummary;activeSchedules:number};
export type RealQuery={start:string|null;end:string|null;direction:"all"|"income"|"expense"|"adjustment";categoryId:string;assetId:string;search:string;ascending:boolean;page:number};
export type RealReport={entries:RealEntry[];summary:LedgerSummary;balance:string;page:number;totalPages:number;categories:LedgerGroup[];assets:LedgerGroup[];timeline:LedgerGroup[];bucket:string;rangeStart:string;rangeEnd:string};
export type RealAction=
 |{type:"save_asset";id:string|null;revision:number|null;name:string;note:string;balance:string|null;isDefault:boolean}
 |{type:"delete_asset";id:string;revision:number}
 |{type:"set_total";amount:string}
 |{type:"save_category";id:string|null;revision:number|null;name:string;icon:string;direction:RealCategory["direction"]}
 |{type:"delete_category";id:string;revision:number}
 |{type:"save_entry";id:string|null;revision:number|null;entry:RealEntryInput;recurrence:RealRule|null}
 |{type:"delete_entry";id:string;revision:number}
 |{type:"save_budgets";day:string|null;month:string|null;year:string|null}
 |{type:"save_schedule";id:string;revision:number;entry:RealEntryInput;rule:RealRule;status:RealPlan["status"]}
 |{type:"resolve_missed";ids:string[];throughId:string|null;record:boolean};
export type RealResult={id?:string|null;scheduleId?:string;changed?:number;alreadyApplied?:boolean};
export type RealListQuery={kind:"schedules"|"pending";search:string;status:string;ascending:boolean;page:number};
export type RealPage<T>={items:T[];total:number;pages:number;page:number};
export type RealCalendarDay={date:string;income:string;expense:string;planned:number;pending:number};
export const emptyRealQuery=():RealQuery=>({start:null,end:null,direction:"all",categoryId:"",assetId:"",search:"",ascending:false,page:1});
export const realMoneyApi={
 overview:()=>invoke<RealOverview>("get_real_money_overview"),
 query:(input:RealQuery)=>invoke<RealReport>("query_real_money",{input}),
 mutate:(requestId:string,action:RealAction)=>invoke<RealResult>("mutate_real_money",{input:{requestId,action}}),
 receipt:(requestId:string)=>invoke<RealResult|null>("get_real_money_operation",{requestId}),
 tasks:<T,>(input:RealListQuery)=>invoke<RealPage<T>>("list_real_money_tasks",{input}),
 calendar:(year:number,month:number)=>invoke<{days:RealCalendarDay[]}>("get_real_money_calendar",{year,month}),
 pending:()=>invoke<PendingSummary>("get_real_money_pending"),
 listen:(receive:()=>void)=>listen("real-money-changed",receive),
 listenError:(receive:(message:string)=>void)=>listen<{message:string}>("real-money-error",e=>receive(e.payload.message)),
};
export const realKindNames={income:"收入",expense:"支出",adjustment:"余额校准"};
export function moneyText(cents:string):string{const n=BigInt(cents),v=n<0n?-n:n;return `${n<0n?"−":""}${(v/100n).toLocaleString("zh-CN")}.${String(v%100n).padStart(2,"0")}`;}
export function moneyInput(cents:string):string{const n=BigInt(cents),v=n<0n?-n:n;return `${n<0n?"-":""}${v/100n}.${String(v%100n).padStart(2,"0")}`;}
export function localMoneyTime(now=new Date()){const pad=(n:number)=>String(n).padStart(2,"0");return `${now.getFullYear()}-${pad(now.getMonth()+1)}-${pad(now.getDate())}T${pad(now.getHours())}:${pad(now.getMinutes())}`;}
export function planEntry(p:RealPlan):RealEntryInput{return {title:p.title,kind:p.kind,categoryId:p.categoryId,assetId:p.assetId,description:p.description,amount:moneyInput(p.amountCents),occurredLocal:p.anchorLocal,recorder:p.recorder};}
export function chartReport(r:RealReport):LedgerReport{return {...r,entries:[],games:r.assets,plugins:[]};}
