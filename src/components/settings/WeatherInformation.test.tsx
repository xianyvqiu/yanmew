import { cleanup,fireEvent,render,screen,waitFor } from "@testing-library/react";
import { afterEach,beforeEach,expect,it,vi } from "vitest";
import WeatherInformation from "./WeatherInformation";
import type { WeatherInformation as Information } from "../../types";
const api=vi.hoisted(()=>({refreshInformation:vi.fn(),retryCare:vi.fn(),open:vi.fn()}));
vi.mock("../../api",()=>({weatherApi:{refreshInformation:api.refreshInformation,retryCare:api.retryCare},webSearchApi:{open:api.open},errorMessage:(e:{message?:string})=>e.message??"操作失败"}));
beforeEach(()=>{vi.clearAllMocks();api.refreshInformation.mockResolvedValue(undefined);api.open.mockResolvedValue(undefined);});
afterEach(cleanup);
function data():Information {return {position:{latitude:31.2,longitude:121.5,locatedAt:new Date().toISOString()},region:{name:"上海市",province:"上海市",city:"上海市",district:"黄浦区"},forecast:null,forecastError:null,warnings:{items:[],error:null,checkedAt:new Date().toISOString(),care:null},typhoons:{items:[],error:null,checkedAt:new Date().toISOString(),care:null},links:[],searchError:null};}
function show(information?:Information|null,personaId="1",refresh=vi.fn().mockResolvedValue(undefined)) {return render(<WeatherInformation information={information} personaId={personaId} contextRevision={7} onRefresh={refresh}/>);}
it("只有成功且新鲜的空台风结果才显示风平浪静，进入页面不主动查询",()=>{
 const view=show(null);expect(screen.queryByText("最近较为风平浪静")).toBeNull();expect(api.refreshInformation).not.toHaveBeenCalled();
 view.rerender(<WeatherInformation information={data()} contextRevision={7} onRefresh={async()=>{}}/>);expect(screen.getByText("最近较为风平浪静")).toBeTruthy();
});
it.each(["error","stale","clockRollback"])("%s状态保留信息但不宣称风平浪静",state=>{
 const info=data();if(state==="error")info.typhoons.error="网站连接失败";else info.typhoons.checkedAt=new Date(Date.now()+(state==="clockRollback"?3600_000:-3600_000)).toISOString();
 show(info);expect(screen.queryByText("最近较为风平浪静")).toBeNull();expect(screen.getByText("台风资料待更新")).toBeTruthy();
});
it("预警卡原位展开正文、机构、原文链接，文字作为文本呈现",async()=>{
 const info=data();info.warnings.items=[{id:"1",title:"上海市暴雨橙色预警",description:"注意避开积水路段 <script>test</script>",level:"orange",issuedAt:"2026/10/22 10:00",url:"https://www.nmc.cn/publish/alarm/1.html",source:"上海市气象台"}];
 show(info);fireEvent.click(screen.getByText("上海市暴雨橙色预警"));expect(screen.getByText(/注意避开积水路段/)).toBeTruthy();
 expect(document.querySelector("script")).toBeNull();fireEvent.click(screen.getByRole("link",{name:"查看预警来源与详情"}));await waitFor(()=>expect(api.open).toHaveBeenCalledWith("https://www.nmc.cn/publish/alarm/1.html"));
});
it("关怀只显示当前人设的缓存，不通过页面重新调用模型",()=>{
 const info=data();info.warnings.care={personaId:"1",name:"电子小猫",text:"出门记得带伞呀"};const view=show(info);expect(screen.getByText("出门记得带伞呀")).toBeTruthy();
 view.rerender(<WeatherInformation information={info} personaId="2" contextRevision={8} onRefresh={async()=>{}}/>);expect(screen.queryByText("出门记得带伞呀")).toBeNull();expect(api.refreshInformation).not.toHaveBeenCalled();
});
it("手动更新只发起一次并补读缓存，失败后保留旧内容",async()=>{
 const refresh=vi.fn().mockResolvedValue(undefined);show(data(),"1",refresh);fireEvent.click(screen.getByRole("button",{name:"更新资讯"}));await waitFor(()=>expect(refresh).toHaveBeenCalledTimes(1));expect(api.refreshInformation).toHaveBeenCalledWith(7);
 api.refreshInformation.mockRejectedValueOnce({message:"公开来源暂不可用"});fireEvent.click(screen.getByRole("button",{name:"更新资讯"}));await screen.findByText("公开来源暂不可用");expect(screen.getByRole("heading",{name:"台风追踪"})).toBeTruthy();
});
it("台风卡提供官方轨迹、来源与中气爱账号入口",async()=>{
 const info=data();info.typhoons.items=[{id:"1",name:"2630 · 测试台风",intensity:"TY",publishedAt:new Date().toISOString(),windSpeed:40,distanceKm:300,points:[{longitude:123,latitude:29,forecast:false},{longitude:122,latitude:30,forecast:true}],url:"https://typhoon.nmc.cn/"}];show(info);
 expect(screen.getByText("强度：台风")).toBeTruthy();expect(screen.getByRole("img",{name:"2630 · 测试台风官方预报路径地图"})).toBeTruthy();expect(screen.queryByText("最近较为风平浪静")).toBeNull();fireEvent.click(screen.getByRole("link",{name:"中气爱 · 中国气象爱好者"}));await waitFor(()=>expect(api.open).toHaveBeenCalledWith("https://weibo.com/tybbs"));
});
it("缺失体感和湿度显示未知，不编造零值或穿衣判断",()=>{
 const info=data();info.forecast={fetchedAt:new Date().toISOString(),feelsLike:null,humidity:null,windSpeed:null,windDirection:null,hourly:[],daily:[]};show(info);
 expect(screen.getByText("体感数据暂缺，穿衣建议待更新")).toBeTruthy();expect(screen.queryByText("0%")).toBeNull();expect(screen.getByText("温度趋势数据暂缺")).toBeTruthy();
});
it("温暖天气的阵雨代码显示阵雨，不误报雨雪",()=>{
 const info=data();info.forecast={fetchedAt:new Date().toISOString(),feelsLike:20,humidity:60,windSpeed:10,windDirection:90,hourly:[],daily:[{date:"2026-10-22",low:18,high:23,rain:80,uv:1,sunrise:null,sunset:null,code:80}]};show(info);
 expect(screen.getByText("阵雨")).toBeTruthy();expect(screen.queryByText("雨雪")).toBeNull();
});

it("关怀区始终可见，失败时只由手动重试调用模型入口",async()=>{
 const info=data();info.warnings.items=[{id:"w",title:"暴雨预警",description:"防范",level:"orange",issuedAt:"now",url:"https://www.nmc.cn/",source:"气象台"}];
 info.warnings.careStatus={"1":{eventKey:"event",state:"failed",error:"模型服务连接失败"}};
 const refresh=vi.fn().mockResolvedValue(undefined);api.retryCare.mockResolvedValue(undefined);show(info,"1",refresh);
 expect(screen.getByLabelText("台风角色关怀")).toBeTruthy();expect(screen.getByText("模型服务连接失败")).toBeTruthy();expect(api.retryCare).not.toHaveBeenCalled();
 fireEvent.click(screen.getByRole("button",{name:"重试关怀"}));await waitFor(()=>expect(refresh).toHaveBeenCalledTimes(1));
 expect(api.retryCare).toHaveBeenCalledWith(7,"warnings");expect(api.refreshInformation).not.toHaveBeenCalled();
});
