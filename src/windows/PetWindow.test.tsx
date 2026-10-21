import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PetWindow from "./PetWindow";
import { newestSnapshot, type PetSnapshot } from "./usePetWindow";
import type { MemoryGenerationView, PetActivity, PetStateNotice } from "../types";

const startDragging = vi.fn();
const desktopState = vi.hoisted(() => ({ fullscreenWander: false, dock: "none", revision: 0, busy: false, error: "", expression: undefined as string | undefined, toggle: vi.fn() }));
vi.mock("./usePetDesktop", () => ({ usePetDesktop: () => desktopState }));
vi.mock("./usePetAppearance", () => ({ usePetAppearance: () => ({ error: "", ready: true }) }));
vi.mock("../components/PetReminderNotifications", () => ({ default: ({ hidden, ready, unobscured, stateBubble }: {
  hidden: boolean; ready: boolean; unobscured: boolean; stateBubble?: ReactNode;
}) => !hidden && ready && unobscured ? stateBubble ?? null : null }));
const invoke = vi.fn();
const stop = vi.fn();
let receive: (event: { payload: PetSnapshot }) => void;
let receiveActivity: (event: { payload: PetActivity }) => void;
let receiveStateBubble: (event: { payload: PetStateNotice }) => void;
let receiveMemoryChanged: () => void;
let receiveDragFinished: () => void;
const initialActivity: PetActivity = {personaId:"1",sessionId:"1",contextRevision:0,revision:1,activity:"normal",expression:"(・ω・)",intimacy:998,sleepUntil:null,error:null,canInteract:true,canWander:true};
const initialMemory: MemoryGenerationView = {
  sessionId: "1", completedCount: 0, pendingCount: 0, activeRoundId: null, failedRoundId: null,
  error: null, autoRetryPending: false, longCompletedCount: 0, longPendingCount: 0,
  longActiveRoundId: null, longFailedRoundId: null, longError: null, longAutoRetryPending: false,
};
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async (name: string, callback: unknown) => {
  if (name === "pet-window-state") receive = callback as typeof receive;
  if (name === "pet-activity-changed") receiveActivity = callback as typeof receiveActivity;
  if (name === "pet-state-bubble") receiveStateBubble = callback as typeof receiveStateBubble;
  if (name === "memory-generation-changed") receiveMemoryChanged = callback as typeof receiveMemoryChanged;
  if (name === "pet-drag-finished") receiveDragFinished = callback as typeof receiveDragFinished;
  return stop;
} }));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging }),
}));

