import MotionDetails from "./MotionDetails";
import type {TravelOrigin} from "../types";
import {travelTime} from "./travelTime";
export function TravelOriginLabel({travel,travelId}:{travel?:TravelOrigin|null;travelId:string|null}){
  return <span className="ym-travel-origin">{travel?`${travelTime(travel.startedAt)}出发 · ${travel.destination??"目的地未记录"}`:travelId?`出行 #${travelId} · 详情未保留`:"未关联出行的旧记录"}</span>;
}
export default function TravelFilter({browser,disabled=false}:{browser:{travels:TravelOrigin[];travelIds:string[];setTravelIds:(ids:string[])=>void};disabled?:boolean}){
  return <MotionDetails className="ym-trip-filter"><summary>按出行筛选 <span>{browser.travelIds.length?`已选 ${browser.travelIds.length} 次`:"全部出行"}</span></summary>
    <div className="ym-trip-filter-body"><div className="ym-page-heading"><p className="ym-page-note">可勾选一次或多次出行；不勾选时显示全部记录。</p><button type="button" className="ym-button" disabled={disabled||!browser.travelIds.length} onClick={()=>browser.setTravelIds([])}>清除筛选</button></div>
    <div className="ym-trip-options">{browser.travels.length?browser.travels.map(trip=><label key={trip.id}><input className="ym-selection-checkbox" type="checkbox" checked={browser.travelIds.includes(trip.id)} disabled={disabled} onChange={event=>browser.setTravelIds(event.target.checked?[...browser.travelIds,trip.id]:browser.travelIds.filter(id=>id!==trip.id))}/><span><strong>{trip.destination??"目的地未记录"}</strong><small>{travelTime(trip.startedAt)}出发</small></span></label>):<p className="ym-page-note">当前会话还没有出行记录。</p>}</div></div>
  </MotionDetails>;
}
