import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import SouvenirPanel from "./SouvenirPanel";
import type { Souvenir } from "../types";
const api = vi.hoisted(() => ({ image:vi.fn(),imageSettings:vi.fn(),setImageSettings:vi.fn(),listenImageSettings:vi.fn(), browse: vi.fn(), browserListen: vi.fn(), list: vi.fn(), get: vi.fn(), favorite: vi.fn(), delete: vi.fn(), status: vi.fn(), retry: vi.fn(), listen: vi.fn() }));
vi.mock("../api", () => ({ souvenirsApi: api, cardBrowserApi: {browse: api.browse, listen: api.browserListen}, travelApi: { listen: api.listen }, errorMessage: (cause: Error) => cause.message }));
const context = { personaId: "1", sessionId: "2", contextRevision: 1 };
const gift: Souvenir = { id: "3", sessionId: "2", travelId: "4", content: "湖边明信片", note: "挑了最漂亮的一张给你。", isFavorite: false, createdAt: "2026-10-13T01:00:00Z" };
beforeEach(() => {
  vi.resetAllMocks();api.imageSettings.mockResolvedValue(true);api.setImageSettings.mockImplementation(async value=>value);api.listenImageSettings.mockResolvedValue(()=>{});api.image.mockResolvedValue({dataUrl:"data:image/png;base64,cG5n",notice:null}); api.list.mockResolvedValue([gift]); api.get.mockResolvedValue(gift);
  api.status.mockResolvedValue({ active: null, pendingCount: 0, failures: [], notice: null });
  api.listen.mockResolvedValue(() => {}); api.favorite.mockResolvedValue(gift); api.delete.mockResolvedValue(undefined); api.retry.mockResolvedValue(undefined);
  api.browserListen.mockResolvedValue(() => {});
  api.browse.mockImplementation(async (_kind, scope, search, order, page) => {
    const rows = (await api.list(scope)).filter((row: Record<string, unknown>) => JSON.stringify(row).includes(search));
    if (order === "oldest") rows.reverse();
    const pages = Math.max(1, Math.ceil(rows.length / 12));
    const current = Math.min(page, pages);
    return {items: rows.slice((current - 1) * 12, current * 12), total: rows.length, page: current, pages};
  });
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);
it("多选出行在请求中传递并显示卡片所属出行",async()=>{
 const trips=[{id:"4",destination:"湖边",startedAt:"2026-10-13T00:00:00Z"},{id:"8",destination:"山间",startedAt:"2026-10-12T00:00:00Z"}];api.browse.mockResolvedValue({items:[{...gift,travel:trips[0]}],travels:trips,total:1,page:1,pages:1});render(<SouvenirPanel context={context}/>);await screen.findByRole("button",{name:/湖边明信片/});expect(screen.getByText(/出发 · 湖边/)).toBeTruthy();fireEvent.click(screen.getByText("按出行筛选"));fireEvent.click(screen.getByRole("checkbox",{name:/湖边/}));await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("souvenirs",context,"","newest",1,["4"]));fireEvent.click(screen.getByRole("checkbox",{name:/山间/}));await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("souvenirs",context,"","newest",1,["4","8"]));fireEvent.click(screen.getByRole("button",{name:"清除筛选"}));await waitFor(()=>expect(api.browse).toHaveBeenLastCalledWith("souvenirs",context,"","newest",1,[]));
});
it("伴手礼有图片时展示，关闭生图只影响未来礼物",async()=>{
 api.list.mockResolvedValue([{...gift,imageFile:"assets/accessories/accessory-1-1-1.png"}]);render(<SouvenirPanel context={context}/>);await screen.findByAltText("伴手礼图片");const toggle=screen.getByRole("switch",{name:"为新伴手礼生成图片"});await waitFor(()=>expect(toggle).toHaveProperty("checked",true));fireEvent.click(toggle);await waitFor(()=>expect(api.setImageSettings).toHaveBeenCalledWith(false));expect(screen.getByAltText("伴手礼图片")).toBeTruthy();
});
it("查看备注和收藏绑定当前会话，不修改钱包", async () => {
  render(<SouvenirPanel context={context} />);
  fireEvent.click(await screen.findByRole("button", { name: /湖边明信片/ }));
  await screen.findByText("挑了最漂亮的一张给你。");
  api.list.mockResolvedValue([{ ...gift, isFavorite: true }]); api.get.mockResolvedValue({ ...gift, isFavorite: true });
  fireEvent.click(screen.getByRole("button", { name: "收藏" }));
  await screen.findByRole("button", { name: "取消收藏" });
  expect(api.favorite).toHaveBeenCalledWith(context, "3", true);
});
it("删除先确认，取消不提交，成功后关闭详情并刷新", async () => {
  render(<SouvenirPanel context={context} />);
  fireEvent.click(await screen.findByRole("button", { name: /湖边明信片/ }));
  fireEvent.click(await screen.findByRole("button", { name: "删除伴手礼" }));
  expect(within(screen.getByRole("dialog")).getByText(/不会再次发放/)).toBeTruthy();
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  expect(api.delete).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "删除伴手礼" }));
  api.list.mockResolvedValue([]);
  fireEvent.click(screen.getByRole("button", { name: "确认删除伴手礼" }));
  await screen.findByText("还没有收到伴手礼。");
  expect(api.delete).toHaveBeenCalledWith(context, "3");
  expect(screen.queryByText("挑了最漂亮的一张给你。")).toBeNull();
});
it("生成失败和字符兜底说明可见，重试仅提交当前会话", async () => {
  api.status.mockResolvedValue({ active: null, pendingCount: 1, failures: [{ travelId: "4", error: { message: "生成失败" }, retryAfterSeconds: 30 }], notice: "恢复饰品使用原图标，不再次付费。" });
  render(<SouvenirPanel context={context} />);
  await screen.findByText("恢复饰品使用原图标，不再次付费。");
  fireEvent.click(screen.getByRole("button", { name: "重试待生成礼物" }));
  await waitFor(() => expect(api.retry).toHaveBeenCalledWith(context));
});
it("切换会话卸载旧确认和迟到详情，不允许删除新会话的旧ID", async () => {
  let resolve!: (value: Souvenir) => void;
  api.get.mockReturnValue(new Promise(done => { resolve = done; }));
  const view = render(<SouvenirPanel context={context} />);
  fireEvent.click(await screen.findByRole("button", { name: /湖边明信片/ }));
  api.list.mockResolvedValue([]);
  view.rerender(<SouvenirPanel context={{ ...context, sessionId: "8", contextRevision: 2 }} />);
  await act(async () => { resolve(gift); });
  await screen.findByText("还没有收到伴手礼。");
  expect(screen.queryByText("挑了最漂亮的一张给你。")).toBeNull();
  expect(screen.queryByRole("button", { name: "删除伴手礼" })).toBeNull();
});
it("翻页和聚焦保持页码，搜索覆盖首屏之外的数据", async () => {
  const records = Array.from({length: 25}, (_, i) => ({...gift,id: String(100-i),content: `礼物编号${100-i}`}));
  api.list.mockResolvedValue(records);
  render(<SouvenirPanel context={context}/>);
  await screen.findByRole("button", {name:/礼物编号100/});
  fireEvent.click(screen.getByRole("button", {name:"下一页"}));
  await screen.findByRole("button", {name:/礼物编号88/});
  await act(async () => fireEvent.focus(window));
  expect(await screen.findByRole("button", {name:/礼物编号88/})).toBeTruthy();
  expect(api.browse).toHaveBeenLastCalledWith("souvenirs",context,"","newest",2,[]);
  fireEvent.change(screen.getByRole("searchbox"), {target:{value:"礼物编号76"}});
  expect(await screen.findByRole("button", {name:/礼物编号76/})).toBeTruthy();
  expect(api.browse).toHaveBeenLastCalledWith("souvenirs",context,"礼物编号76","newest",1,[]);
});
it("通知后保留当前筛选并更新收藏详情", async () => {
  let notify!: () => void;
  api.browserListen.mockImplementation(async (_kind, cb) => {notify=cb;return () => {};});
  render(<SouvenirPanel context={context}/>);
  await screen.findByRole("button",{name:/湖边明信片/});
  fireEvent.change(screen.getByRole("searchbox"),{target:{value:"湖边"}});
  fireEvent.click(await screen.findByRole("button",{name:/湖边明信片/}));
  await screen.findByRole("button",{name:"收藏"});
  api.get.mockResolvedValue({...gift,isFavorite:true});
  await act(async () => {notify();fireEvent.focus(window);});
  await screen.findByRole("button",{name:"取消收藏"});
  expect(api.browse).toHaveBeenLastCalledWith("souvenirs",context,"湖边","newest",1,[]);
});

it("新礼物显示独立标题，旧礼物按目的地显示标题",async()=>{
 api.list.mockResolvedValue([{...gift,title:"蓝湾海螺",travel:{id:"4",destination:"蓝鲸湾",startedAt:gift.createdAt}}]);
 const view=render(<SouvenirPanel context={context}/>);await screen.findByText("蓝湾海螺");
 view.unmount();api.list.mockResolvedValue([{...gift,travel:{id:"4",destination:"湖边",startedAt:gift.createdAt}}]);render(<SouvenirPanel context={context}/>);await screen.findByText("湖边·伴手礼");
});
