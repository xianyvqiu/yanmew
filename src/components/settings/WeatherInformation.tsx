import { useState, type ReactNode } from "react";
import { weatherApi, webSearchApi, errorMessage } from "../../api";
import type { WeatherInformation as Information, WeatherFeed } from "../../types";
import { WeatherIcon } from "./WeatherCard";
import "./WeatherInformation.css";
import Track from "./WeatherTyphoonMap";

const dateTime=(value:string|null|undefined)=>value?new Date(value).toLocaleString("zh-CN"):"尚未更新";
const number=(value:number|null|undefined,unit="")=>value==null?"—":`${Math.round(value)}${unit}`;
const label=(code:number)=>{const descriptions:Record<number,string>={0:"晴",1:"大部晴朗",2:"多云",3:"阴",45:"雾",48:"雾凇",51:"毛毛雨",53:"毛毛雨",55:"毛毛雨",56:"冻雨",57:"冻雨",61:"小雨",63:"中雨",65:"大雨",66:"冻雨",67:"冻雨",71:"小雪",73:"中雪",75:"大雪",77:"米雪",80:"阵雨",81:"阵雨",82:"强阵雨",85:"阵雪",86:"阵雪",95:"雷雨",96:"雷雨冰雹",99:"雷雨冰雹"};return descriptions[code]??"天气待更新";};
const kind=(code:number)=>code===0?"sun":code<=3&&code>=0?"cloud":code===45||code===48?"fog":[95,96,99].includes(code)?"storm":[71,73,75,77,85,86].includes(code)?"snow":[51,53,55,56,57,61,63,65,66,67,80,81,82].includes(code)?"rain":"unknown";
const intensityLabel=(value:string)=>({TD:"热带低压",TS:"热带风暴",STS:"强热带风暴",TY:"台风",STY:"强台风",SUPERTY:"超强台风"}[value.trim().toUpperCase()]??(value.trim()||"等级暂缺"));
function Icon({type}:{type:"alert"|"storm"|"shirt"|"drop"|"wind"|"sun"}) {
 return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{type==="alert"?<><path d="m12 3 10 18H2L12 3Z"/><path d="M12 9v5m0 3v.1"/></>:type==="storm"?<><path d="M20 8a8 8 0 0 0-14-3M4 16a8 8 0 0 0 14 3M6 5 4 9l5-1m9 11 2-4-5 1"/><circle cx="12" cy="12" r="3"/></>:type==="shirt"?<path d="m8 3-6 4 3 5 3-2v11h8V10l3 2 3-5-6-4c0 4-8 4-8 0Z"/>:type==="drop"?<path d="M12 2S5 11 5 15a7 7 0 0 0 14 0c0-4-7-13-7-13Z"/>:type==="wind"?<path d="M3 8h12a3 3 0 1 0-3-3M3 12h16a3 3 0 1 1-3 3M3 16h5a2 2 0 1 1-2 2"/>:<><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1 1m12 12 1 1M5 19l1-1M18 6l1-1"/></>}</svg>;
}
function Care({feed,personaId,kind,revision,onRefresh,disabled}:{feed?:WeatherFeed<unknown>;personaId?:string;kind:"warnings"|"typhoons";revision:number;onRefresh:()=>Promise<void>;disabled:boolean}) {
 const [busy,setBusy]=useState(false),[error,setError]=useState("");
 const care=feed?.care&&feed.care.personaId===personaId?feed.care:null;
 const status=personaId?feed?.careStatus?.[personaId]:undefined;
 const generating=busy||status?.state==="generating";
 const retry=async()=>{if(generating)return;setBusy(true);setError("");try{await weatherApi.retryCare(revision,kind);}catch(e){setError(errorMessage(e));}finally{try{await onRefresh();}catch(e){setError(errorMessage(e));}finally{setBusy(false);}}};
 return <div className="ym-weather-care" data-state={care?"ready":generating?"generating":status?.state??"waiting"} aria-label={kind==="warnings"?"预警角色关怀":"台风角色关怀"}>
  <div className="ym-weather-care-content"><span className="ym-weather-care-title">{care?`${care.name}的话`:"角色关怀"}</span>
   <p role="status">{care?.text??(generating?"正在准备一句关心你的话…":status?.state==="failed"?"关怀话语未能生成。":feed?.items.length?"等待角色关怀；请先配置可用的聊天服务。":"等待预警或台风发生重大变化时，角色会在这里关心你。")}</p>
   {!care&&(error||status?.error)&&<p className="ym-page-note" role="alert">{error||status?.error}</p>}
  </div>
  {!care&&!!feed?.items.length&&<button className="ym-button" disabled={disabled||generating||!personaId} title="使用当前聊天模型重试，可能消耗模型额度" onClick={()=>void retry()}>{generating?"生成中…":"重试关怀"}</button>}
 </div>;
}
function SourceLink({url,children,onError}:{url:string;children:ReactNode;onError:(error:string)=>void}) {
 return <a className="ym-weather-source" href={url} target="_blank" rel="noopener noreferrer" onClick={event=>{event.preventDefault();void webSearchApi.open(url).catch(error=>onError(errorMessage(error)));}}>{children}<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="M14 3h7v7m0-7L10 14M10 3H4v17h17v-6"/></svg></a>;
}
export function TemperatureTrend({days}:{days:NonNullable<Information["forecast"]>["daily"]}) {
 const values=days.flatMap(d=>[d.low,d.high]).filter((n):n is number=>n!=null);
 if(!values.length)return <p className="ym-page-note">温度趋势数据暂缺</p>;
 const min=Math.min(...values)-2,max=Math.max(...values)+2;
 const y=(n:number)=>108-(n-min)/(max-min)*76;
 return <svg className="ym-temperature-trend" viewBox="0 0 700 150" role="img" aria-label="未来7天最高最低温度趋势">
  <path d="M25 35h650M25 75h650M25 115h650" className="ym-weather-chart-grid"/>
  {(["high","low"] as const).map(key=><g key={key} className={`ym-temp-${key}`}>
   <polyline fill="none" points={days.map((d,i)=>d[key]==null?null:`${50+i*100},${y(d[key]!)}`).filter(Boolean).join(" ")}/>
   {days.map((d,i)=>d[key]==null?null:<g key={d.date}><circle cx={50+i*100} cy={y(d[key]!)} r="3.5"/><text x={50+i*100} y={y(d[key]!)+(key==="high"?-10:18)} textAnchor="middle">{number(d[key],"°")}</text></g>)}
  </g>)}
  {days.map((d,i)=><text key={d.date} x={50+i*100} y="143" textAnchor="middle">{d.date.slice(5)}</text>)}
 </svg>;
}
export default function WeatherInformation({information,personaId,contextRevision,onRefresh,disabled=false}:{information?:Information|null;personaId?:string;contextRevision:number;onRefresh:()=>Promise<void>;disabled?:boolean}) {
 const [busy,setBusy]=useState(false),[error,setError]=useState("");
 const refresh=async()=>{if(busy)return;setBusy(true);setError("");try{await weatherApi.refreshInformation(contextRevision);await onRefresh();}catch(e){setError(errorMessage(e));}finally{setBusy(false);}};
 const stale=(time:string|null|undefined)=>{const age=time?Date.now()-new Date(time).getTime():NaN;return !Number.isFinite(age)||age<0||age>30*60_000;};
 const warning=information?.warnings,typhoon=information?.typhoons,forecast=information?.forecast;
 const active=(typhoon?.items??[]).filter(t=>Date.now()-new Date(t.publishedAt).getTime()<24*3600_000);
 const forecastOld=forecast&&Date.now()-new Date(forecast.fetchedAt).getTime()>2*3600_000;
 const today=forecast?.daily[0],temperature=forecast?.feelsLike;
 const clothing=temperature==null?"体感数据暂缺，穿衣建议待更新":temperature<5?"厚外套或羽绒服，留意保暖":temperature<12?"厚外套搭配长袖，早晚添衣":temperature<20?"长袖搭配薄外套，方便随时增减":temperature<26?"轻薄长袖或短袖，按体感调整":"透气短袖，尽量选择清爽衣物";
 return <div className="ym-weather-information">
  <section className="ym-weather-info-card ym-warning-card" aria-label="气象预警">
   <header><div className="ym-weather-module-title"><Icon type="alert"/><div><h3>气象预警</h3><p>留意身边的天气变化</p></div></div><button className="ym-button" disabled={busy||disabled} onClick={()=>void refresh()}>{busy?"正在更新…":"更新资讯"}</button></header>
   {error&&<p className="ym-page-error" role="alert">{error}</p>}
   {warning?.error&&<p className="ym-weather-status-note">本次查询未完成，保留上次结果。{warning.error}</p>}
   {warning?.items.length?<div className="ym-weather-warning-list">{warning.items.map(w=><details key={w.id} className="ym-weather-warning" data-level={w.level}><summary><span className="ym-weather-level">{{red:"红色",orange:"橙色",yellow:"黄色",blue:"蓝色"}[w.level]??"关注"}</span><strong>{w.title}</strong><span>查看详情</span></summary><div><p>{w.description}</p><p className="ym-page-note">{w.source} · 发布时间：{w.issuedAt}</p><SourceLink url={w.url} onError={setError}>查看预警来源与详情</SourceLink></div></details>)}</div>:<div className="ym-weather-empty"><Icon type="alert"/><strong>{!warning?.checkedAt?"正在等候预警信息":warning.error||stale(warning.checkedAt)?"预警信息待核实":"暂未检索到当地的近期预警"}</strong><p>公开检索关注本地区及上级发布的信号；来源可能不完整，空列表不代表官方解除，最新状态以官方发布为准。</p></div>}
   <Care key={`warnings:${personaId}:${contextRevision}`} feed={warning} personaId={personaId} kind="warnings" revision={contextRevision} onRefresh={onRefresh} disabled={disabled}/>
   <footer>检查于 {dateTime(warning?.checkedAt)}{warning?.checkedAt&&stale(warning.checkedAt)?" · 缓存已过期":""}<SourceLink url="https://www.nmc.cn/publish/alarm.html" onError={setError}>全国预警</SourceLink></footer>
  </section>
  <section className="ym-weather-info-card ym-typhoon-card" aria-label="台风追踪">
   <header><div className="ym-weather-module-title"><Icon type="storm"/><div><h3>台风追踪</h3><p>关注可能接近所在地的台风</p></div></div><SourceLink url="https://typhoon.nmc.cn/" onError={setError}>完整路径</SourceLink></header>
   {typhoon?.error&&<p className="ym-weather-status-note">暂未取得最新台风资料，以下可能是旧结果。{typhoon.error}</p>}
   {active.length&&information?<div className="ym-weather-storms">{active.map(storm=><article key={storm.id}><div className="ym-storm-heading"><h4>{storm.name}</h4><span>强度：{intensityLabel(storm.intensity)}</span><span>中心风速 {number(storm.windSpeed," m/s")}</span></div><p>官方预报路径的部分位置距所在地约 {number(storm.distanceKm,"公里")}，值得持续关注。</p><Track storm={storm} information={information}/><p className="ym-page-note">资料发布于 {dateTime(storm.publishedAt)}。按路径与本地距离筛选，不代表确定登陆或直接影响。</p></article>)}</div>:<div className="ym-weather-empty ym-weather-calm"><Icon type="storm"/><strong>{!typhoon?.checkedAt?"正在等候台风信息":typhoon.error||stale(typhoon.checkedAt)||!!typhoon.items.length?"台风资料待更新":"最近较为风平浪静"}</strong><p>{typhoon&&!typhoon.error&&!stale(typhoon.checkedAt)&&!typhoon.items.length?"暂未发现官方预报路径接近所在地的活跃台风。":"取得最新资料后，会显示与所在地有关的台风动态。"}</p></div>}
   <Care key={`typhoons:${personaId}:${contextRevision}`} feed={typhoon} personaId={personaId} kind="typhoons" revision={contextRevision} onRefresh={onRefresh} disabled={disabled}/>
   <div className="ym-weather-source-row"><SourceLink url="https://weibo.com/tybbs" onError={setError}>中气爱 · 中国气象爱好者</SourceLink><span>科普解读</span><SourceLink url="https://www.nmc.cn/publish/typhoon/typhoon_new.html" onError={setError}>中央气象台快讯</SourceLink></div>
   <details className="ym-weather-extended-reading"><summary>天气、预警与台风资讯</summary><p className="ym-page-note">检索所在地的天气预报、气象预警和台风动态。报道时间和内容以原文为准，不作为实时预警判定。</p>{information?.searchError&&<p className="ym-page-note">气象资讯检索暂不可用，仍可打开上方来源。</p>}{information?.links.length?information.links.map(link=><article key={link.url}><SourceLink url={link.url} onError={setError}>{link.title}</SourceLink><p>{link.summary}</p></article>):<p className="ym-page-note">暂未取得相关气象报道，可查看上方官方预警与台风来源。</p>}</details>
   <footer>检查于 {dateTime(typhoon?.checkedAt)}{typhoon?.checkedAt&&stale(typhoon.checkedAt)?" · 缓存已过期":""}</footer>
  </section>
  {forecast?<>
   <section className="ym-weather-info-card" aria-label="天气预报"><header><div className="ym-weather-module-title"><Icon type="sun"/><div><h3>未来天气</h3><p>最高与最低温度趋势</p></div></div><span className="ym-page-note">{forecastOld?"过期预报 · ":""}{dateTime(forecast.fetchedAt)}</span></header>
    {information?.forecastError&&<p className="ym-weather-status-note">预报更新暂未完成，保留上次结果。</p>}
    <TemperatureTrend days={forecast.daily}/><div className="ym-weather-days">{forecast.daily.map((day,i)=><article key={day.date}><span>{i===0?"今天":i===1?"明天":day.date.slice(5)}</span><WeatherIcon kind={kind(day.code)} night={false}/><strong>{label(day.code)}</strong><span>{number(day.low,"°")} / {number(day.high,"°")}</span><small>降水 {number(day.rain,"%")}</small></article>)}</div>
    <h4 className="ym-weather-subheading">未来24小时</h4><div className="ym-weather-hours">{forecast.hourly.filter((_,i)=>i%3===0).map(hour=><div key={hour.time}><time>{new Date(hour.time*1000).getHours()}时</time><WeatherIcon kind={kind(hour.code)} night={new Date(hour.time*1000).getHours()<6||new Date(hour.time*1000).getHours()>=18}/><strong>{number(hour.temperature,"°")}</strong><small>{number(hour.rain,"%")} 降水</small></div>)}</div>
    <footer>预报来源 Open-Meteo · 预报可能随观测更新</footer>
   </section>
   <section className="ym-weather-info-card" aria-label="穿衣与生活建议"><header><div className="ym-weather-module-title"><Icon type="shirt"/><div><h3>今天怎么安排</h3><p>根据体感、风与降水给你的参考</p></div></div></header><div className="ym-weather-clothing"><Icon type="shirt"/><div><strong>{clothing}</strong><p>{today?.rain!=null&&today.rain>=50?"出门带伞，留意临近降雨。":"出门前再看看天空与临近预报。"}{forecastOld?" 当前为旧缓存，请结合实际天气调整。":""}</p></div></div>
    <div className="ym-weather-metrics">{[{icon:"sun" as const,title:"体感温度",value:number(forecast.feelsLike,"°")},{icon:"drop" as const,title:"湿度",value:number(forecast.humidity,"%")},{icon:"wind" as const,title:"风速",value:number(forecast.windSpeed," km/h")},{icon:"sun" as const,title:"今日紫外线",value:number(today?.uv)}].map(item=><article key={item.title}><span><Icon type={item.icon}/>{item.title}</span><strong>{item.value}</strong></article>)}</div>
    <footer>日出 {today?.sunrise?new Date(today.sunrise*1000).toLocaleTimeString("zh-CN",{hour:"2-digit",minute:"2-digit"}):"—"} · 日落 {today?.sunset?new Date(today.sunset*1000).toLocaleTimeString("zh-CN",{hour:"2-digit",minute:"2-digit"}):"—"}</footer>
   </section>
  </>:<p className="ym-page-note">{information?.forecastError??"天气预报与生活建议将在后台取得资料后显示。"}</p>}
  {information?.region&&<p className="ym-weather-attribution">地区名称：<SourceLink url="https://www.openstreetmap.org/copyright" onError={setError}>© OpenStreetMap contributors</SourceLink> · 无需天气 API Key 的公开资料模式；和风配置为可选兜底。</p>}
 </div>;
}