describe("PetWindow", () => {
  it("图片模式在原生接管鼠标后保留拎后颈图，松手恢复原图",async()=>{
    const fallback=invoke.getMockImplementation()!;
    const base={id:"1",personaId:"1",kind:"base",name:"默认图片",prompt:"",description:"",category:"normal",enabled:true,imageFile:"base.png",previewFile:null,revision:0,createdAt:"2026-10-20",updatedAt:"2026-10-20"};
    invoke.mockImplementation(async(command,args)=>{
      if(command==="get_pet_activity")return {...initialActivity,expression:"@pet-image:1"};
      if(command==="get_pet_lift_expression")return "@pet-image:2";
      if(command==="get_pet_creation")return {enabled:true,revision:1,items:[base,{...base,id:"2",kind:"expression",name:"拎后颈图片",category:"lifted",imageFile:"lifted.png"}]};
      if(command==="set_pet_artwork_layout")return {hidden:false,revision:1,chatOpen:false};
      if(command==="get_pet_artwork_image")return `data:image/png;base64,${args.id}`;
      return fallback(command,args);
    });
    await act(async()=>{render(<PetWindow/>);});await screen.findByRole("img",{name:"默认图片"});
    const pet=screen.getByRole("img",{name:"桌宠"});
    await act(async()=>{
      fireEvent.pointerDown(pet,{button:0,clientX:0,clientY:0,pointerId:1});
      fireEvent.pointerMove(pet,{clientX:20,clientY:0,pointerId:1});
    });
    await screen.findByRole("img",{name:"拎后颈图片"});
    fireEvent.lostPointerCapture(pet,{pointerId:1});fireEvent.pointerCancel(pet,{pointerId:1});fireEvent.blur(window);
    expect(screen.getByRole("img",{name:"拎后颈图片"})).toBeTruthy();
    await act(async()=>receiveDragFinished());await screen.findByRole("img",{name:"默认图片"});
  });
  it.each(["pointerCancel", "lostPointerCapture", "blur", "pointerUp"])("原生拖动中的 %s 不提前清除拎起表情，真正结束才恢复",async(eventName)=>{
    await act(async()=>{render(<PetWindow/>);});const pet=screen.getByRole("img",{name:"桌宠"});
    await act(async()=>{
      fireEvent.pointerDown(pet,{button:0,clientX:0,clientY:0,pointerId:1});
      fireEvent.pointerMove(pet,{clientX:20,clientY:0,pointerId:1});
    });
    expect(screen.getByText("(｡>﹏<｡)")).toBeTruthy();
    if(eventName==="blur")fireEvent.blur(window);
    else if(eventName==="pointerCancel")fireEvent.pointerCancel(pet,{pointerId:1});
    else if(eventName==="lostPointerCapture")fireEvent.lostPointerCapture(pet,{pointerId:1});
    else fireEvent.pointerUp(pet,{pointerId:1});
    expect(screen.getByTestId("pet-window").getAttribute("data-dragging")).toBe("true");
    expect(screen.getByText("(｡>﹏<｡)")).toBeTruthy();
    expect(invoke.mock.calls.some(([command])=>command==="interact_pet")).toBe(false);
    await act(async()=>receiveDragFinished());
    expect(screen.getByTestId("pet-window").getAttribute("data-dragging")).toBe("false");
    expect(screen.getByText("(・ω・)")).toBeTruthy();
  });
  it("原生拖动吞掉释放事件后仍可再次拖动", async () => {
    await act(async () => { render(<PetWindow />); });
    const pet = screen.getByRole("img");
    await act(async () => {
      fireEvent.pointerDown(pet, { button: 0, clientX: 0, clientY: 0, pointerId: 1 });
      fireEvent.pointerMove(pet, { clientX: 20, clientY: 0, pointerId: 1 });
    });
    expect(invoke.mock.calls.some(([command, args]) => command === "set_pet_dragging" && args.dragging === false)).toBe(false);
    await act(async () => { receiveDragFinished(); });
    await act(async () => {
      fireEvent.pointerDown(pet, { button: 0, clientX: 0, clientY: 0, pointerId: 2 });
      fireEvent.pointerMove(pet, { clientX: 20, clientY: 0, pointerId: 2 });
    });
    expect(startDragging).toHaveBeenCalledTimes(2);
  });

  it("首次连接失败显示错误并可重试恢复", async () => {
    let failed = false;
    invoke.mockImplementation(async (command) => {
      if (command === "get_pet_window_state") {
        if (!failed) { failed = true; throw new Error("连接中断"); }
        return { hidden: false, revision: 0 };
      }
      return undefined;
    });
    await act(async () => { render(<PetWindow />); });
    expect(screen.getByRole("alert").textContent).toContain("连接中断");
    fireEvent.click(screen.getByRole("button", { name: "重试连接" }));
    await screen.findByRole("button", { name: "隐藏宠窗" });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("建库失败显示可读错误且仍允许隐藏", async () => {
    invoke.mockImplementation(async (command, args) => {
      if (command === "get_configuration_status") throw { code: "STORAGE_ERROR", message: "请检查数据目录权限" };
      if (command === "get_pet_window_state") return { hidden: false, revision: 0 };
      if (command === "set_pet_hidden") return { hidden: args.hidden, revision: 1 };
    });
    await act(async () => { render(<PetWindow />); });
    expect(screen.getByRole("alert").textContent).toContain("请检查数据目录权限");
    fireEvent.click(screen.getByRole("button", { name: "隐藏宠窗" }));
    await screen.findByRole("button", { name: "显示宠窗" });
  });

  beforeEach(() => {
  Object.assign(desktopState, { fullscreenWander: false, dock: "none", revision: 0, busy: false, error: "", expression: undefined }); desktopState.toggle.mockReset();
    startDragging.mockReset().mockResolvedValue(undefined);
    invoke.mockReset().mockImplementation(async (command, args) => {
      if (command === "set_pet_chat_open") return {hidden:false,revision:3,chatOpen:args.open};
      if(command === "list_foods") return [{id:"token",name:"Token小点心",unlimited:true,price:0,quantity:0,revision:0,updatedAt:"2026-10-19"}];
      if(command === "get_food_preferences") return {defaultFoodId:"token",favoriteFoodId:null};
      if(command === "get_food_expression") return "(o_o)";
      if(command === "get_pet_lift_expression") return "(｡>﹏<｡)";
      if(command === "feed_food") return {actionId:args.actionId,kind:"feed",foodId:"token",quantity:1,personaId:"1",sessionId:"1",favoriteBonus:false,intimacyDelta:0,coinDelta:0,replayed:false};
      if (command === "get_pet_activity") return initialActivity;
      if (command === "get_memory_generation_status") return initialMemory;
      if (command === "interact_pet") return {...initialActivity,intimacy:999,revision:2,expression:"(^_^)"};
      if (command === "get_pet_window_state") return { hidden: false, revision: 0 };
      if (command === "set_pet_hidden") return { hidden: args.hidden, revision: args.hidden ? 1 : 2 };
      return undefined;
    });
    vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
    vi.stubGlobal("matchMedia", () => ({ addEventListener() {}, removeEventListener() {} }));
  });

  it("渲染颜文字形象", async () => {
    await act(async () => { render(<PetWindow />); });

    expect(screen.getByText("(・ω・)")).toBeTruthy();
  });

  it("单击调用当前会话互动，右键显示满值并仍可喂食", async () => {
    await act(async () => { render(<PetWindow />); });
    const pet=screen.getByRole("img");
    await act(async () => {
      fireEvent.pointerDown(pet,{button:0,clientX:10,clientY:10,pointerId:1});
      fireEvent.pointerUp(pet,{clientX:10,clientY:10,pointerId:1});
    });
    expect(invoke).toHaveBeenCalledWith("interact_pet",{input:{personaId:"1",sessionId:"1",contextRevision:0,action:"click"}});
    fireEvent.contextMenu(pet);
    expect(screen.getByText("亲密度 999 / 999 · 已达上限")).toBeTruthy();
    await act(async () => {fireEvent.click(screen.getByRole("button",{name:"投喂"}));});
    expect(invoke).not.toHaveBeenCalledWith("interact_pet",{input:{personaId:"1",sessionId:"1",contextRevision:0,action:"feed"}});
    await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"投喂 Token小点心"}));});
    expect(invoke).not.toHaveBeenCalledWith("feed_food",expect.anything());
    await act(async()=>{fireEvent.pointerDown(pet,{button:0,pointerId:2});});
    expect(invoke).toHaveBeenCalledWith("feed_food",expect.objectContaining({foodId:"token",personaId:"1",sessionId:"1",contextRevision:0}));
    await act(async()=>{receiveActivity({payload:{...initialActivity,revision:20,activity:"standby",canWander:false}});});
    await act(async()=>{fireEvent.pointerDown(pet,{button:0,pointerId:3});fireEvent.pointerUp(pet,{pointerId:3});});
    expect(invoke.mock.calls.filter(([command])=>command==="feed_food")).toHaveLength(1);
    expect(invoke.mock.calls.filter(([command])=>command==="interact_pet").slice(-1)[0]?.[1].input.action).toBe("click");

  });

  it("状态旧事件不覆盖新会话，出门禁用互动", async () => {
    await act(async () => {render(<PetWindow />);});
    await act(async () => {
      receiveActivity({payload:{...initialActivity,sessionId:"2",contextRevision:1,revision:4,activity:"away",expression:"喵喵出门啦",canInteract:false}});
      receiveActivity({payload:{...initialActivity,revision:2}});
    });
    expect(screen.getByText("喵喵出门啦")).toBeTruthy();
    fireEvent.contextMenu(screen.getByRole("img"));
    expect((screen.getByRole("button",{name:"投喂"}) as HTMLButtonElement).disabled).toBe(true);
  });

  it("左键单击颜文字时不开始拖动", async () => {
    await act(async () => { render(<PetWindow />); });
    const pet = screen.getByText("(・ω・)");

    await act(async () => {
      fireEvent.pointerDown(pet, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
      fireEvent.pointerUp(pet, { clientX: 10, clientY: 10, pointerId: 1 });
    });

    expect(startDragging).not.toHaveBeenCalled();
  });

  it("移动超过阈值后只开始拖动一次", async () => {
    await act(async () => { render(<PetWindow />); });
    const pet = screen.getByText("(・ω・)");

    fireEvent.pointerDown(pet, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerMove(pet, { clientX: 13, clientY: 13, pointerId: 1 });
    expect(startDragging).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.pointerMove(pet, { clientX: 16, clientY: 10, pointerId: 1 });
      fireEvent.pointerMove(pet, { clientX: 20, clientY: 10, pointerId: 1 });
    });

    expect(startDragging).toHaveBeenCalledTimes(1);
  });

  it("非颜文字区域不触发拖动", async () => {
    await act(async () => { render(<PetWindow />); });
    const windowCanvas = screen.getByTestId("pet-window");

    fireEvent.pointerDown(windowCanvas, {
      button: 0,
      clientX: 10,
      clientY: 10,
      pointerId: 1,
    });
    fireEvent.pointerMove(windowCanvas, { clientX: 30, clientY: 10, pointerId: 1 });

    expect(startDragging).not.toHaveBeenCalled();
  });

  it("隐藏后只保留恢复按钮，并能恢复颜文字", async () => {
    await act(async () => { render(<PetWindow />); });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("get_pet_window_state"));
    fireEvent.click(screen.getByRole("button", { name: "隐藏宠窗" }));
    await screen.findByRole("button", { name: "显示宠窗" });
    expect(screen.queryByRole("img")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "显示宠窗" }));
    await screen.findByRole("img");
    expect(startDragging).not.toHaveBeenCalled();
  });

  it("托盘状态事件解除隐藏，旧事件不能回退状态", async () => {
    await act(async () => { render(<PetWindow />); });
    await act(async () => { receive({ payload: { hidden: true, revision: 3 } }); });
    expect(screen.getByRole("button", { name: "显示宠窗" })).toBeTruthy();
    await act(async () => { receive({ payload: { hidden: false, revision: 4 } }); });
    await act(async () => { receive({ payload: { hidden: true, revision: 3 } }); });
    expect(screen.getByRole("img")).toBeTruthy();
    expect(newestSnapshot({ hidden: false, revision: 4 }, { hidden: true, revision: 1 })).toEqual({ hidden: false, revision: 4 });
  });

  it("订阅后延迟返回的旧快照不会覆盖新事件", async () => {
    let resolve!: (state: PetSnapshot) => void;
    invoke.mockImplementation((command) => command === "get_pet_window_state" ? new Promise((done) => { resolve = done; }) : Promise.resolve());
    await act(async () => { render(<PetWindow />); });
    await waitFor(() => expect(resolve).toBeDefined());
    await act(async () => { receive({ payload: { hidden: true, revision: 2 } }); resolve({ hidden: false, revision: 0 }); });
    expect(screen.getByRole("button", { name: "显示宠窗" })).toBeTruthy();
  });

  it.each(["pointerCancel", "lostPointerCapture"])("%s 结束拖动候选", async (eventName) => {
    await act(async () => { render(<PetWindow />); });
    const pet = screen.getByRole("img");
    fireEvent.pointerDown(pet, { button: 0, clientX: 0, clientY: 0, pointerId: 1 });
    if (eventName === "pointerCancel") fireEvent.pointerCancel(pet, { pointerId: 1 });
    else fireEvent.lostPointerCapture(pet, { pointerId: 1 });
    fireEvent.pointerMove(pet, { clientX: 20, clientY: 0, pointerId: 1 });
    expect(startDragging).not.toHaveBeenCalled();
  });

  it("忽略右键和其他指针的移动", async () => {
    await act(async () => { render(<PetWindow />); });
    const pet = screen.getByRole("img");
    fireEvent.pointerDown(pet, { button: 2, clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(pet, { clientX: 20, clientY: 0, pointerId: 1 });
    expect(startDragging).not.toHaveBeenCalled();
    fireEvent.pointerDown(pet, { button: 0, clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(pet, { clientX: 20, clientY: 0, pointerId: 2 });
    expect(startDragging).not.toHaveBeenCalled();
  });

  it("原生拖动失败会提供中文错误信息", async () => {
    startDragging.mockRejectedValue(new Error("拖动不可用"));
    await act(async () => { render(<PetWindow />); });
    const pet = screen.getByRole("img");
    fireEvent.pointerDown(pet, { button: 0, clientX: 0, clientY: 0, pointerId: 1 });
    fireEvent.pointerMove(pet, { clientX: 20, clientY: 0, pointerId: 1 });
    await waitFor(() => expect(screen.getByRole("button", { name: "隐藏宠窗" }).getAttribute("aria-description")).toContain("宠窗操作失败"));
  });

  it("隐藏调用失败后回读权威状态并保留恢复入口", async () => {
    invoke.mockImplementation(async (command) => {
      if (command === "get_pet_window_state") return { hidden: true, revision: 2 };
      if (command === "set_pet_hidden") throw new Error("调整尺寸失败");
    });
    await act(async () => { render(<PetWindow />); });
    fireEvent.click(screen.getByRole("button", { name: "显示宠窗" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "显示宠窗" }).getAttribute("aria-description")).toContain("调整尺寸失败"));
    expect(screen.queryByRole("img")).toBeNull();
    expect(invoke.mock.calls.filter(([command]) => command === "get_pet_window_state")).toHaveLength(2);
  });

  it("区域携带CSS坐标与DPI，布局变化刷新，相同布局不重复提交", async () => {
    let x = 10;
    const bounds = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("pet-tooltip")) return new DOMRect();
      return { x, y: 20, width: 30, height: 40, top: 20, left: x, right: x + 30, bottom: 60, toJSON: () => ({}) };
    });
    await act(async () => { render(<PetWindow />); });
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("update_pet_regions", {
      revision: 0, scale: window.devicePixelRatio,
      rects: Array.from({ length: 4 }, () => ({ x: 10, y: 20, width: 30, height: 40 })),
    }));
    x = 50;
    await act(async () => { screen.getByRole("img").style.transform = "translateX(40px)"; });
    await waitFor(() => expect(invoke.mock.calls.filter(([command]) => command === "update_pet_regions")).toHaveLength(2));
    fireEvent.resize(window);
    await act(async () => { await new Promise((resolve) => requestAnimationFrame(resolve)); });
    expect(invoke.mock.calls.filter(([command]) => command === "update_pet_regions")).toHaveLength(2);
    bounds.mockRestore();
  });
  it("状态气泡出现和收起时同步原生裁剪区域", async () => {
    const bubbleRect = { x: 20, y: 55, width: 180, height: 40 };
    const bounds = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("pet-tooltip")) return new DOMRect();
      const rect = this.classList.contains("pet-state-bubble") ? bubbleRect : { x: 10, y: 20, width: 30, height: 40 };
      return { ...rect, top: rect.y, left: rect.x, right: rect.x + rect.width, bottom: rect.y + rect.height, toJSON: () => ({}) };
    });
    const lastRegions = () => {
      const calls = invoke.mock.calls.filter(([command]) => command === "update_pet_regions");
      return calls[calls.length - 1]?.[1]?.rects;
    };
    try {
      await act(async () => { render(<PetWindow />); });
      await waitFor(() => expect(lastRegions()).toHaveLength(4));
      await act(async () => {
        receiveActivity({ payload: { ...initialActivity, revision: 2, activity: "bored" } });
        receiveStateBubble({ payload: { personaId: "1", sessionId: "1", contextRevision: 0, revision: 2, activity: "bored", text: "好无聊…" } });
      });
      expect(screen.getByRole("status").textContent).toContain("好无聊…");
      await waitFor(() => expect(lastRegions()).toContainEqual(bubbleRect));
      await act(async () => { receiveActivity({ payload: { ...initialActivity, revision: 3 } }); });
      expect(screen.queryByRole("status")).toBeNull();
      await waitFor(() => {
        expect(lastRegions()).toHaveLength(4);
        expect(lastRegions()).not.toContainEqual(bubbleRect);
      });
    } finally { bounds.mockRestore(); }
  });

  it.each([
    ["peek_left", "|・ω・)"], ["peek_right", "(・ω・|"],
    ["rest_sit", "(｡･ω･｡)＿"], ["rest_lie", "＿(｡-ω-｡)＿"],
    ["none", "(・ω・)"],
  ])("连接异常时%s保留本地停靠姿态与错误提示", async (dock, expected) => {
    Object.assign(desktopState, { fullscreenWander: true, dock, expression: dock === "none" ? undefined : expected });
    const fallback = invoke.getMockImplementation()!;
    invoke.mockImplementation(async (command, args) => command === "get_pet_activity"
      ? { ...initialActivity, activity: "dormant", error: "暂无聊天提供商", canWander: false }
      : fallback(command, args));
    await act(async () => { render(<PetWindow />); });
    expect(screen.getByText(expected)).toBeTruthy();
    expect(document.querySelector("[data-walking]")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "查看错误" }));
    expect(screen.getByRole("alert").textContent).toContain("暂无聊天提供商");
    expect(invoke.mock.calls.some(([command]) => command === "submit_chat" || command === "refresh_pet_provider")).toBe(false);
  });

  it("停靠不覆盖睡眠或出行表达", async () => {
    Object.assign(desktopState, { dock: "rest_lie", expression: "＿(｡-ω-｡)＿" });
    await act(async () => { render(<PetWindow />); });
    for (const [activity, expression] of [["sleeping", "(-_-) zZ"], ["away", "小猫出门啦"]] as const) {
      await act(async () => { receiveActivity({ payload: { ...initialActivity, revision: activity === "sleeping" ? 2 : 3, activity, expression, canWander: false } }); });
      expect(screen.getByText(expression)).toBeTruthy();
      expect(screen.queryByText("＿(｡-ω-｡)＿")).toBeNull();
    }
  });

  it("连接错误可刷新，失败保留错误，成功恢复且不重发聊天", async () => {
    const fallback = invoke.getMockImplementation()!;
    let recovered = false;
    let attempts = 0;
    invoke.mockImplementation(async (command, args) => {
      if (command === "set_pet_chat_open") return {hidden:false,revision:3,chatOpen:args.open};
      if (command === "get_pet_activity") return recovered ? { ...initialActivity, revision: 3 }
        : { ...initialActivity, activity: "dormant", error: "模型服务无法连接", canWander: false };
      if (command === "refresh_pet_provider") {
        attempts++;
        if (attempts === 1) throw { code: "PROVIDER_UNAVAILABLE", message: "连接依然失败" };
        recovered = true;
        return;
      }
      return fallback(command, args);
    });
    await act(async () => { render(<PetWindow />); });
    fireEvent.click(await screen.findByRole("button", { name: "查看错误" }));
    expect(screen.getByRole("alert").textContent).toContain("模型服务无法连接");
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("连接依然失败"));
    await waitFor(() => expect(screen.getByRole("button", { name: "刷新" }).hasAttribute("disabled")).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.queryByRole("button", { name: "查看错误" })).toBeNull();
    expect(invoke.mock.calls.filter(([command]) => command === "refresh_pet_provider")).toHaveLength(2);
    expect(invoke.mock.calls.some(([command]) => command === "submit_chat")).toBe(false);
  });

  it("刷新期间配置改变时回读当前状态，不显示旧连接结果", async () => {
    const fallback = invoke.getMockImplementation()!;
    let changed = false;
    invoke.mockImplementation(async (command, args) => {
      if (command === "set_pet_chat_open") return {hidden:false,revision:3,chatOpen:args.open};
      if (command === "get_pet_activity") return changed ? { ...initialActivity, revision: 4 }
        : { ...initialActivity, activity: "dormant", error: "旧提供商连接失败", canWander: false };
      if (command === "refresh_pet_provider") {
        changed = true;
        throw { code: "CONTEXT_CHANGED", message: "提供商配置或恢复状态已改变，旧连接测试结果已忽略" };
      }
      return fallback(command, args);
    });
    await act(async () => { render(<PetWindow />); });
    fireEvent.click(await screen.findByRole("button", { name: "查看错误" }));
    fireEvent.click(screen.getByRole("button", { name: "刷新" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.queryByRole("button", { name: "查看错误" })).toBeNull();
  });

  it("后台记忆失败只提示并打开记忆窗，成功事件移除错误徽标", async () => {
    const fallback = invoke.getMockImplementation()!;
    let failed = true;
    invoke.mockImplementation(async (command, args) => {
      if (command === "get_memory_generation_status") return failed ? { ...initialMemory,
        error: { code: "PROVIDER_UNAVAILABLE", message: "记忆模型暂不可用" } } : initialMemory;
      return fallback(command, args);
    });
    await act(async () => { render(<PetWindow />); });
    fireEvent.click(await screen.findByRole("button", { name: "查看错误" }));
    expect(screen.getByRole("alert").textContent).toContain("短期记忆生成失败：记忆模型暂不可用");
    expect(screen.queryByRole("button", { name: "刷新" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "查看记忆" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("focus_or_create_window", { page: "memory" }));
    expect(invoke.mock.calls.some(([command]) => command === "refresh_pet_provider")).toBe(false);
    await act(async () => { failed = false; receiveMemoryChanged(); });
    await waitFor(() => expect(screen.queryByRole("button", { name: "查看错误" })).toBeNull());
  });

  it("聊天与设置入口只导航，不触发桌宠互动或拖动", async () => {
    await act(async () => { render(<PetWindow />); });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "更多操作" }));
    });
    await act(async () => { fireEvent.click(screen.getByRole("menuitem", { name: "聊天详情" })); });
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "更多操作" })); });
    await act(async () => {
      fireEvent.click(screen.getByRole("menuitem", { name: "设置" }));
    });
    expect(invoke).toHaveBeenCalledWith("focus_or_create_window", { page: "chat" });
    expect(invoke).toHaveBeenCalledWith("focus_or_create_window", { page: "settings" });
    expect(invoke.mock.calls.some(([command]) => command === "interact_pet")).toBe(false);
    expect(startDragging).not.toHaveBeenCalled();
  });

