import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PetWindow from "./PetWindow";

const startDragging = vi.fn();

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging }),
}));

describe("PetWindow", () => {
  beforeEach(() => {
    startDragging.mockClear();
  });

  it("渲染颜文字形象", () => {
    render(<PetWindow />);

    expect(screen.getByText("(・ω・)")).toBeTruthy();
  });

  it("左键单击颜文字时不开始拖动", () => {
    render(<PetWindow />);
    const pet = screen.getByText("(・ω・)");

    fireEvent.pointerDown(pet, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerUp(pet, { clientX: 10, clientY: 10, pointerId: 1 });

    expect(startDragging).not.toHaveBeenCalled();
  });

  it("移动超过阈值后只开始拖动一次", () => {
    render(<PetWindow />);
    const pet = screen.getByText("(・ω・)");

    fireEvent.pointerDown(pet, { button: 0, clientX: 10, clientY: 10, pointerId: 1 });
    fireEvent.pointerMove(pet, { clientX: 13, clientY: 13, pointerId: 1 });
    expect(startDragging).not.toHaveBeenCalled();

    fireEvent.pointerMove(pet, { clientX: 16, clientY: 10, pointerId: 1 });
    fireEvent.pointerMove(pet, { clientX: 20, clientY: 10, pointerId: 1 });

    expect(startDragging).toHaveBeenCalledTimes(1);
  });

  it("非颜文字区域不触发拖动", () => {
    render(<PetWindow />);
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
});
