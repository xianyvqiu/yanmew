import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { moneyInput, moneyText, type RealOverview, type RealReport } from "../../realMoneyApi";
import RealMoneyPanel from "./RealMoneyPanel";
import { RealAssetEditor, RealEntryEditor } from "./RealMoneyForms";
import RealMoneyBudgets from "./RealMoneyBudgets";
const mocks=vi.hoisted(()=>({overview:vi.fn(),mutate:vi.fn(),receipt:vi.fn(),tasks:vi.fn(),query:vi.fn()}));
vi.mock("../../realMoneyApi",async()=>({...await vi.importActual("../../realMoneyApi"),realMoneyApi:{...mocks,listen:async()=>()=>{},listenError:async()=>()=>{}}}));
vi.mock("../../api",()=>({errorMessage:(e:Error)=>e.message}));
const asset={id:"1",name:"默认资产",note:"",isDefault:true,archived:false,revision:1,balanceCents:"10000",createdAt:"2026-10-20T00:00:00Z",updatedAt:"2026-10-20T00:00:00Z"};
const base:RealOverview={assets:[asset],categories:[{id:"1",name:"食物",icon:"🍜",direction:"expense",archived:false,revision:1,createdAt:asset.createdAt,updatedAt:asset.updatedAt}],totalAssetsCents:"10000",today:"2026-10-20",todaySummary:{income:"0",expense:"1234",net:"-1234",count:1},monthSummary:{income:"10000",expense:"1234",net:"8766",count:2},budgets:(["day","month","year"]as const).map(period=>({period,amountCents:null,spentCents:"1234",remainingCents:null,averageDailyCents:"61",remainingDailyCents:null,elapsedDays:20,remainingDays:12})),pending:{count:2,throughId:"7",incomeCents:"0",expenseCents:"1000"},activeSchedules:1};
beforeEach(()=>{vi.clearAllMocks();mocks.overview.mockResolvedValue(base);mocks.mutate.mockResolvedValue({id:"1"});mocks.receipt.mockResolvedValue(null);mocks.tasks.mockResolvedValue({items:[{id:"6",scheduleId:"1",scheduledLocal:"2026-10-18T08:00",entry:{title:"每日早餐",kind:"expense",categoryId:"1",assetId:"1",description:"",amount:"5.00",occurredLocal:"2026-10-18T08:00",recorder:"用户"}}],total:2,pages:1,page:1});Object.defineProperty(HTMLDialogElement.prototype,"showModal",{configurable:true,value:function(this:HTMLDialogElement){this.setAttribute("open","");}});Object.defineProperty(HTMLDialogElement.prototype,"close",{configurable:true,value:function(this:HTMLDialogElement){this.removeAttribute("open");}});});
afterEach(()=>{cleanup();vi.restoreAllMocks();});
describe("现实记账模块入口",()=>{
 it("主页不展开预算表单或筛选，进入明细和数据面板各司其职",async()=>{
    const summary={income:"10000",expense:"1234",net:"8766",count:2};
    const report:RealReport={entries:[],summary,balance:"10000",page:1,totalPages:1,categories:[],assets:[],timeline:[{key:"2026-10-20",name:"2026-10-20",summary}],bucket:"day",rangeStart:"2026-10-01",rangeEnd:"2026-10-20"};
    mocks.query.mockResolvedValue(report);render(<RealMoneyPanel/>);
    await screen.findByRole("button",{name:"预算"});
    expect(screen.queryByLabelText("每日预算（元）")).toBeNull();expect(screen.queryByText("日均消费")).toBeNull();
    expect(screen.queryByLabelText("现实账单筛选")).toBeNull();expect(screen.queryByText("区间收入")).toBeNull();
    expect(screen.getByText("2 项待确认")).toBeTruthy();expect(mocks.query).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button",{name:"账单"}));await screen.findByText("没有符合条件的现实账单");
    expect(screen.queryByText("总资产")).toBeNull();expect(screen.queryByText("区间收入")).toBeNull();
    fireEvent.click(screen.getByRole("button",{name:"筛选账单"}));
    fireEvent.change(screen.getByLabelText("分类"),{target:{value:"1"}});
    await waitFor(()=>expect(mocks.query.mock.lastCall?.[0].categoryId).toBe("1"));
    fireEvent.click(screen.getByRole("button",{name:"账单数据面板"}));await screen.findByRole("img",{name:/收支柱状图/});
    expect(screen.getByText("区间收入")).toBeTruthy();expect(screen.queryByLabelText("现实账单分页")).toBeNull();
    fireEvent.click(screen.getByRole("button",{name:"返回账单"}));
    fireEvent.click(screen.getByRole("button",{name:"筛选账单"}));
    expect((screen.getByLabelText("分类")as HTMLSelectElement).value).toBe("1");
    fireEvent.click(screen.getByRole("button",{name:"返回现实记账"}));fireEvent.click(screen.getByRole("button",{name:"预算"}));
    expect(screen.getByLabelText("每日预算（元）")).toBeTruthy();expect(screen.queryByText("总资产")).toBeNull();
 });
});
describe("现实记账金额与表单",()=>{
 it("以整数分显示超出JS安全整数的金额，编辑值不丢精度",()=>{expect(moneyText("9007199254740993")).toBe("90,071,992,547,409.93");expect(moneyInput("-1234")).toBe("-12.34");});
 it("手动录入传递元的文本、资产、分类和记账者",async()=>{const run=vi.fn().mockResolvedValue(undefined);render(<RealEntryEditor overview={base} busy={false} run={run} onClose={()=>{}}/>);fireEvent.change(screen.getByLabelText("标题"),{target:{value:"午餐"}});fireEvent.change(screen.getByLabelText("金额（人民币元）"),{target:{value:"12.34"}});fireEvent.click(screen.getByRole("button",{name:"保存"}));await waitFor(()=>expect(run).toHaveBeenCalledWith(expect.objectContaining({type:"save_entry",recurrence:null,entry:expect.objectContaining({amount:"12.34",assetId:"1",categoryId:"1",recorder:"用户"})}),expect.any(Function)));});
 it("只改资产名不回写打开表单时的旧余额",()=>{const run=vi.fn().mockResolvedValue(undefined);render(<RealAssetEditor asset={asset} busy={false} run={run} onClose={()=>{}}/>);fireEvent.change(screen.getByLabelText("资产名称"),{target:{value:"日常资产"}});fireEvent.click(screen.getByRole("button",{name:"保存"}));expect(run.mock.calls[0][0]).toMatchObject({type:"save_asset",name:"日常资产",balance:null});});
 it("预算空值表示不设，0表示零预算，保持文本精度",()=>{const run=vi.fn().mockResolvedValue(undefined);render(<RealMoneyBudgets overview={base} busy={false} run={run}/>);fireEvent.change(screen.getByLabelText("每日预算（元）"),{target:{value:"0"}});fireEvent.change(screen.getByLabelText("每月预算（元）"),{target:{value:"1234.56"}});fireEvent.click(screen.getByRole("button",{name:"保存预算"}));expect(run.mock.calls[0][0]).toEqual({type:"save_budgets",day:"0",month:"1234.56",year:null});});
});
describe("漏期确认与写入重试",()=>{
 it("先展示漏期、取消保留勾选，确认后仅补选中项目",async()=>{render(<RealMoneyPanel request={{date:null}}/>);const checkbox=await screen.findByRole("checkbox",{name:/选择 每日早餐/});fireEvent.click(checkbox);fireEvent.click(screen.getByRole("button",{name:"补记已选"}));expect(mocks.mutate).not.toHaveBeenCalled();fireEvent.click(within(screen.getByRole("dialog")).getByRole("button",{name:"取消"}));expect((checkbox as HTMLInputElement).checked).toBe(true);fireEvent.click(screen.getByRole("button",{name:"补记已选"}));fireEvent.click(within(screen.getByRole("dialog")).getByRole("button",{name:"确认"}));await waitFor(()=>expect(mocks.mutate).toHaveBeenCalledWith(expect.any(String),{type:"resolve_missed",ids:["6"],throughId:null,record:true}));});
 it("全部补记使用页面已知的最大期次编号，未来新发现的不混入",async()=>{render(<RealMoneyPanel request={{date:null}}/>);fireEvent.click(await screen.findByRole("button",{name:"全部补记（2项）"}));expect(mocks.mutate).not.toHaveBeenCalled();fireEvent.click(within(screen.getByRole("dialog")).getByRole("button",{name:"确认"}));await waitFor(()=>expect(mocks.mutate.mock.calls[0]?.[1]).toEqual({type:"resolve_missed",ids:[],throughId:"7",record:true}));});
 it("写入结果不明时在弹窗里重试相同操作编号",async()=>{mocks.mutate.mockRejectedValueOnce(new Error("连接中断"));mocks.receipt.mockRejectedValueOnce(new Error("无法核对"));render(<RealMoneyPanel request={{date:null}}/>);fireEvent.click(await screen.findByRole("button",{name:"全部跳过"}));fireEvent.click(within(screen.getByRole("dialog")).getByRole("button",{name:"确认"}));const retry=await within(screen.getByRole("dialog")).findByRole("button",{name:"核对并重试原操作"});const first=mocks.mutate.mock.calls[0];fireEvent.click(retry);await waitFor(()=>expect(mocks.mutate).toHaveBeenCalledTimes(2));expect(mocks.mutate.mock.calls[1]).toEqual(first);await waitFor(()=>expect(screen.queryByRole("dialog")).toBeNull());});
});
