import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PetWindow from "./PetWindow";
import { newestSnapshot, type PetSnapshot } from "./usePetWindow";
import type { PetActivity } from "../types";

const startDragging = vi.fn();
vi.mock("./usePetAppearance", () => ({ usePetAppearance: () => ({ error: "", ready: true }) }));
const invoke = vi.fn();
const stop = vi.fn();
let receive: (event: { payload: PetSnapshot }) => void;
let receiveActivity: (event: { payload: PetActivity }) => void;
const initialActivity: PetActivity = {personaId:"1",sessionId:"1",contextRevision:0,revision:1,activity:"normal",expression:"(・ω・)",intimacy:998,sleepUntil:null,error:null,canInteract:true,canWander:true};
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async (name: string, callback: unknown) => {
  if (name === "pet-window-state") receive = callback as typeof receive;
  if (name === "pet-activity-changed") receiveActivity = callback as typeof receiveActivity;
  return stop;
} }));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging }),
}));

describe("PetWindow", () => {
  it("原生拖动吞掉释放事件后仍可再次拖动", async () => {
    await act(async () => { render(<PetWindow />); });
    const pet = screen.getByRole("img");
    await act(async () => {
      fireEvent.pointerDown(pet, { button: 0, clientX: 0, clientY: 0, pointerId: 1 });
      fireEvent.pointerMove(pet, { clientX: 20, clientY: 0, pointerId: 1 });
    });
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
    startDragging.mockReset().mockResolvedValue(undefined);
    invoke.mockReset().mockImplementation(async (command, args) => {
      if (command === "get_pet_activity") return initialActivity;
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
    await act(async () => {fireEvent.click(screen.getByRole("button",{name:"喂食"}));});
    expect(invoke).toHaveBeenCalledWith("interact_pet",{input:{personaId:"1",sessionId:"1",contextRevision:0,action:"feed"}});
  });

  it("状态旧事件不覆盖新会话，出门禁用互动", async () => {
    await act(async () => {render(<PetWindow />);});
    await act(async () => {
      receiveActivity({payload:{...initialActivity,sessionId:"2",contextRevision:1,revision:4,activity:"away",expression:"喵喵出门啦",canInteract:false}});
      receiveActivity({payload:{...initialActivity,revision:2}});
    });
    expect(screen.getByText("喵喵出门啦")).toBeTruthy();
    fireEvent.contextMenu(screen.getByRole("img"));
    expect((screen.getByRole("button",{name:"喂食"}) as HTMLButtonElement).disabled).toBe(true);
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
    await waitFor(() => expect(screen.getByRole("button", { name: "隐藏宠窗" }).title).toContain("宠窗操作失败"));
  });

  it("隐藏调用失败后回读权威状态并保留恢复入口", async () => {
    invoke.mockImplementation(async (command) => {
      if (command === "get_pet_window_state") return { hidden: true, revision: 2 };
      if (command === "set_pet_hidden") throw new Error("调整尺寸失败");
    });
    await act(async () => { render(<PetWindow />); });
    fireEvent.click(screen.getByRole("button", { name: "显示宠窗" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "显示宠窗" }).title).toContain("调整尺寸失败"));
    expect(screen.queryByRole("img")).toBeNull();
    expect(invoke.mock.calls.filter(([command]) => command === "get_pet_window_state")).toHaveLength(2);
  });

  it("区域携带CSS坐标与DPI，布局变化刷新，相同布局不重复提交", async () => {
    let x = 10;
    const bounds = vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => ({ x, y: 20, width: 30, height: 40, top: 20, left: x, right: x + 30, bottom: 60, toJSON: () => ({}) }));
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
  it("聊天与设置入口只导航，不触发桌宠互动或拖动", async () => {
    await act(async () => { render(<PetWindow />); });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "聊天" }));
      fireEvent.click(screen.getByRole("button", { name: "设置" }));
    });
    expect(invoke).toHaveBeenCalledWith("focus_or_create_window", { page: "chat" });
    expect(invoke).toHaveBeenCalledWith("focus_or_create_window", { page: "settings" });
    expect(invoke.mock.calls.some(([command]) => command === "interact_pet")).toBe(false);
    expect(startDragging).not.toHaveBeenCalled();
  });
});