it("出门后使用快捷邮箱写信并跳转统一邮箱，不触发聊天互动",async()=> {
  const previous=invoke.getMockImplementation()!;
  invoke.mockImplementation(async(command,args)=>command==="get_pet_mail_status"?{travelId:"3",unread:1}:previous(command,args));
  await act(async()=>{render(<PetWindow/>);});
  await act(async()=>{receiveActivity({payload:{...initialActivity,revision:4,activity:"away",expression:"小喵出门啦",canInteract:false,canWander:false}});});
  expect(await screen.findByText("有新信件！")).toBeTruthy();
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:/^邮箱$/}));});
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:/^写信$/}));});
  fireEvent.change(screen.getByRole("textbox",{name:"信件正文"}),{target:{value:"旅途顺利呀"}});
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:/^寄出$/}));});
  expect(invoke).toHaveBeenCalledWith("send_letter",{personaId:"1",sessionId:"1",contextRevision:0,input:{travelId:"3",content:"旅途顺利呀",replyToId:null}});
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:/^查看邮箱$/}));});
  expect(invoke).toHaveBeenCalledWith("focus_or_create_window",{page:"mailbox"});expect(invoke.mock.calls.some(([command])=>command==="interact_pet")).toBe(false);expect(startDragging).not.toHaveBeenCalled();
});

  it("图标统一使用自绘悬浮提示，聊天仍可展开快捷输入",async()=>{
    await act(async()=>{render(<PetWindow/>);});
    expect(screen.queryByRole("menu")).toBeNull();
    const toggle=screen.getByRole("button",{name:"聊天"});
    for (const name of ["聊天", "更多操作", "隐藏宠窗"]) {
      const button=screen.getByRole("button",{name});
      expect(button.hasAttribute("title")).toBe(false);
      const tooltip=button.querySelector(".pet-tooltip");
      expect(tooltip?.textContent).toBe(name);
      expect(tooltip?.getAttribute("aria-hidden")).toBe("true");
      fireEvent.pointerEnter(button);
      expect(button.getAttribute("data-tooltip-open")).toBe("true");
      fireEvent.pointerLeave(button);
      expect(button.getAttribute("data-tooltip-open")).toBe("false");
      expect(Array.from(button.childNodes).filter(node=>node.nodeType===Node.TEXT_NODE).map(node=>node.textContent).join("")).toBe(name==="更多操作"?"...":"");
    }
    await act(async()=>{fireEvent.click(toggle);});
    expect(invoke).toHaveBeenCalledWith("set_pet_chat_open",{open:true});
    expect(screen.getByRole("region",{name:"宠窗快捷聊天"})).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith("focus_or_create_window",{page:"chat"});
  });

