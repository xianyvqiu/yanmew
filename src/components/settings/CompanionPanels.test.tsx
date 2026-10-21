import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import FoodPanel from "./FoodPanel";
import SkillPanel from "./SkillPanel";
const mock = vi.hoisted(() => ({ foods: vi.fn(), buy: vi.fn(), receipt: vi.fn(), skills: vi.fn(), saveSkill: vi.fn(), deleteSkill: vi.fn(), bind: vi.fn(), listPersonas: vi.fn(), open: vi.fn(), getWallet: vi.fn(), listenSkills:async()=>()=>{}, listenFoods: vi.fn() }));
vi.mock("../../companionApi", () => ({ companionApi: mock, foodImage: (id: string) => `${id}.png` }));
vi.mock("../../api", () => ({ configurationApi: { listPersonas: mock.listPersonas }, walletApi: { get: mock.getWallet }, errorMessage: (e: Error) => e.message }));
vi.mock("../../platform/navigation", () => ({ navigation: { openSettingsDetail: mock.open } }));
afterEach(cleanup);
beforeEach(() => { vi.resetAllMocks(); mock.open.mockResolvedValue(undefined); mock.getWallet.mockResolvedValue({ coins: "50" }); mock.listenFoods.mockResolvedValue(() => { }); mock.foods.mockResolvedValue([{ id: "token", name: "Token小点心", unlimited: true, quantity: 0, price: 0, revision: 0, createdAt: "2026-10-19", updatedAt: "2026-10-19" }, { id: "rice", name: "白饭", unlimited: false, quantity: 0, price: 5, revision: 0, createdAt: "2026-10-19", updatedAt: "2026-10-19" }]); mock.listPersonas.mockResolvedValue([{ id: "1", name: "电子小猫" }]); mock.skills.mockResolvedValue([{ id: "1", title: "自然表达", summary: "简洁", body: "保留自己的语气", revision: 0, createdAt: "2026-10-19", updatedAt: "2026-10-19", personas: [{ id: "1", name: "电子小猫" }] }]); });
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
