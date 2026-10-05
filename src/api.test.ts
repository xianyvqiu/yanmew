import { beforeEach, expect, it, vi } from "vitest";
import { chatApi, embeddingApi, memoryManagementApi } from "./api";
import type { CurrentContext, MemoryDetail } from "./types";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
beforeEach(() => { invoke.mockReset().mockResolvedValue(null); });

it("继续生成和尾轮恢复传递固定上下文及原回合，不提交新用户输入", async () => {
  const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 3 };
  await chatApi.continue(context, "4");
  expect(invoke).toHaveBeenLastCalledWith("chat_continue_reply", { context, roundId: "4" });
  await chatApi.lastRound(context);
  expect(invoke).toHaveBeenLastCalledWith("get_last_chat_round", { context });
});

it("Embedding 保存只使用模型配置字段", async () => {
  const input = { name: "Embedding", baseUrl: "https://example.com/v1", model: "embedding-model", apiKey: { action: "keep" as const } };
  await embeddingApi.save(null, input);
  expect(invoke).toHaveBeenCalledWith("save_embedding_provider", { id: null, input });
});

it("记忆管理传递完整上下文，预览确认使用独立命令和 previewId", async () => {
  const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 3 };
  const expected: MemoryDetail = {
    record: { id: "4", sessionId: "2", kind: "short", roundId: "5", integrationState: "pending", title: null,
      content: "旧正文", sourceCount: null, activatedByRoundId: null, coveredThroughRoundId: null, coveredThroughMessageId: null, createdAt: "now", updatedAt: "now" },
    memoryRevision: 6, sources: [], relatedLongMemories: [],
  };
  await memoryManagementApi.list(context, "short");
  expect(invoke).toHaveBeenLastCalledWith("list_memories", { context, kind: "short", afterId: null, limit: 30 });
  await memoryManagementApi.list(context, "long", "9", 15);
  expect(invoke).toHaveBeenLastCalledWith("list_memories", { context, kind: "long", afterId: "9", limit: 15 });
  await memoryManagementApi.get(context, "short", "4");
  expect(invoke).toHaveBeenLastCalledWith("get_memory", { context, kind: "short", id: "4" });
  await memoryManagementApi.save(context, expected, null, "手动正文");
  expect(invoke).toHaveBeenLastCalledWith("save_memory", { context, expected, title: null, content: "手动正文" });
  await memoryManagementApi.preview(context, "short", "4");
  expect(invoke).toHaveBeenLastCalledWith("preview_memory", { context, kind: "short", id: "4" });
  await memoryManagementApi.confirm(context, "preview-1", null, "修改后的预览正文");
  expect(invoke).toHaveBeenLastCalledWith("confirm_memory_preview", { context, previewId: "preview-1", title: null, content: "修改后的预览正文" });
  await memoryManagementApi.cancel(context);
  expect(invoke).toHaveBeenLastCalledWith("cancel_memory_preview", { context });
  const receive = vi.fn();
  await memoryManagementApi.listen(receive);
  expect(listen).toHaveBeenCalledWith("memory-generation-changed", receive);
});
