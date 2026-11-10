import {beforeEach,expect,it,vi} from "vitest";
const invoke=vi.hoisted(()=>vi.fn());
vi.mock("@tauri-apps/api/core",()=>({invoke}));
import {cardBrowserApi} from "./api";
const context={personaId:"1",sessionId:"2",contextRevision:3};
beforeEach(()=>invoke.mockReset());
it("收藏条件以favoritesOnly送入分页查询，与搜索和出行条件组合",async()=>{
 invoke.mockResolvedValue({items:[],total:0,page:1,pages:1});
 await cardBrowserApi.browse("letters",context,"灯塔","oldest",2,["7"],true);
 expect(invoke).toHaveBeenCalledWith("browse_cards",{input:{kind:"letters",context,search:"灯塔",order:"oldest",page:2,travelIds:["7"],favoritesOnly:true}});
});
it("未启用收藏筛选的原有调用保持请求兼容",async()=>{
 invoke.mockResolvedValue({items:[],total:0,page:1,pages:1});
 await cardBrowserApi.browse("journals",context,"","newest",1);
 expect(invoke).toHaveBeenCalledWith("browse_cards",{input:{kind:"journals",context,search:"",order:"newest",page:1,travelIds:[]}});
});
