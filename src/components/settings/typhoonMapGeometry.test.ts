import {expect,it} from "vitest";
import {typhoonMapGeometry} from "./typhoonMapGeometry";
it("地图同时容纳本地与实况预报位置，保留真实陆地轮廓",()=>{
 const points=[{longitude:126,latitude:24},{longitude:124,latitude:27},{longitude:122,latitude:30}];const local={longitude:121.5,latitude:31.2};const map=typhoonMapGeometry(points,local);
 for(const p of [...points,local]){const at=map.project(p);expect(at.x).toBeGreaterThan(16);expect(at.x).toBeLessThan(704);expect(at.y).toBeGreaterThan(18);expect(at.y).toBeLessThan(330);}
 expect(map.paths.length).toBeGreaterThan(0);expect(map.paths.join("")).not.toMatch(/NaN|Infinity/);
});
it("跨180度的相邻路径点不会被画到地图两侧",()=>{
 const local={longitude:179,latitude:20};const first={longitude:178,latitude:19},second={longitude:-179,latitude:20};const map=typhoonMapGeometry([first,second],local);
 expect(Math.abs(map.project(first).x-map.project(second).x)).toBeLessThan(300);
});
