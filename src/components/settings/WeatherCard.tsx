import { useEffect, useState } from "react";
import type { WeatherView } from "../../types";
import "./WeatherCard.css";

export function weatherKind(description:string):"sun"|"cloud"|"rain"|"snow"|"storm"|"fog"|"unknown" {
  if (!description.trim())return "unknown";
  if (/雷|thunder/i.test(description))return "storm";
  if (/雪|snow/i.test(description))return "snow";
  if (/雨|rain|drizzle/i.test(description))return "rain";
  if (/雾|霾|fog|mist/i.test(description))return "fog";
  if (/云|阴|cloud|overcast/i.test(description))return "cloud";
  return "sun";
}
export function WeatherIcon({kind,night}:{kind:ReturnType<typeof weatherKind>;night:boolean}) {
  return <svg viewBox="0 0 80 80" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    {kind==="unknown"?<><circle cx="40" cy="40" r="24"/><path d="M33 32a8 8 0 0 1 15 4c0 6-8 6-8 12m0 7v1"/></>:kind==="sun"?(night?<path d="M53 15a25 25 0 1 0 12 44A25 25 0 0 1 53 15Z"/>:<><circle cx="40" cy="40" r="14"/>{[0,45,90,135,180,225,270,315].map(angle=><path key={angle} transform={`rotate(${angle} 40 40)`} d="M40 10v8"/>)}</>):<path d="M20 52a13 13 0 0 1 0-26 20 20 0 0 1 38-1 13 13 0 0 1 4 27H20Z"/>}
    {kind==="rain"&&<path d="m25 60-4 8m20-8-4 8m20-8-4 8"/>}{kind==="snow"&&<path d="M26 61v10m-5-5h10m20-5v10m-5-5h10"/>}{kind==="storm"&&<path d="m42 54-8 11h10l-6 11"/>}{kind==="fog"&&<path d="M18 61h44M24 69h32"/>}
  </svg>;
}
export default function WeatherCard({weather}:{weather:WeatherView}) {
  const [now,setNow]=useState(()=>new Date());
  useEffect(()=>{const update=()=>setNow(new Date());const timer=window.setInterval(update,30000);window.addEventListener("focus",update);return()=>{window.clearInterval(timer);window.removeEventListener("focus",update);};},[]);
  const night=now.getHours()<6||now.getHours()>=18;
  const observation=weather.data?.observation;
  const status = weather.availability === "stale" ? "缓存已过期" : weather.availability === "unavailable" ? "等待天气数据" : weather.availability === "cached" ? "缓存天气" : "最新天气";
  return <section className="ym-weather-visual" data-period={night ? "night" : "day"} aria-label="当前天气卡片">
    <header className="ym-weather-heading"><span className="ym-weather-eyebrow">{weather.information?.region?.name ?? "此刻 · 时间与天气"}</span><span className="ym-weather-period"><svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.6">{night ? <path d="M16 3a8 8 0 1 0 5 14A9 9 0 0 1 16 3Z"/> : <><circle cx="12" cy="12" r="4"/><path d="M12 1v3m0 16v3M1 12h3m16 0h3M4 4l2 2m12 12 2 2M4 20l2-2M18 6l2-2"/></>}</svg>{night ? "夜晚" : "白天"}</span></header>
    <div className="ym-weather-scene">
      <div className="ym-weather-clock"><time dateTime={now.toISOString()}>{now.toLocaleTimeString("zh-CN", {hour:"2-digit", minute:"2-digit"})}</time><span>{now.toLocaleDateString("zh-CN", {year:"numeric", month:"long", day:"numeric", weekday:"long"})}</span><p>跟随电脑当前时间</p></div>
      <div className="ym-weather-primary"><div className="ym-weather-icon"><WeatherIcon kind={weatherKind(observation?.description ?? "")} night={night}/></div><div className="ym-weather-reading"><strong>{observation?.temperatureCelsius == null ? "—" : `${observation.temperatureCelsius}°`}<small>{observation?.temperatureCelsius == null ? "" : "C"}</small></strong><p>{observation?.description || "暂无天气数据"}</p>{weather.information?.forecast?.daily[0] && <p className="ym-weather-range">最高 {weather.information.forecast.daily[0].high ?? "—"}° · 最低 {weather.information.forecast.daily[0].low ?? "—"}°</p>}</div></div>
    </div>
    <footer className="ym-weather-caption"><span className="ym-weather-status"><i aria-hidden="true"/>{status}</span><span>{observation ? `观测于 ${new Date(observation.observedAt).toLocaleString("zh-CN")}` : "点击下方刷新，获取当前位置的天气"}</span></footer>
  </section>;
}
