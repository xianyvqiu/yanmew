import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import FoodPanel from "./FoodPanel";
import SkillPanel from "./SkillPanel";
const mock = vi.hoisted(() => ({ foods: vi.fn(), buy: vi.fn(), receipt: vi.fn(), skills: vi.fn(), saveSkill: vi.fn(), deleteSkill: vi.fn(), bind: vi.fn(), listPersonas: vi.fn(), open: vi.fn(), getWallet: vi.fn(), listenWallet: vi.fn(), listenSkills:vi.fn(), listenFoods: vi.fn() }));
vi.mock("../../companionApi", () => ({ companionApi: mock, foodImage: (id: string) => `${id}.png` }));
vi.mock("../../api", () => ({ configurationApi: { listPersonas: mock.listPersonas }, walletApi: { get: mock.getWallet, listen:mock.listenWallet }, errorMessage: (e: Error) => e.message }));
vi.mock("../../platform/navigation", () => ({ navigation: { openSettingsDetail: mock.open } }));
afterEach(cleanup);
it("Skill通知与聚焦补查状态，旧查询不会盖回旧绑定",async()=>{
 let changed!:()=>void,old!:(items:unknown[])=>void;
 mock.listenSkills.mockImplementation(async(receive)=>{changed=receive;return()=>{};});
 render(<SkillPanel/>);fireEvent.click(await screen.findByRole("button",{name:/自然表达/}));
 expect(screen.getByRole("button",{name:"解除绑定"})).toBeTruthy();
 const items=await mock.skills();mock.skills.mockImplementationOnce(()=>new Promise(resolve=>{old=resolve;}));act(()=>changed());
 mock.skills.mockResolvedValue(items.map((item:any)=>({...item,personas:[]})));
 await act(async()=>window.dispatchEvent(new Event("focus")));
 await waitFor(()=>expect(screen.queryByRole("button",{name:"解除绑定"})).toBeNull());
 await act(async()=>old(items));expect(screen.queryByRole("button",{name:"解除绑定"})).toBeNull();
});
it("钱包通知立即更新食物页余额，不依赖食物通知或重新聚焦",async()=>{
 let walletChanged!:()=>void;mock.listenWallet.mockImplementation(async(receive)=>{walletChanged=receive;return()=>{};});
 render(<FoodPanel/>);await screen.findByText("余额 50 喵币");
 mock.getWallet.mockResolvedValue({coins:"70"});await act(async()=>walletChanged());
 expect(await screen.findByText("余额 70 喵币")).toBeTruthy();
});
it("食物页旧查询迟到不覆盖新的库存和余额",async()=>{
 let changed!:()=>void,oldWallet!:(value:{coins:string})=>void,oldFood!:(value:unknown[])=>void;
 mock.listenWallet.mockImplementation(async(receive)=>{changed=receive;return()=>{};});
 render(<FoodPanel/>);await screen.findByText("余额 50 喵币");
 const foods=await mock.foods();
 mock.getWallet.mockImplementationOnce(()=>new Promise(resolve=>{oldWallet=resolve;}));
 mock.foods.mockImplementationOnce(()=>new Promise(resolve=>{oldFood=resolve;}));
 act(()=>changed());mock.getWallet.mockResolvedValue({coins:"30"});mock.foods.mockResolvedValue(foods.map((f:any)=>({...f,quantity:f.id==="rice"?4:f.quantity})));
 await act(async()=>changed());await screen.findByText("余额 30 喵币");fireEvent.click(screen.getByRole("button",{name:"商城"}));
 await act(async()=>{oldWallet({coins:"40"});oldFood(foods);});
 expect(screen.getByText("余额 30 喵币")).toBeTruthy();expect(screen.getByText("5 喵币／份 · 拥有 4")).toBeTruthy();
});
it("食物购买用弹窗选择数量，取消不扣币",async()=>{render(<FoodPanel/>);await screen.findByText("Token小点心");fireEvent.click(screen.getByRole("button",{name:"商城"}));fireEvent.click(screen.getByRole("button",{name:/白饭/}));expect(screen.getByRole("dialog",{name:"购买白饭"})).toBeTruthy();fireEvent.change(screen.getByLabelText("购买数量"),{target:{value:"3"}});expect(screen.getByText("共 15 喵币")).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"取消"}));expect(screen.queryByRole("dialog")).toBeNull();expect(mock.buy).not.toHaveBeenCalled();});
beforeEach(() => { vi.resetAllMocks(); mock.open.mockResolvedValue(undefined); mock.listenSkills.mockResolvedValue(()=>{}); mock.getWallet.mockResolvedValue({ coins: "50" }); mock.listenWallet.mockResolvedValue(()=>{}); mock.listenFoods.mockResolvedValue(() => { }); mock.foods.mockResolvedValue([{ id: "token", name: "Token小点心", unlimited: true, quantity: 0, price: 0, revision: 0, createdAt: "2026-10-19", updatedAt: "2026-10-19" }, { id: "rice", name: "白饭", unlimited: false, quantity: 0, price: 5, revision: 0, createdAt: "2026-10-19", updatedAt: "2026-10-19" }]); mock.listPersonas.mockResolvedValue([{ id: "1", name: "电子小猫" }]); mock.skills.mockResolvedValue([{ id: "1", title: "自然表达", summary: "简洁", body: "保留自己的语气", revision: 0, createdAt: "2026-10-19", updatedAt: "2026-10-19", personas: [{ id: "1", name: "电子小猫" }] }]); });
it("仓库只显示拥有食物，商城购买使用显示报价并显示实际扣款", async () => { mock.buy.mockResolvedValue({ coinDelta: -5 }); render(<FoodPanel />); await screen.findByText("Token小点心"); expect(screen.queryByText("白饭")).toBeNull(); fireEvent.click(screen.getByRole("button", { name: "商城" })); fireEvent.click(screen.getByRole("button", { name: /白饭/ })); fireEvent.click(screen.getByRole("button", { name: "确认购买" })); await waitFor(() => expect(mock.buy).toHaveBeenCalledWith(expect.any(String), "rice", 1, 0)); expect(await screen.findByText("-5 喵币")).toBeTruthy(); });
it("Skill卡片进入详情，显示正文、绑定状态并能跳转人设", async () => { render(<SkillPanel />); fireEvent.click(await screen.findByRole("button", { name: /自然表达/ })); expect(screen.getByText("保留自己的语气")).toBeTruthy(); expect(screen.getByRole("button", { name: "解除绑定" })).toBeTruthy(); fireEvent.click(screen.getByRole("button", { name: "人设详情" })); expect(mock.open).toHaveBeenCalledWith("persona", "1"); fireEvent.click(screen.getByRole("button", { name: "解除绑定" })); await waitFor(() => expect(mock.bind).toHaveBeenCalledWith("1", "1", false)); });
it("购买成功后刷新失败仍确认扣款并关闭购买详情", async () => {
    mock.buy.mockImplementation(async () => {
        mock.foods.mockRejectedValue(new Error("暂时断开"));
        return { actionId: "purchase-once", coinDelta: -5 };
    });
    render(<FoodPanel />);
    await screen.findByText("Token小点心");
    fireEvent.click(screen.getByRole("button", { name: "商城" }));
    fireEvent.click(screen.getByRole("button", { name: /白饭/ }));
    fireEvent.click(screen.getByRole("button", { name: "确认购买" }));
    expect(await screen.findByText(/购买已成功，暂时无法刷新库存/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "确认购买" })).toBeNull();
    expect(mock.receipt).not.toHaveBeenCalled();
    expect(mock.buy).toHaveBeenCalledTimes(1);
});
it("购买成功先展示回执，停留后完成退出动画才关闭，不能重复购买",async()=>{
    mock.buy.mockResolvedValue({actionId:"once",quantity:2,coinDelta:-10});
    render(<FoodPanel/>);await screen.findByText("Token小点心");
    fireEvent.click(screen.getByRole("button",{name:"商城"}));fireEvent.click(screen.getByRole("button",{name:/白饭/}));
    fireEvent.change(screen.getByLabelText("购买数量"),{target:{value:"2"}});
    const modal=screen.getByRole("dialog");let finishExit!:()=>void;
    const animate=vi.fn(()=>({finished:new Promise<void>(resolve=>{finishExit=resolve;}),cancel:vi.fn()}));
    Object.defineProperty(modal,"animate",{configurable:true,value:animate});
    vi.useFakeTimers();
    try {
        fireEvent.click(within(modal).getByRole("button",{name:"确认购买"}));
        await act(async()=>{await Promise.resolve();});
        expect(within(modal).getByText("购买成功")).toBeTruthy();
        expect(within(modal).getByText("白饭 × 2 已加入仓库")).toBeTruthy();
        expect(within(modal).queryByRole("button",{name:"确认购买"})).toBeNull();
        expect(mock.buy).toHaveBeenCalledTimes(1);
        await act(async()=>{vi.advanceTimersByTime(1099);});expect(animate).not.toHaveBeenCalled();
        await act(async()=>{vi.advanceTimersByTime(1);});expect(animate).toHaveBeenCalledTimes(1);expect(screen.getByRole("dialog")).toBe(modal);
        await act(async()=>{finishExit();});expect(screen.queryByRole("dialog")).toBeNull();
    } finally { vi.useRealTimers(); }
});
it("购买失败保留数量与弹窗，不显示成功",async()=>{
    mock.buy.mockRejectedValue(new Error("余额不足"));mock.receipt.mockResolvedValue(null);
    render(<FoodPanel/>);await screen.findByText("Token小点心");fireEvent.click(screen.getByRole("button",{name:"商城"}));fireEvent.click(screen.getByRole("button",{name:/白饭/}));
    fireEvent.click(screen.getByRole("button",{name:"确认购买"}));
    expect(await screen.findByText("余额不足")).toBeTruthy();expect(screen.queryByText("购买成功")).toBeNull();expect(screen.getByLabelText("购买数量")).toBeTruthy();expect(screen.getByRole("dialog")).toBeTruthy();
});
it("新建Skill成功后刷新失败会退出编辑，避免误报未保存", async () => {
    mock.saveSkill.mockImplementation(async () => { mock.skills.mockRejectedValue(new Error("暂时断开")); });
    render(<SkillPanel />);
    await screen.findByRole("button", { name: /自然表达/ });
    fireEvent.click(screen.getByRole("button", { name: "新建 Skill" }));
    fireEvent.change(screen.getByLabelText("标题"), { target: { value: "清晰表达" } });
    fireEvent.change(screen.getByLabelText("Skill 正文"), { target: { value: "先说结论" } });
    fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(await screen.findByText(/操作已成功，暂时无法刷新列表/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "保存" })).toBeNull();
    expect(mock.saveSkill).toHaveBeenCalledTimes(1);
});
