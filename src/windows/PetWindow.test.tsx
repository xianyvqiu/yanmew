import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PetWindow from "./PetWindow";
import type {Toss,Chase} from "./usePetDesktop";
import * as petWander from "./usePetWander";
import { newestSnapshot, type PetSnapshot } from "./usePetWindow";
import type { MemoryGenerationView, PetActivity, PetStateNotice } from "../types";

const startDragging = vi.fn();
const desktopState = vi.hoisted(() => ({ chase:undefined as Chase|undefined, toss:undefined as Toss|undefined, fullscreenWander: false, dock: "none", revision: 0, busy: false, error: "", expression: undefined as string | undefined, toggle: vi.fn() }));
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
  it("追到后显示等摸摸，按下不会先取消等待，松手仍走正常点击接口",async()=>{
    Object.assign(desktopState,{fullscreenWander:true,chase:{phase:"waiting",personaId:"1",sessionId:"1",contextRevision:0},expression:"(｡･ω･｡)ﾉ"});
    const wander=vi.spyOn(petWander,"usePetWander");
    await act(async()=>{render(<PetWindow/>);});
    expect(screen.getByText("等摸摸").parentElement).toBe(screen.getByTestId("pet-window"));expect(screen.getByText("等摸摸").hasAttribute("data-pet-interactive")).toBe(true);expect(screen.getByText("(｡･ω･｡)ﾉ")).toBeTruthy();
    const face=screen.getByRole("img",{name:"桌宠"});
    fireEvent.pointerDown(face,{button:0,pointerId:9,clientX:140,clientY:170});
    expect(screen.getByText("等摸摸")).toBeTruthy();expect(wander.mock.calls[wander.mock.calls.length-1]?.[3]).toBe(true);
    fireEvent.pointerUp(face,{button:0,pointerId:9,clientX:140,clientY:170});
    await waitFor(()=>expect(invoke).toHaveBeenCalledWith("interact_pet",{input:{personaId:"1",sessionId:"1",contextRevision:0,action:"click"}}));
    await act(async()=>receiveActivity({payload:{...initialActivity,activity:"away",expression:"出门啦",canInteract:false,canWander:false,revision:4}}));
    expect(screen.queryByText("等摸摸")).toBeNull();expect(screen.getByText("出门啦")).toBeTruthy();wander.mockRestore();
  });
  it("失落阶段使用对应表情而不显示等待文字，旧上下文不会覆盖当前人设",async()=>{
    Object.assign(desktopState,{fullscreenWander:true,chase:{phase:"sad",personaId:"1",sessionId:"1",contextRevision:0},expression:"(｡•́︿•̀｡)"});
    const r=await act(async()=>render(<PetWindow/>));expect(screen.getByText("(｡•́︿•̀｡)")).toBeTruthy();expect(screen.queryByText("等摸摸")).toBeNull();
    desktopState.chase={...desktopState.chase!,contextRevision:99};r.rerender(<PetWindow/>);expect(screen.queryByText("(｡•́︿•̀｡)")).toBeNull();
  });
  it("落地动作按透明边界贴地，不因坐姿变矮悬空",async()=>{
    desktopState.toss={phase:"landed",personaId:"1",sessionId:"1",contextRevision:0,body:{x:62,y:102,width:196,height:196},controlsBelow:false};desktopState.expression="@pet-image:4";
    const fallback=invoke.getMockImplementation()!;
    const base={id:"1",personaId:"1",kind:"base",name:"默认图片",prompt:"",description:"",category:"normal",enabled:true,imageFile:"base.png",previewFile:null,revision:0,createdAt:"a",updatedAt:"a"};
    invoke.mockImplementation(async(command,args)=>{
      if(command==="get_pet_creation")return{enabled:true,revision:1,items:[base,{...base,id:"4",kind:"expression",category:"landing",name:"落地动作",imageFile:"landing.png"}]};
      if(command==="set_pet_artwork_layout")return{hidden:false,revision:1,chatOpen:false};
      if(command==="get_pet_artwork_image")return `data:image/png;base64,${args.id}`;
      return fallback(command,args);
    });
    const pixels=new Uint8ClampedArray(8*8*4);for(let y=2;y<6;y++)pixels[(y*8+4)*4+3]=255;
    const spy=vi.spyOn(HTMLCanvasElement.prototype,"getContext").mockReturnValue({drawImage(){},getImageData:()=>({data:pixels})} as unknown as CanvasRenderingContext2D);
    try{await act(async()=>{render(<PetWindow/>);});const image=await screen.findByRole("img",{name:"落地动作"});
      Object.defineProperties(image,{naturalWidth:{value:8},naturalHeight:{value:8},complete:{value:true}});fireEvent.load(image);
      expect(image.parentElement!.style.getPropertyValue("--ym-art-visible-ratio")).toBe("0.5");
    }finally{spy.mockRestore();}
  });
  it("飞行中按下立绘立即接住并显示拎起，不触发普通点击",async()=>{
    desktopState.toss={phase:"down",personaId:"1",sessionId:"1",contextRevision:0,body:{x:62,y:102,width:196,height:196},controlsBelow:false};
    desktopState.expression="(>﹏<)ﾉ";
    await act(async()=>{render(<PetWindow/>);});
    fireEvent.pointerDown(screen.getByRole("img",{name:"桌宠"}),{button:0,pointerId:1,clientX:150,clientY:180});
    await waitFor(()=>expect(startDragging).toHaveBeenCalledTimes(1));
    expect(screen.getByText("(｡>﹏<｡)")).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith("interact_pet",expect.anything());
  });
  it("出行时即使收到旧飞行快照也不显示抛接或按下接住",async()=>{
    desktopState.toss={phase:"down",personaId:"1",sessionId:"1",contextRevision:0,body:{x:62,y:102,width:196,height:196},controlsBelow:true};desktopState.expression="(>﹏<)ﾉ";
    const fallback=invoke.getMockImplementation()!;
    invoke.mockImplementation(async(command,args)=>command==="get_pet_activity"?{...initialActivity,activity:"away",expression:"出门啦",canInteract:false,canWander:false}:fallback(command,args));
    await act(async()=>{render(<PetWindow/>);});expect(screen.getByText("出门啦")).toBeTruthy();
    expect(screen.getByTestId("pet-window").hasAttribute("data-toss")).toBe(false);
    fireEvent.pointerDown(screen.getByRole("img",{name:"桌宠"}),{button:0,pointerId:1,clientX:150,clientY:180});
    expect(startDragging).not.toHaveBeenCalled();
  });
  it("报错表情使用本地设置，收起详情仍保留，解除后恢复停靠",async()=>{
    Object.assign(desktopState,{dock:"peek_left",expression:"|・ω・)"});
    const fallback=invoke.getMockImplementation()!;
    invoke.mockImplementation(async(command,args)=>command==="get_pet_error_expression"?"(=_=;)":fallback(command,args));
    await act(async()=>{render(<PetWindow/>);});expect(screen.getByText("|・ω・)")).toBeTruthy();
    await act(async()=>{receiveActivity({payload:{...initialActivity,revision:2,activity:"dormant",error:"连接失败",canWander:false}});});
    expect(screen.getByText("(=_=;)")).toBeTruthy();
    fireEvent.click(screen.getByRole("button",{name:"查看错误"}));expect(screen.getByRole("alert")).toBeTruthy();
    fireEvent.click(screen.getByRole("button",{name:"查看错误"}));expect(screen.queryByRole("alert")).toBeNull();expect(screen.getByText("(=_=;)")).toBeTruthy();
    await act(async()=>{receiveActivity({payload:{...initialActivity,revision:3}});});expect(screen.getByText("|・ω・)")).toBeTruthy();
    expect(invoke.mock.calls.some(([command])=>command==="submit_chat"||command==="interact_pet")).toBe(false);
  });
  it("图片模式报错显示超载图，问题解除后恢复当前图",async()=>{
    const fallback=invoke.getMockImplementation()!;
    const base={id:"1",personaId:"1",kind:"base",name:"默认图片",prompt:"",description:"",category:"normal",enabled:true,imageFile:"base.png",previewFile:null,revision:0,createdAt:"a",updatedAt:"a"};
    invoke.mockImplementation(async(command,args)=>{
      if(command==="get_pet_activity")return {...initialActivity,expression:"@pet-image:1"};
      if(command==="get_pet_error_expression")return "@pet-image:3";
      if(command==="get_pet_creation")return {enabled:true,revision:1,items:[base,{...base,id:"3",kind:"expression",name:"报错超载",category:"error",imageFile:"error.png"}]};
      if(command==="set_pet_artwork_layout")return {hidden:false,revision:1,chatOpen:false};
      if(command==="get_pet_artwork_image")return `data:image/png;base64,${args.id}`;
      return fallback(command,args);
    });
    await act(async()=>{render(<PetWindow/>);});await screen.findByRole("img",{name:"默认图片"});
    await act(async()=>{receiveActivity({payload:{...initialActivity,revision:2,expression:"@pet-image:1",error:"模型报错"}});});await screen.findByRole("img",{name:"报错超载"});
    await act(async()=>{receiveActivity({payload:{...initialActivity,revision:3,expression:"@pet-image:1"}});});await screen.findByRole("img",{name:"默认图片"});
  });

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

  it("警示可收起为叹号并重新打开，收起不清除故障状态",async()=>{
    desktopState.error="拖动暂时失败";await act(async()=>{render(<PetWindow/>);});
    expect(screen.getByRole("alert").textContent).toContain("拖动暂时失败");
    fireEvent.click(screen.getByRole("button",{name:"收起警示"}));expect(screen.queryByRole("alert")).toBeNull();
    const badge=screen.getByRole("button",{name:"查看错误"});expect(badge.getAttribute("aria-expanded")).toBe("false");expect(document.querySelector(".pet-window")?.getAttribute("data-error")).toBe("true");
    fireEvent.click(badge);expect(screen.getByRole("alert").textContent).toContain("拖动暂时失败");expect(badge.getAttribute("aria-expanded")).toBe("true");
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
  Object.assign(desktopState, { chase:undefined, toss:undefined, fullscreenWander: false, dock: "none", revision: 0, busy: false, error: "", expression: undefined }); desktopState.toggle.mockReset();
    startDragging.mockReset().mockResolvedValue(undefined);
    invoke.mockReset().mockImplementation(async (command, args) => {
      if (command === "set_pet_chat_open") return {hidden:false,revision:3,chatOpen:args.open};
      if(command === "list_foods") return [{id:"token",name:"Token小点心",unlimited:true,price:0,quantity:0,revision:0,updatedAt:"2026-10-19"}];
      if(command === "get_food_preferences") return {defaultFoodId:"token",favoriteFoodId:null};
      if(command === "get_food_expression") return "(o_o)";
      if(command === "get_pet_error_expression") return "(×_×)";
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

  it.each(["standby", "bored", "hungry"] as const)("顶部放置保留%s活动表情，底部仍用停靠动作", async activity => {
    Object.assign(desktopState,{dock:"top",expression:"顶部默认图"});
    const result=await act(async()=>render(<PetWindow/>));
    await act(async()=>receiveActivity({payload:{...initialActivity,activity,expression:"当前活动表情",canWander:false,revision:20}}));
    expect(screen.getByText("当前活动表情")).toBeTruthy();
    expect(screen.queryByText("顶部默认图")).toBeNull();
    Object.assign(desktopState,{dock:"rest_sit",expression:"底部坐下"});
    result.rerender(<PetWindow/>);
    expect(screen.getByText("底部坐下")).toBeTruthy();
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

  it("出行左键切换时长且不报错，右键菜单与时长互斥",async()=>{
    const fallback=invoke.getMockImplementation()!;
    invoke.mockImplementation(async(command,args)=>command==="get_current_travel"?{id:"3",status:"away",startedAt:new Date(Date.now()-3600000).toISOString()}:fallback(command,args));
    await act(async()=>{render(<PetWindow/>);});
    await act(async()=>receiveActivity({payload:{...initialActivity,revision:4,activity:"away",expression:"喵喵出门啦",canInteract:false}}));
    expect(screen.queryByLabelText("已经出门时间")).toBeNull();const pet=screen.getByRole("img",{name:"桌宠"});
    await act(async()=>{fireEvent.pointerDown(pet,{button:0,pointerId:1,clientX:10,clientY:10});fireEvent.pointerUp(pet,{pointerId:1,clientX:10,clientY:10});});
    expect(await screen.findByLabelText("已经出门时间")).toBeTruthy();expect(screen.getByText("01:00")).toBeTruthy();
    expect(screen.queryByRole("button",{name:"查看错误"})).toBeNull();expect(invoke.mock.calls.some(([command])=>command==="interact_pet")).toBe(false);
    fireEvent.contextMenu(pet);expect(screen.queryByLabelText("已经出门时间")).toBeNull();expect(screen.getByRole("button",{name:"投喂"})).toHaveProperty("disabled",true);
    await act(async()=>{fireEvent.pointerDown(pet,{button:0,pointerId:2,clientX:10,clientY:10});fireEvent.pointerUp(pet,{pointerId:2,clientX:10,clientY:10});});
    expect(await screen.findByLabelText("已经出门时间")).toBeTruthy();expect(screen.queryByRole("button",{name:"投喂"})).toBeNull();
  });
  it("同会话正常返回展示我回来了，不等待回来信",async()=>{
    const fallback=invoke.getMockImplementation()!;invoke.mockImplementation(async(command,args)=>command==="list_travels"?[{id:"3",status:"returned"}]:fallback(command,args));
    await act(async()=>{render(<PetWindow/>);});
    await act(async()=>receiveActivity({payload:{...initialActivity,revision:3,activity:"away",canInteract:false}}));
    await act(async()=>receiveActivity({payload:{...initialActivity,revision:4}}));
    expect(await screen.findByRole("status",{name:"返回提示"})).toHaveProperty("textContent","我回来了");
  });
  it("点击撞上出发时后端PET_AWAY不产生感叹号警示",async()=>{
    const fallback=invoke.getMockImplementation()!;
    invoke.mockImplementation(async(command,args)=>{
      if(command==="interact_pet")throw {code:"PET_AWAY",message:"它出门啦，回来以后再互动吧"};
      return fallback(command,args);
    });
    await act(async()=>{render(<PetWindow/>);});const pet=screen.getByRole("img",{name:"桌宠"});
    await act(async()=>{fireEvent.pointerDown(pet,{button:0,pointerId:1,clientX:10,clientY:10});fireEvent.pointerUp(pet,{pointerId:1,clientX:10,clientY:10});});
    expect(screen.queryByRole("button",{name:"查看错误"})).toBeNull();expect(screen.queryByRole("alert")).toBeNull();
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

  it("出门后拖动只移动占位区域，不显示拎起表情", async () => {
    await act(async () => { render(<PetWindow />); });
    await act(async () => { receiveActivity({payload:{...initialActivity,revision:4,activity:"away",expression:"喵喵出门啦",canInteract:false}}); });
    const pet=screen.getByRole("img");
    await act(async () => {
      fireEvent.pointerDown(pet,{button:0,clientX:10,clientY:10,pointerId:1});
      fireEvent.pointerMove(pet,{clientX:30,clientY:10,pointerId:1});
    });
    expect(startDragging).toHaveBeenCalledTimes(1);
    expect(screen.getByText("喵喵出门啦")).toBeTruthy();
    expect(screen.queryByText("(｡>﹏<｡)")).toBeNull();
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
  ])("连接异常时%s显示报错表情但保留停靠位置", async (dock, expected) => {
    Object.assign(desktopState, { fullscreenWander: true, dock, expression: dock === "none" ? undefined : expected });
    const fallback = invoke.getMockImplementation()!;
    invoke.mockImplementation(async (command, args) => command === "get_pet_activity"
      ? { ...initialActivity, activity: "dormant", error: "暂无聊天提供商", canWander: false }
      : fallback(command, args));
    await act(async () => { render(<PetWindow />); });
    expect(screen.getByText("(×_×)")).toBeTruthy();
    expect(screen.getByTestId("pet-window").getAttribute("data-dock")).toBe(dock);
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

  it.each(["error", "longError"] as const)("%s 错误徽标未展开或收起时仍暂停游走，解除后恢复", async (field) => {
    desktopState.fullscreenWander = true;
    const movement = vi.spyOn(petWander, "usePetWander").mockReturnValue({});
    const fallback = invoke.getMockImplementation()!;
    let failed = false;
    invoke.mockImplementation(async (command, args) => command === "get_memory_generation_status"
      ? { ...initialMemory, [field]: failed ? { code: "PROVIDER_UNAVAILABLE", message: "记忆模型暂不可用" } : null }
      : fallback(command, args));
    const enabled = () => movement.mock.calls[movement.mock.calls.length - 1]?.[3];
    try {
      await act(async () => { render(<PetWindow />); });
      await waitFor(() => expect(enabled()).toBe(true));
      await act(async () => { failed = true; receiveMemoryChanged(); });
      await screen.findByRole("button", { name: "查看错误" });
      expect(screen.queryByRole("alert")).toBeNull();
      await waitFor(() => expect(enabled()).toBe(false));
      expect(screen.getByTestId("pet-window").dataset.error).toBe("true");
      fireEvent.click(screen.getByRole("button", { name: "查看错误" }));
      expect(screen.getByRole("alert")).toBeTruthy();
      fireEvent.click(screen.getByRole("button", { name: "查看错误" }));
      expect(screen.queryByRole("alert")).toBeNull();
      expect(enabled()).toBe(false);
      await act(async () => { failed = false; receiveMemoryChanged(); });
      await waitFor(() => expect(enabled()).toBe(true));
      expect(screen.getByTestId("pet-window").dataset.error).toBe("false");
    } finally { movement.mockRestore(); }
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
      expect(Array.from(button.childNodes).filter(node=>node.nodeType===Node.TEXT_NODE).map(node=>node.textContent).join("")).toBe("");
      expect(button.querySelector("svg")).toBeTruthy();
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
