vi.mock("../../hooks/useAccessories",()=>({useAccessories:()=>({items:[],refresh:()=>{}})}));
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import AccessoryPlacementEditor from "./AccessoryPlacementEditor";
const api = vi.hoisted(() => ({ save: vi.fn(), get: vi.fn(), listen: vi.fn(), layer: vi.fn() }));
vi.mock("../../api", () => ({ moveAccessoryLayer: api.layer, saveAccessoryPlacement: api.save, petCreationApi: { get: api.get, listen: api.listen }, errorMessage: (e: Error) => e.message }));
vi.mock("../AccessoryVisual", () => ({ default: () => <span>帽子预览</span> }));
const context = { personaId: "1", sessionId: "2", contextRevision: 3 };
const item = { id: "4", sessionId: "2", name: "帽子", displayText: "☆", imageFile: null, description: "帽子", cost: "10", source: "crafted" as const, travelId: null, isWorn: true, createdAt: "2026-10-16T00:00:00Z", positionX: 100, positionY: -500, scale: 1000 };
beforeEach(() => { vi.resetAllMocks(); api.get.mockResolvedValue({ enabled: false, revision: 0, items: [] }); api.listen.mockResolvedValue(() => { }); api.save.mockResolvedValue(item); });
afterEach(cleanup);
it("键盘调整相对位置与大小后显式保存，取消不保存", async () => { const close = vi.fn(), saved = vi.fn(); render(<AccessoryPlacementEditor context={context} item={item} onClose={close} onSaved={saved}/>); fireEvent.keyDown(screen.getByRole("slider", { name: "饰品位置" }), { key: "ArrowRight" }); fireEvent.change(screen.getByLabelText("大小：100%"), { target: { value: "1500" } }); fireEvent.click(screen.getByRole("button", { name: "保存位置" })); await waitFor(() => expect(api.save).toHaveBeenCalledWith(context, "4", 125, -500, 1500)); expect(saved).toHaveBeenCalled(); fireEvent.click(screen.getByRole("button", { name: "取消" })); expect(close).toHaveBeenCalled(); expect(api.save).toHaveBeenCalledTimes(1); });
it("恢复默认必须点击保存才写入NULL坐标", async () => { render(<AccessoryPlacementEditor context={context} item={item} onClose={() => { }} onSaved={() => { }}/>); fireEvent.click(screen.getByRole("button", { name: "恢复默认位置与大小" })); expect(api.save).not.toHaveBeenCalled(); fireEvent.click(screen.getByRole("button", { name: "保存位置" })); await waitFor(() => expect(api.save).toHaveBeenCalledWith(context, "4", null, null, 1000)); });

it("默认位置首次向右调整不会同时跳动纵向位置", async () => {
  render(<AccessoryPlacementEditor context={context} item={{ ...item, positionX: null, positionY: null }} onClose={() => {}} onSaved={() => {}} />);
  fireEvent.keyDown(screen.getByRole("slider", { name: "饰品位置" }), { key: "ArrowRight" });
  fireEvent.click(screen.getByRole("button", { name: "保存位置" }));
  await waitFor(() => expect(api.save).toHaveBeenCalledWith(context, "4", 25, -929, 1000));
});

it("切换桌宠前后及同层排序立即保存，不覆盖位置",async()=>{api.layer.mockImplementation(async(_c,_id,plane)=>({...item,renderLayer:plane,layerOrder:0}));render(<AccessoryPlacementEditor context={context} item={item} onClose={()=>{}} onSaved={()=>{}}/>);fireEvent.change(screen.getByLabelText("挂件位于桌宠前后"),{target:{value:"back"}});await waitFor(()=>expect(api.layer).toHaveBeenCalledWith(context,"4","back","keep"));await waitFor(()=>expect((screen.getByRole("button",{name:"置于底层"}) as HTMLButtonElement).disabled).toBe(false));fireEvent.click(screen.getByRole("button",{name:"置于底层"}));await waitFor(()=>expect(api.layer).toHaveBeenCalledWith(context,"4","back","bottom"));expect(api.save).not.toHaveBeenCalled();});
