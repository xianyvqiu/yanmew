import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { coinText, ledgerRange, type LedgerReport } from "../../ledgerApi";
import LedgerPanel from "./LedgerPanel";
const query=vi.hoisted(()=>vi.fn());
vi.mock("../../ledgerApi",async()=>({...await vi.importActual("../../ledgerApi"),ledgerApi:{query}}));
vi.mock("../../api",()=>({walletApi:{listen:async()=>()=>{}},errorMessage:(e:Error)=>e.message}));
const base:LedgerReport={entries:[{id:"1",delta:"-5",reason:"food_purchase",pluginId:null,pluginName:null,pluginVersion:null,description:null,refId:null,createdAt:"2026-10-20T01:00:00Z"}],balance:"9007199254740993",summary:{income:"20",expense:"5",net:"15",count:13},totalPages:2,page:1,categories:[{key:"food_purchase",name:"食物购买",summary:{income:"0",expense:"5",net:"-5",count:1}},{key:"daily_sign_in",name:"签到",summary:{income:"20",expense:"0",net:"20",count:12}}],games:[],plugins:[],timeline:[{key:"2026-10-20",name:"2026-10-20",summary:{income:"20",expense:"5",net:"15",count:13}}],bucket:"day",rangeStart:"2026-10-20",rangeEnd:"2026-10-20"};
beforeEach(()=>{query.mockReset();query.mockResolvedValue(base);});
afterEach(cleanup);
describe("账单日期与精度",()=>{
    it("按本地日期计算昨日、七天及闰年月末，不用UTC截取",()=>{
        const day=new Date(2024,2,31,0,15);
        expect(ledgerRange("yesterday",day)).toEqual({start:"2024-03-30",end:"2024-03-30"});
        expect(ledgerRange("week",day)).toEqual({start:"2024-03-25",end:"2024-03-31"});
        expect(ledgerRange("month",day)).toEqual({start:"2024-03-01",end:"2024-03-31"});
        expect(ledgerRange("year",day)).toEqual({start:"2023-04-01",end:"2024-03-31"});
        expect(coinText("9007199254740993")).toBe("9,007,199,254,740,993");
    });
});
describe("账单页面",()=>{
    it("主页只保留钱包和模块入口，明细不重复统计面板",async()=>{
        render(<LedgerPanel/>);await screen.findByText("9,007,199,254,740,993");
        expect(screen.queryByLabelText("账单筛选")).toBeNull();expect(screen.queryByText("-5")).toBeNull();
        fireEvent.click(screen.getByRole("button",{name:"账单明细"}));await screen.findByText("-5");
        expect(screen.queryByText("区间收入")).toBeNull();expect(screen.queryByRole("combobox",{name:"分类"})).toBeNull();
        fireEvent.click(screen.getByRole("button",{name:"账单数据面板"}));
        expect(screen.getByText("区间收入")).toBeTruthy();expect(screen.getByRole("img",{name:/收支柱状图/})).toBeTruthy();
        expect(screen.queryByLabelText("账单分页")).toBeNull();
    });
    it("分页和分类查询交给后端，展开历史详情",async()=>{
        render(<LedgerPanel/>);
        fireEvent.click(screen.getByRole("button",{name:"账单明细"}));
        await screen.findByText("-5");
        fireEvent.click(screen.getByRole("button",{name:/食物购买.*支出/}));
        expect(screen.getByText("账单编号：1")).toBeTruthy();
        fireEvent.click(screen.getByRole("button",{name:"下一页"}));
        await waitFor(()=>expect(query.mock.calls[query.mock.calls.length-1]?.[0].page).toBe(2));
        fireEvent.click(screen.getByRole("button",{name:"筛选账单"}));
        fireEvent.change(screen.getByLabelText("分类"),{target:{value:"food_purchase"}});
        await waitFor(()=>expect(query.mock.calls[query.mock.calls.length-1]?.[0]).toMatchObject({reason:"food_purchase",page:1}));
    });
    it("统计分类可跳回筛选账单，自定义日期先校验",async()=>{
        render(<LedgerPanel/>);await screen.findByText("9,007,199,254,740,993");
        fireEvent.click(screen.getByRole("button",{name:"账单数据面板"}));
        expect(screen.getByRole("img",{name:"收支柱状图：收入向上，支出向下"})).toBeTruthy();
        fireEvent.click(screen.getByRole("button",{name:/食物购买，支出5喵币/}));
        await waitFor(()=>expect(query.mock.calls[query.mock.calls.length-1]?.[0]).toMatchObject({reason:"food_purchase",direction:"expense",page:1}));
        expect(screen.getByRole("button",{name:"账单数据面板"})).toBeTruthy();
        fireEvent.click(screen.getByRole("button",{name:"筛选账单"}));
        await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"自定义"}));});
        fireEvent.change(screen.getByLabelText("开始日期"),{target:{value:"2026-10-21"}});
        fireEvent.change(screen.getByLabelText("结束日期"),{target:{value:"2026-10-20"}});
        fireEvent.click(screen.getByRole("button",{name:"应用日期"}));
        expect(screen.getByRole("alert").textContent).toContain("开始日期不能晚于结束日期");
    });
    it("快速切换查询丢弃过期响应，失败时保留已显示记录",async()=>{
        render(<LedgerPanel/>);fireEvent.click(screen.getByRole("button",{name:"账单明细"}));await screen.findByText("-5");
        fireEvent.click(screen.getByRole("button",{name:"筛选账单"}));
        let finish:(value:LedgerReport)=>void=()=>{};
        query.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
        fireEvent.change(screen.getByLabelText("分类"),{target:{value:"food_purchase"}});
        query.mockResolvedValueOnce({...base,balance:"77"});
        fireEvent.change(screen.getByLabelText("收支"),{target:{value:"expense"}});
        await screen.findByText("77");
        await act(async()=>finish({...base,balance:"999"}));
        expect(screen.queryByText("999")).toBeNull();
        query.mockRejectedValueOnce(new Error("查询失败"));
        fireEvent.click(screen.getByRole("button",{name:"刷新"}));
        await screen.findByRole("alert");
        expect(screen.getByText("77")).toBeTruthy();expect(screen.getByText("-5")).toBeTruthy();
    });
});
