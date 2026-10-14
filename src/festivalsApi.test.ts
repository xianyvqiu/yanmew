import { expect, it, vi } from "vitest";
import { festivalsApi } from "./api";
const invoke = vi.hoisted(() => vi.fn().mockResolvedValue(null));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
it("日期API保持字符串ID、当前会话与旧快照token，新增没有母表关联", async () => {
  const context = { personaId: "1", sessionId: "9007199254740993", contextRevision: 4 };
  const input = { kind: "birthday" as const, name: "生日", calendarType: "lunar" as const, month: 11, day: 20, isLeapMonth: false, blessing: "生日快乐", note: null };
  await festivalsApi.catalog(); await festivalsApi.list(context); await festivalsApi.get(context, "9007199254740994");
  await festivalsApi.create(context, input); await festivalsApi.update(context, "9007199254740994", "old-token", input); await festivalsApi.delete(context, "9007199254740994", "new-token");
  expect(invoke.mock.calls).toEqual([
    ["list_festival_catalog"], ["list_session_festivals", { context, beforeId: null, limit: 30 }],
    ["get_session_festival", { context, id: "9007199254740994" }], ["create_session_festival", { context, input }],
    ["update_session_festival", { context, id: "9007199254740994", expected: "old-token", input }],
    ["delete_session_festival", { context, id: "9007199254740994", expected: "new-token" }],
  ]);
});
