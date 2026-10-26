import { useId } from "react";
import type { WeatherInformation, WeatherTyphoon } from "../../types";
import { typhoonMapGeometry } from "./typhoonMapGeometry";
import "./WeatherTyphoonMap.css";

export default function WeatherTyphoonMap({storm,information}:{storm:WeatherTyphoon;information:WeatherInformation}){
    const clip=`typhoon-map-${useId().replace(/:/g,"")}`,map=typhoonMapGeometry(storm.points,information.position);
    const observed=storm.points.filter(p=>!p.forecast),forecast=storm.points.filter(p=>p.forecast);
    const current=observed[observed.length-1],local=map.project(information.position),center=current?map.project(current):null;
    const line=(points:typeof storm.points)=>points.map(p=>{const at=map.project(p);return `${at.x.toFixed(1)},${at.y.toFixed(1)}`;}).join(" ");
    return <figure className="ym-typhoon-map"><svg viewBox="0 0 720 360" role="img" aria-label={`${storm.name}官方预报路径地图`}><title>{storm.name}实况与预报路径，标出所在地和当前中心；地理底图仅作示意。</title><defs><clipPath id={clip}><rect x="16" y="18" width="688" height="312" rx="16"/></clipPath></defs>
        <rect className="ym-map-ocean" x="16" y="18" width="688" height="312" rx="16"/>
        <g clipPath={`url(#${clip})`}><path className="ym-map-land" d={map.paths.join(" ")} fillRule="evenodd"/>
        {map.longitudes.map(lon=>{const x=map.project({longitude:lon,latitude:0},false).x;return <line key={lon} x1={x} x2={x} y1="18" y2="330" className="ym-map-grid"/>;})}
        {map.latitudes.map(lat=>{const y=map.project({longitude:0,latitude:lat},false).y;return <line key={lat} x1="16" x2="704" y1={y} y2={y} className="ym-map-grid"/>;})}
        <polyline points={line(observed)} className="ym-map-observed"/><polyline points={line([...observed.slice(-1),...forecast])} className="ym-map-forecast"/>
        {forecast.filter((_,i)=>i%Math.max(1,Math.ceil(forecast.length/8))===0).map((p,i)=>{const at=map.project(p);return <circle key={i} cx={at.x} cy={at.y} r="4" className="ym-map-forecast-point"><title>官方预报点：{p.latitude.toFixed(2)}°，{p.longitude.toFixed(2)}°</title></circle>;})}
        {center&&<g><circle cx={center.x} cy={center.y} r="13" className="ym-map-center-halo"/><circle cx={center.x} cy={center.y} r="6" className="ym-map-center"/><text x={center.x+(center.x>550?-18:18)} y={center.y-14} textAnchor={center.x>550?"end":"start"} className="ym-map-label">当前中心</text></g>}
        <circle cx={local.x} cy={local.y} r="11" className="ym-map-local-halo"/><circle cx={local.x} cy={local.y} r="5" className="ym-map-local"/><text x={local.x+(local.x>550?-16:16)} y={local.y+22} textAnchor={local.x>550?"end":"start"} className="ym-map-label">所在地</text>
        </g>
        {map.longitudes.map(lon=><text key={lon} x={map.project({longitude:lon,latitude:0},false).x} y="349" textAnchor="middle" className="ym-map-axis">{Math.abs(((lon+540)%360)-180).toFixed(0)}°{((lon+540)%360)-180<0?"W":"E"}</text>)}
        {map.latitudes.map(lat=><text key={lat} x="23" y={map.project({longitude:0,latitude:lat},false).y-5} className="ym-map-axis">{Math.abs(lat).toFixed(0)}°{lat<0?"S":"N"}</text>)}
        </svg><div className="ym-typhoon-map-legend"><span data-kind="observed"><i/>实况路径</span><span data-kind="forecast"><i/>官方预报</span><span data-kind="center"><i/>当前中心</span><span data-kind="local"><i/>所在地</span></div><figcaption><span>{information.region?.name??"所在地"} · 路径示意，不代表确定影响</span><span>底图 Natural Earth</span></figcaption></figure>;
}
