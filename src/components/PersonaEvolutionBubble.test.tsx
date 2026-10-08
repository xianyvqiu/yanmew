import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import PersonaEvolutionBubble from "./PersonaEvolutionBubble";
import type { PersonaUpdatedNotice, PetActivity } from "../types";

const api = vi.hoisted(() => ({ listen: vi.fn(), stop: vi.fn() }));
vi.mock("../api", () => ({ configurationApi: { listenPersonaUpdates: api.listen } }));
let receive: (notice: PersonaUpdatedNotice) => void;
const view: PetActivity = { personaId: "1", sessionId: "2", contextRevision: 3, revision: 4,
  activity: "normal", expression: "(・ω・)", intimacy: 1, sleepUntil: null, error: null, canInteract: true, canWander: true };
const notice: PersonaUpdatedNotice = { personaId: "1", sessionId: "2", contextRevision: 3, evolvedAt: "2026-10-08T08:00:00.000Z" };
const fallback = <span>普通提示</span>;
beforeEach(() => {
  vi.useFakeTimers(); vi.clearAllMocks();
  api.listen.mockImplementation(async callback => { receive = callback; return api.stop; });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

it("成功提示8秒收起，重复事件不延时，不写入聊天", async () => {
  await act(async () => { render(<PersonaEvolutionBubble view={view} fallback={fallback} />); });
  act(() => receive(notice));
  expect(screen.getByRole("status").getAttribute("data-pet-interactive")).not.toBeNull();
  act(() => vi.advanceTimersByTime(7000));
  act(() => receive(notice));
  act(() => vi.advanceTimersByTime(1000));
  expect(screen.queryByRole("status")).toBeNull();
  act(() => receive(notice));
  expect(screen.getByText("普通提示")).toBeTruthy();
});

it.each(["personaId", "sessionId", "contextRevision"] as const)("不展示旧 %s 的通知", async field => {
  await act(async () => { render(<PersonaEvolutionBubble view={view} fallback={fallback} />); });
  act(() => receive({ ...notice, [field]: field === "contextRevision" ? 2 : "other" }));
  expect(screen.queryByRole("status")).toBeNull();
});

it("完成通知早于活动刷新时等空闲快照，切换后不重放", async () => {
  let page!: ReturnType<typeof render>;
  await act(async () => { page = render(<PersonaEvolutionBubble view={{ ...view, activity: "thinking", canWander: false }} fallback={fallback} />); });
  act(() => receive(notice));
  expect(screen.queryByRole("status")).toBeNull();
  page.rerender(<PersonaEvolutionBubble view={{ ...view, revision: 5 }} fallback={fallback} />);
  expect(screen.getByRole("status")).toBeTruthy();
  page.rerender(<PersonaEvolutionBubble view={{ ...view, contextRevision: 4 }} fallback={fallback} />);
  expect(screen.queryByRole("status")).toBeNull();
  page.rerender(<PersonaEvolutionBubble view={view} fallback={fallback} />);
  expect(screen.queryByRole("status")).toBeNull();
  page.unmount();
  expect(api.stop).toHaveBeenCalledTimes(1);
});