it("更多菜单用滑动开关控制全屏游走，停靠布局保留独立标记", async () => {
  desktopState.dock = "peek_left"; desktopState.fullscreenWander = true;
  await act(async () => { render(<PetWindow />); });
  expect(screen.getByTestId("pet-window").dataset.dock).toBe("peek_left");
  fireEvent.click(screen.getByRole("button", { name: "更多操作" }));
  const control = screen.getByRole("switch", { name: "全屏游走" });
  expect(control.getAttribute("aria-checked")).toBe("true");
  fireEvent.click(control); expect(desktopState.toggle).toHaveBeenCalledTimes(1);
});

it("拿着食物时右键只取消，不投喂也不打开原互动菜单", async () => {
  await act(async () => { render(<PetWindow />); });
  fireEvent.contextMenu(screen.getByRole("img", { name: "桌宠" }));
  fireEvent.click(screen.getByRole("button", { name: "投喂" }));
  fireEvent.click(await screen.findByRole("button", { name: "投喂 Token小点心" }));
  await screen.findByText(/拿着 Token小点心/);
  fireEvent.contextMenu(screen.getByRole("img", { name: "桌宠" }));
  await waitFor(() => expect(screen.queryByText(/拿着 Token小点心/)).toBeNull());
  expect(screen.queryByRole("button", { name: "投喂" })).toBeNull();
  expect(invoke.mock.calls.some(([command]) => command === "feed_food")).toBe(false);
});

it("饥饿显示独立饿提示，恢复日常后移除", async () => {
  await act(async () => { render(<PetWindow />); });
  await act(async () => { receiveActivity({ payload: { ...initialActivity, revision: 20, activity: "hungry", expression: "(>_<)" } }); });
  expect(screen.getByText("饿～")).toBeTruthy();
  await act(async () => { receiveActivity({ payload: { ...initialActivity, revision: 21 } }); });
  expect(screen.queryByText("饿～")).toBeNull();
});

});
