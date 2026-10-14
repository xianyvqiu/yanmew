import { beforeEach, expect, it, vi } from "vitest";
import { travelJournalsApi, souvenirsApi, accessoryCraftApi, imageGenerationApi, accessoriesApi, backgroundApi, chatApi, embeddingApi, gamePluginsApi, lettersApi, travelApi, memoryManagementApi, walletApi } from "./api";
import type { CurrentContext, MemoryDetail } from "./types";

import { navigation } from "./platform/navigation";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
beforeEach(() => { invoke.mockReset().mockResolvedValue(null); });
it("插件安装分为预览与确认，使用数据管理固定版本且不传人设或会话", async () => {
  await gamePluginsApi.previewInstall("example.yanmew-game", "YWJj");
  expect(invoke).toHaveBeenLastCalledWith("preview_game_plugin_install", {filename: "example.yanmew-game", bytesBase64: "YWJj"});
  const choices = [{permission: "wallet.modify" as const, scope: "", decision: "denied" as const}];
  await gamePluginsApi.confirmInstall("install-token", choices);
  expect(invoke).toHaveBeenLastCalledWith("confirm_game_plugin_install", {token: "install-token", choices});
  await gamePluginsApi.uninstall("org.example.test", "1.0.0", false);
  expect(invoke).toHaveBeenLastCalledWith("uninstall_game_plugin", {id: "org.example.test", version: "1.0.0", clearData: false});
  await gamePluginsApi.history("org.example.test");
  expect(invoke).toHaveBeenLastCalledWith("list_game_plugin_runs", {id: "org.example.test", before: null, limit: 30});
});

it("钱包只查询全局余额，亲密度附带当前人设会话与版本", async () => {
  invoke.mockResolvedValueOnce({ coins: "9223372036854775807", updatedAt: "2026-10-08T00:00:00.000Z" });
  expect((await walletApi.get()).coins).toBe("9223372036854775807");
  expect(invoke).toHaveBeenLastCalledWith("get_wallet");
  await walletApi.intimacy({ personaId: "1", sessionId: "2", contextRevision: 3 });
  expect(invoke).toHaveBeenLastCalledWith("get_session_intimacy", { personaId: "1", sessionId: "2", contextRevision: 3 });
});

it("签到只发出全局命令，不传日期或奖励数量，并订阅钱包提交通知", async () => {
  await walletApi.signInStatus();
  expect(invoke).toHaveBeenLastCalledWith("get_sign_in_status");
  await walletApi.signIn();
  expect(invoke).toHaveBeenLastCalledWith("daily_sign_in");
  const receive = () => {};
  await walletApi.listen(receive);
  expect(listen).toHaveBeenLastCalledWith("wallet_changed", receive);
});

it("背景命令固定人设，聊天入口附带版本，设置入口不传版本", async () => {
  await backgroundApi.get("1", 3);
  expect(invoke).toHaveBeenLastCalledWith("get_chat_background", { id: "1", contextRevision: 3 });
  const input = { expected: { personaId: "1", file: null, opacity: 85, blur: 0 }, opacity: 60, blur: 8, image: null, reset: false };
  await backgroundApi.save("1", input);
  expect(invoke).toHaveBeenLastCalledWith("save_chat_background", { id: "1", input, contextRevision: null });
});

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

it("写信绑定当前版本及本次出行，不由前端传回信期限或完成状态", async () => {
  const context = { personaId: "1", sessionId: "2", contextRevision: 3 };
  await lettersApi.send(context, { travelId: "9007199254740993", content: "一路平安", replyToId: "7" });
  expect(invoke).toHaveBeenLastCalledWith("send_letter", { ...context, input: { travelId: "9007199254740993", content: "一路平安", replyToId: "7" } });
  await lettersApi.flags(context, "7", { read: false });
  expect(invoke).toHaveBeenLastCalledWith("set_letter_flags", { ...context, id: "7", favorite: null, read: false });
  await lettersApi.status(context);
  expect(invoke).toHaveBeenLastCalledWith("get_letter_generation_status", context);
  const receive = vi.fn();
  await lettersApi.listen(receive);
  const registration = listen.mock.calls[listen.mock.calls.length - 1]!;
  expect(registration[0]).toBe("letters-changed");
  registration[1]({ event: "letters-changed", id: 1, payload: { sessionId: "2" } });
  expect(receive).toHaveBeenCalledWith({ sessionId: "2" });
});

it("出行查询和写信窗口固定携带上下文与字符串 ID",async()=> {
  const context:CurrentContext={personaId:"1",sessionId:"2",contextRevision:3};
  await travelApi.current(context);
  expect(invoke).toHaveBeenLastCalledWith("get_current_travel",context);
  await travelApi.list(context,"9223372036854775807",15);
  expect(invoke).toHaveBeenLastCalledWith("list_travels",{...context,beforeId:"9223372036854775807",limit:15});
  await travelApi.status(context);
  expect(invoke).toHaveBeenLastCalledWith("get_travel_generation_status",context);
  await travelApi.retry(context);
  expect(invoke).toHaveBeenLastCalledWith("retry_travel_generation",context);
  await navigation.openLetter(context,"4","5");
  expect(invoke).toHaveBeenLastCalledWith("open_letter_window",{...context,travelId:"4",replyToId:"5"});
  await navigation.takeLetterRequest();
  expect(invoke).toHaveBeenLastCalledWith("take_letter_navigation");
  await navigation.takeTravelRequest();
  expect(invoke).toHaveBeenLastCalledWith("take_travel_navigation");
});

