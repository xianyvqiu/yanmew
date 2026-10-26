import {cleanup,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import GameSaveSummary from "./GameSaveSummary";
import type {PluginJson} from "../types";
afterEach(cleanup);
const saved=(progress:Record<string,PluginJson>,pending:PluginJson=null)=>({"state.main":{version:1,progress,pending}});
const today=()=>{const d=new Date();return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")}`;};
it("2048只显示状态和三个重点，不展示棋盘、内部字段或历史数组，也不修改存档",()=>{
 const data=saved({day:today(),paidRounds:2,bestScore:1200,totalPlayed:99,totalWins:7,current:{settled:false,highest:512,score:100,moves:60,board:[2,4,8,16],seed:"内部种子"},history:["不应展示的历史"],secret:"内部数据"});const before=JSON.stringify(data);
 const {container}=render(<GameSaveSummary data={data} pluginId="com.yanmew.game.2048"/>);
 expect(screen.getByText("有未结束的棋局")).toBeTruthy();expect(screen.getByText("最高得分")).toBeTruthy();expect(screen.getByText("512")).toBeTruthy();expect(screen.getByText("2 局")).toBeTruthy();
 expect(container.querySelectorAll("dt")).toHaveLength(3);expect(container.querySelector("details,pre,code")).toBeNull();
 expect(container.textContent).not.toMatch(/内部数据|内部种子|不应展示的历史|移动次数|棋盘|累计局数/);expect(JSON.stringify(data)).toBe(before);
});
it("扫雷展示通关和本局难度，不展示每个格子",()=>{
 const {container}=render(<GameSaveSummary pluginId="com.yanmew.game.minesweeper" data={saved({totalWins:8,current:{level:"hard",settled:false,cells:[{mine:true,number:3}]}})}/>);
 expect(screen.getByText("累计通关")).toBeTruthy();expect(screen.getByText("8 次")).toBeTruthy();expect(screen.getByText("高级")).toBeTruthy();expect(container.textContent).not.toMatch(/mine|格子|number/);
});
it("我的家只显示生活技能和解锁区域，不铺开物品仓库",()=>{
 const {container}=render(<GameSaveSummary pluginId="com.yanmew.game.my-home" data={saved({affinity:80,farm:{enabled:true,readyAt:123},barn:{enabled:false},inventory:{"番茄":40,"巧克力":8}})}/>);
 expect(screen.getByText("80")).toBeTruthy();expect(screen.getByText("田地")).toBeTruthy();expect(container.querySelectorAll("dt")).toHaveLength(2);expect(container.textContent).not.toMatch(/番茄|巧克力|readyAt|可收获时间/);
});
it("未知插件和额外存档键只提示已保存，不猜测或输出任意字段",()=>{
 const {container}=render(<GameSaveSummary pluginId="third.party.game" data={{unknown:{private:"不应显示",board:[1,2,3]},extra:"extra-key-value","state.main":{pending:{request_id:"第三方请求"},progress:{bestScore:9999}}}}/>);
 expect(screen.getByText("进度已保存")).toBeTruthy();expect(container.querySelector("dl,details")).toBeNull();expect(container.textContent).not.toMatch(/不应显示|extra-key-value|进度记录|游戏存档 2/);
});
it("空存档不伪造分数或继续游戏状态",()=>{
 const {container}=render(<GameSaveSummary pluginId="com.yanmew.game.2048" data={{}}/>);
 expect(screen.getByText("还没有存档，玩一局后会自动保存。")).toBeTruthy();expect(container.querySelector("dl")).toBeNull();expect(screen.queryByText("进度已保存")).toBeNull();
});
it("跨日期后今日赚钱次数归零，不将旧局数显示为今天",()=>{
 const {container}=render(<GameSaveSummary pluginId="com.yanmew.game.dice" data={saved({day:"2000-01-01",usedToday:17,walletMode:true})}/>);
 expect(screen.getByText("0 局")).toBeTruthy();expect(container.textContent).not.toContain("17");
});
it("待核对结算只保留一句恢复说明，不显示请求内容",()=>{
 const {container}=render(<GameSaveSummary pluginId="com.yanmew.game.2048" data={saved({bestScore:0},{request:{request_id:"内部请求编号",delta:20},after:{score:500}})}/>);
 expect(screen.getByText("进度等待恢复")).toBeTruthy();expect(screen.getByText("打开游戏后会自动核对上次结算。")).toBeTruthy();expect(container.textContent).not.toMatch(/内部请求编号|500|delta/);
});
it("非法摘要数值不作为玩家成绩展示",()=>{
 const {container}=render(<GameSaveSummary pluginId="com.yanmew.game.2048" data={saved({bestScore:-10,day:today(),paidRounds:"乱码",current:{highest:-2}})}/>);
 expect(screen.getByText("进度已保存")).toBeTruthy();expect(container.querySelector("dl")).toBeNull();
});
it("第三方作者通过统一摘要展示玩家进度，文本不会变成HTML",()=>{
 const data={"state.custom":{private:"内部字段"},"ui.summary":{version:1,status:"playing",metrics:[{label:"目前关卡",value:"第 8 关"},{label:"收藏",value:"<img src=x>"}]}};
 const before=JSON.stringify(data);const {container}=render(<GameSaveSummary pluginId="third.party.game" data={data}/>);
 expect(screen.getByText("游戏进行中")).toBeTruthy();expect(screen.getByText("第 8 关")).toBeTruthy();expect(screen.getByText("<img src=x>")).toBeTruthy();
 expect(container.querySelector("img")).toBeNull();expect(container.textContent).not.toContain("内部字段");expect(JSON.stringify(data)).toBe(before);
});
const invalidSummaries:PluginJson[]=[
 {version:2,status:"completed"},
 {version:1,status:["playing"]},
 {version:1,status:null},
 {version:1,metrics:null},
 {version:1,metrics:Array.from({length:4},()=>({label:"指标",value:"1"}))},
 {version:1,metrics:[{label:" ",value:"1"}]},
 {version:1,metrics:[{label:"指标",value:"a".repeat(41)}]},
 {version:1,metrics:[{label:"指标\n",value:"1"}]},
 {version:1,html:"<div>"}
 ];
it.each(invalidSummaries)("无效摘要安全回退，不展示内部字段 %j",value=>{
 const {container}=render(<GameSaveSummary pluginId="third.party.game" data={{"ui.summary":value}}/>);
 expect(screen.getByText("进度已保存")).toBeTruthy();expect(container.querySelector("dl")).toBeNull();
});
it("摘要是可选的，通关状态统一显示，且不能覆盖内置游戏结算恢复提示",()=>{
 const {rerender}=render(<GameSaveSummary pluginId="third.party.game" data={{"ui.summary":{version:1,status:"completed"}}}/>);
 expect(screen.getByText("已通关")).toBeTruthy();
 rerender(<GameSaveSummary pluginId="com.yanmew.game.2048" data={{...saved({bestScore:2},{request:"pending"}),"ui.summary":{version:1,status:"completed",metrics:[{label:"最高数字",value:"2048"}]}}}/>);
 expect(screen.getByText("进度等待恢复")).toBeTruthy();expect(screen.queryByText("已通关")).toBeNull();
});
