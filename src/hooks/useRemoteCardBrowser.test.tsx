import {act,cleanup,renderHook,waitFor} from "@testing-library/react";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {useRemoteCardBrowser} from "./useRemoteCardBrowser";
const api=vi.hoisted(()=>({browse:vi.fn(),listen:vi.fn()}));
vi.mock("../api",()=>({cardBrowserApi:api,errorMessage:(e:Error)=>e.message}));
const context={personaId:"1",sessionId:"1",contextRevision:1};
const page=(name:string)=>({items:[{id:name}],total:1,page:1,pages:1,travels:[]});
beforeEach(()=>{vi.resetAllMocks();api.browse.mockResolvedValue(page("current"));api.listen.mockResolvedValue(()=>{});});
afterEach(cleanup);
it("收藏条件回首页，刷新保留搜索和出行，切换会话重置",async()=>{
 let changed!:()=>void;api.listen.mockImplementation(async(_,receive)=>{changed=receive;return()=>{};});
 const {result,rerender}=renderHook(scope=>useRemoteCardBrowser<{id:string}>("letters",scope),{initialProps:context});
 await waitFor(()=>expect(result.current.loading).toBe(false));
 act(()=>{result.current.setSearch("灯塔");result.current.setTravelIds(["7"]);result.current.setPage(3);});
 await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("letters",context,"灯塔","newest",3,["7"]));
 act(()=>result.current.setFavoritesOnly(true));await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("letters",context,"灯塔","newest",1,["7"],true));
 act(()=>result.current.setPage(2));await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("letters",context,"灯塔","newest",2,["7"],true));
 const key=result.current.selectionKey;await act(async()=>changed());await waitFor(()=>expect(result.current.loading).toBe(false));expect(result.current.favoritesOnly).toBe(true);expect(result.current.selectionKey).toBe(key);
 const next={...context,sessionId:"2",contextRevision:2};rerender(next);await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("letters",next,"","newest",1,[]));expect(result.current.favoritesOnly).toBe(false);
});
it("收藏筛选完成后迟到的全部列表不会覆盖结果",async()=>{
 const old:((value:ReturnType<typeof page>)=>void)[]=[];
 api.browse.mockImplementation((...args:unknown[])=>args[6]===true?Promise.resolve(page("favorite")):new Promise(resolve=>old.push(resolve)));
 const {result}=renderHook(()=>useRemoteCardBrowser<{id:string}>("souvenirs",context));await waitFor(()=>expect(old.length).toBeGreaterThan(0));
 act(()=>result.current.setFavoritesOnly(true));await waitFor(()=>expect(result.current.items[0]?.id).toBe("favorite"));
 await act(async()=>old.forEach(resolve=>resolve(page("unfiltered"))));expect(result.current.items[0]?.id).toBe("favorite");
});
it("订阅建立后补查并拒绝较早的迟到结果",async()=>{
 let old!:(value:ReturnType<typeof page>)=>void,ready!:(stop:()=>void)=>void;
 api.browse.mockImplementationOnce(()=>new Promise(resolve=>{old=resolve;}));
 api.listen.mockImplementationOnce(()=>new Promise(resolve=>{ready=resolve;}));
 const {result}=renderHook(()=>useRemoteCardBrowser<{id:string}>("letters",context));
 await waitFor(()=>expect(api.browse).toHaveBeenCalledTimes(1));
 await act(async()=>ready(()=>{}));await waitFor(()=>expect(result.current.items[0]?.id).toBe("current"));
 await act(async()=>old(page("stale")));expect(result.current.items[0]?.id).toBe("current");
});
it("事件补查保留关键词、分页、出行筛选及详情选择键",async()=>{
 let changed!:()=>void;api.listen.mockImplementation(async(_,receive)=>{changed=receive;return()=>{};});
 const {result}=renderHook(()=>useRemoteCardBrowser<{id:string}>("journals",context));
 await waitFor(()=>expect(result.current.loading).toBe(false));
 act(()=>{result.current.setSearch("灯塔");result.current.setTravelIds(["7"]);result.current.setPage(2);});
 await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("journals",context,"灯塔","newest",2,["7"]));
 const key=result.current.selectionKey;
 await act(async()=>changed());await waitFor(()=>expect(result.current.loading).toBe(false));
 expect(result.current.selectionKey).toBe(key);expect(result.current.search).toBe("灯塔");expect(result.current.travelIds).toEqual(["7"]);
 expect(api.browse).toHaveBeenLastCalledWith("journals",context,"灯塔","newest",2,["7"]);
});
it("关闭后才建立的订阅立即释放且不补查",async()=>{
 let ready!:(stop:()=>void)=>void;const stop=vi.fn();api.listen.mockImplementation(()=>new Promise(resolve=>{ready=resolve;}));
 const {unmount}=renderHook(()=>useRemoteCardBrowser("souvenirs",context));
 await waitFor(()=>expect(api.browse).toHaveBeenCalledTimes(1));unmount();
 await act(async()=>ready(stop));expect(stop).toHaveBeenCalledTimes(1);expect(api.browse).toHaveBeenCalledTimes(1);
});
it("切换会话后旧查询不能覆盖当前列表",async()=>{
 let old!:(value:ReturnType<typeof page>)=>void;
 api.browse.mockImplementation((_,scope)=>scope.sessionId==="1"?new Promise(resolve=>{old=resolve;}):Promise.resolve(page("new-session")));
 const {result,rerender}=renderHook(scope=>useRemoteCardBrowser<{id:string}>("accessories",scope),{initialProps:context});
 await waitFor(()=>expect(api.browse).toHaveBeenCalled());
 rerender({...context,sessionId:"2",contextRevision:2});await waitFor(()=>expect(result.current.items[0]?.id).toBe("new-session"));
 await act(async()=>old(page("old-session")));expect(result.current.items[0]?.id).toBe("new-session");
});