it("饰品接口附带完整上下文，只传ID，不允许页面提交图片路径或售价", async () => {
  const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 3 };
  await accessoriesApi.list(context);
  expect(invoke).toHaveBeenLastCalledWith("list_accessories", { context, beforeId: null, limit: 30 });
  await accessoriesApi.image(context, "4");
  expect(invoke).toHaveBeenLastCalledWith("get_accessory_image", { context, id: "4" });
  await accessoriesApi.wear(context, "4");
  expect(invoke).toHaveBeenLastCalledWith("wear_accessory", { context, id: "4" });
  await accessoriesApi.unwear(context, "4");
  expect(invoke).toHaveBeenLastCalledWith("unwear_accessory", { context, id: "4" });
  await accessoriesApi.sell(context, "4");
  expect(invoke).toHaveBeenLastCalledWith("sell_accessory", { context, id: "4" });
  await accessoriesApi.delete(context, "4");
  expect(invoke).toHaveBeenLastCalledWith("delete_accessory", { context, id: "4" });
  const receive = vi.fn();
  await accessoriesApi.listen(receive);
  const [name, callback] = listen.mock.calls[listen.mock.calls.length - 1];
  expect(name).toBe("accessories-changed");
  callback({ payload: { sessionId: "2" } });
  expect(receive).toHaveBeenCalledWith({ sessionId: "2" });
});

it("生图配置独立传版本与Key动作，制作只能查询确认固定预览", async () => {
  await imageGenerationApi.get(); expect(invoke).toHaveBeenLastCalledWith("get_image_generation_config");
  const input = { name: "生图", protocol: "openai-compatible" as const, baseUrl: "https://images.example/v1", model: "image-model", apiKey: { action: "keep" as const } };
  await imageGenerationApi.save("1", 2, input); expect(invoke).toHaveBeenLastCalledWith("save_image_generation_provider", { id: "1", expectedRevision: 2, input });
  await imageGenerationApi.select(null, 3); expect(invoke).toHaveBeenLastCalledWith("select_image_generation_provider", { id: null, expectedRevision: 3 });
  await imageGenerationApi.enable(false, 4); expect(invoke).toHaveBeenLastCalledWith("set_image_generation_enabled", { enabled: false, expectedRevision: 4 });
  const context: CurrentContext = { personaId: "1", sessionId: "2", contextRevision: 3 };
  await accessoryCraftApi.current(context); expect(invoke).toHaveBeenLastCalledWith("get_current_accessory_craft_preview", { context });
  await accessoryCraftApi.get(context, "preview"); expect(invoke).toHaveBeenLastCalledWith("get_accessory_craft_preview", { context, previewId: "preview" });
  await accessoryCraftApi.confirm(context, "preview"); expect(invoke).toHaveBeenLastCalledWith("confirm_accessory_craft", { context, previewId: "preview" });
  await accessoryCraftApi.cancel(context, "preview"); expect(invoke).toHaveBeenLastCalledWith("cancel_accessory_craft", { context, previewId: "preview" });
});

it("伴手礼管理和重试仅传当前上下文，不接受发放状态或金额",async()=> {
  const context:CurrentContext={personaId:"1",sessionId:"2",contextRevision:3};
  await souvenirsApi.list(context);expect(invoke).toHaveBeenLastCalledWith("list_souvenirs",{context,beforeId:null,limit:30});
  await souvenirsApi.get(context,"4");expect(invoke).toHaveBeenLastCalledWith("get_souvenir",{context,id:"4"});
  await souvenirsApi.favorite(context,"4",true);expect(invoke).toHaveBeenLastCalledWith("set_souvenir_favorite",{context,id:"4",favorite:true});
  await souvenirsApi.delete(context,"4");expect(invoke).toHaveBeenLastCalledWith("delete_souvenir",{context,id:"4"});
  await souvenirsApi.status(context);expect(invoke).toHaveBeenLastCalledWith("get_gift_generation_status",{context});
  await souvenirsApi.retry(context);expect(invoke).toHaveBeenLastCalledWith("retry_gift_generation",{context});
});

it("游记编辑携带旧内容快照与来源出行，删除不提交重置状态",async()=> {
  const context:CurrentContext={personaId:"1",sessionId:"2",contextRevision:3};
  const expected={title:"原标题",content:"原正文",updatedAt:"original-time"};
  await travelJournalsApi.list(context);expect(invoke).toHaveBeenLastCalledWith("list_travel_journals",{context,beforeId:null,limit:30});
  await travelJournalsApi.get(context,"4");expect(invoke).toHaveBeenLastCalledWith("get_travel_journal",{context,id:"4"});
  await travelJournalsApi.update(context,"4","5",expected,"新标题","正文");expect(invoke).toHaveBeenLastCalledWith("update_travel_journal",{context,id:"4",travelId:"5",expected,title:"新标题",content:"正文"});
  await travelJournalsApi.delete(context,"4","5");expect(invoke).toHaveBeenLastCalledWith("delete_travel_journal",{context,id:"4",travelId:"5"});
  await travelJournalsApi.status(context);expect(invoke).toHaveBeenLastCalledWith("get_journal_generation_status",{context});
  await travelJournalsApi.retry(context);expect(invoke).toHaveBeenLastCalledWith("retry_journal_generation",{context});
});
