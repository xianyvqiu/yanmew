import { invoke } from "@tauri-apps/api/core";
/** 应用信息来自运行程序；退出复用已有原生退出事件的保存流程。 */
export type AppInfo = { name:string;version:string;buildId:string;mode:"portable"|"installed";dataDirectory:string;programDirectory:string };
export type AppUpdateStatus = {phase:"idle"|"checking"|"no_release"|"available"|"up_to_date"|"downloading"|"installing"|"error";message:string;checkedAt:string|null;downloaded:number;total:number;candidate:{manifest:{version:string;buildId:string;assetName:string;size:number;sha256:string;schemaVersion:number};releaseUrl:string;notes:string}|null};
export const appApi = {
  info: () => invoke<AppInfo>("get_app_info"),
  exit: () => invoke<void>("request_app_exit"),
  updateStatus:()=>invoke<AppUpdateStatus>("get_app_update_status"),
  checkUpdate:()=>invoke<AppUpdateStatus>("check_app_update"),
  update:()=>invoke<void>("install_app_update",{confirmedByUser:true}),
  openInformation:(kind:"repository"|"community"|"issues"|"releases"|"updateRelease"|"data")=>invoke<void>("open_app_information_link",{kind}),
  uninstall:()=>invoke<void>("uninstall_app",{confirmedByUser:true}),
};
import type { BackupRequest, BackupArchive, BackupExportOptions, BackupImportPreview, BackupImportConfirm, BackupImported } from "./types";
/** 六类导出共用流式文件协议；后端系统窗口选择目标，取消返回null。 */
export const backupApi = {
  prepareImport: () => invoke<BackupImportPreview | null>("prepare_backup_import"),
  refreshImport: (previewId: string) => invoke<BackupImportPreview>("refresh_backup_import", { previewId }),
  cancelImport: (previewId: string) => invoke<void>("cancel_backup_import", { previewId }),
  confirmImport: (input: BackupImportConfirm) => invoke<BackupImported>("confirm_backup_import", { input }),
  options: () => invoke<BackupExportOptions>("get_backup_export_options"),
  export: (input: BackupRequest) => invoke<BackupArchive | null>("export_backup", { input }),
  exportPluginData: (ids: string[]) => invoke<BackupArchive | null>("export_game_plugin_data", { ids }),
};
import type { GamePlugin, GamePluginDetail, GameInstallPreview, GamePermissionChoice, GameRun, GamePluginLedger, PluginJson } from "./types";
/** 仅设置窗口可调用管理接口；小游戏启动与管理共用现有宿主。 */
export const gamePluginsApi = {
  list: () => invoke<GamePlugin[]>("list_game_plugins"),
  detail: (id: string) => invoke<GamePluginDetail>("get_game_plugin", { id }),
  previewInstall: (filename: string, bytesBase64: string) => invoke<GameInstallPreview>("preview_game_plugin_install", { filename, bytesBase64 }),
  confirmInstall: (token: string, choices: GamePermissionChoice[]) => invoke<GamePlugin>("confirm_game_plugin_install", { token, choices }),
  cancelInstall: (token: string) => invoke<void>("cancel_game_plugin_install", { token }),
  enable: (id: string, version: string, enabled: boolean) => invoke<void>("set_game_plugin_enabled", { id, version, enabled }),
  permission: (id: string, version: string, choice: GamePermissionChoice) => invoke<void>("set_game_plugin_permission", { id, version, choice }),
  setting: (id: string, version: string, key: string, value: PluginJson) => invoke<void>("save_game_plugin_setting", { id, version, key, value }),
  uninstall: (id: string, version: string, clearData: boolean) => invoke<void>("uninstall_game_plugin", { id, version, clearData }),
  clearData: (id: string, version: string) => invoke<void>("clear_game_plugin_data", { id, version }),
  history: (id: string, before: string | null = null, limit = 30) => invoke<GameRun[]>("list_game_plugin_runs", { id, before, limit }),
  ledger: (id: string, before: string | null = null, limit = 30) => invoke<GamePluginLedger[]>("list_game_plugin_ledger", { id, before, limit }),
  open: (id: string) => invoke<string>("open_game_plugin", { id }),
  listen: (receive: () => void) => listen("game_plugins_changed", receive),
};
import type { WebSearchSettings, WebSearchProviderInput } from "./types";
export const webSearchApi = {
  get: () => invoke<WebSearchSettings>("get_web_search_settings"),
  save: (id: string | null, input: WebSearchProviderInput) => invoke<void>("save_web_search_provider", { id, input }),
  delete: (id: string) => invoke<void>("delete_web_search_provider", { id }),
  select: (id: string) => invoke<void>("set_active_web_search_provider", { id }),
  enable: (enabled: boolean) => invoke<void>("set_web_search_enabled", { enabled }),
  test: (id: string) => invoke<number>("test_web_search_provider", { id }),
  open: (url: string) => invoke<void>("open_web_source", { url }),
  listen: (receive: () => void) => listen("configuration_changed", receive),
};
import type { Wallet, SessionIntimacy, SignInStatus, DailySignInResult } from "./types";
export const walletApi = {
  get: () => invoke<Wallet>("get_wallet"),
  signInStatus: () => invoke<SignInStatus>("get_sign_in_status"),
  signIn: () => invoke<DailySignInResult>("daily_sign_in"),
  listen: (receive: () => void) => listen("wallet_changed", receive),
  intimacy: (context: CurrentContext) => invoke<SessionIntimacy>("get_session_intimacy", {
    personaId: context.personaId, sessionId: context.sessionId, contextRevision: context.contextRevision,
  }),
};
import type { ChatBackgroundInput, ChatBackgroundRecord, ChatBackgroundView } from "./types";
export const backgroundApi = {
  get: (id: RecordId, contextRevision?: number) => invoke<ChatBackgroundView>("get_chat_background", { id, contextRevision: contextRevision ?? null }),
  save: (id: RecordId, input: ChatBackgroundInput, contextRevision?: number) => invoke<ChatBackgroundRecord>("save_chat_background", { id, input, contextRevision: contextRevision ?? null }),
};
import type { AutostartStatus } from "./types";
export const autostartApi = {
  get: () => invoke<AutostartStatus>("get_autostart_status"),
  set: (enabled: boolean) => invoke<AutostartStatus>("set_autostart", { enabled }),
};
import type { EmbeddingProvider, EmbeddingProviderInput, EmbeddingStatus } from "./types";
export const embeddingApi = {
  list: () => invoke<EmbeddingProvider[]>("list_embedding_providers"),
  save: (id: RecordId | null, input: EmbeddingProviderInput) => invoke<EmbeddingProvider>("save_embedding_provider", { id, input }),
  delete: (id: RecordId) => invoke<void>("delete_embedding_provider", { id }),
  select: (id: RecordId) => invoke<void>("set_active_embedding_provider", { id }),
  test: (id: RecordId) => invoke<number>("test_embedding_provider", { id }),
  status: () => invoke<EmbeddingStatus>("get_embedding_status"),
  listen: (receive: () => void) => listen("embedding-state-changed", receive),
};
import type { Notebook, NotebookView } from "./types";
import type { MemoryGenerationView } from "./types";
import type { MemoryDetail, MemoryKind, MemoryPage, MemoryPreview } from "./types";
export const memoryManagementApi = {
  list: (context: CurrentContext, kind: MemoryKind, afterId: RecordId | null = null, limit = 30) =>
    invoke<MemoryPage>("list_memories", { context, kind, afterId, limit }),
  get: (context: CurrentContext, kind: MemoryKind, id: RecordId) =>
    invoke<MemoryDetail>("get_memory", { context, kind, id }),
  save: (context: CurrentContext, expected: MemoryDetail, title: string | null, content: string) =>
    invoke<MemoryDetail>("save_memory", { context, expected, title, content }),
  preview: (context: CurrentContext, kind: MemoryKind, id: RecordId) =>
    invoke<MemoryPreview>("preview_memory", { context, kind, id }),
  confirm: (context: CurrentContext, previewId: string, title: string | null, content: string) =>
    invoke<MemoryDetail>("confirm_memory_preview", { context, previewId, title, content }),
  cancel: (context: CurrentContext) => invoke<void>("cancel_memory_preview", { context }),
  listen: (receive: () => void) => listen("memory-generation-changed", receive),
};
export const memoryGenerationApi = {
  get: (context: CurrentContext) => invoke<MemoryGenerationView>("get_memory_generation_status", context),
  retry: (context: CurrentContext) => invoke<void>("retry_short_memories", context),
  listen: (receive: () => void) => listen("memory-generation-changed", receive),
};
export const notebookApi = {
  get: (context: CurrentContext) => invoke<NotebookView>("get_notebook", context),
  save: (context: CurrentContext, expected: Notebook | null, content: string) =>
    invoke<NotebookView>("save_notebook", { ...context, expected, content }),
  listen: (receive: () => void) => listen("notebook-changed", receive),
};
import type { ChatReplyEvent, ChatRequestSnapshot, RoundSnapshot } from "./types";
import type { Message } from "./types";

export const chatApi = {
  image:(context:CurrentContext,id:string)=>invoke<string>("get_chat_image",{context,id}),
  edit: (context: CurrentContext, message: Message, content: string) => invoke<Message>("update_chat_message", { context, messageId: message.id, expectedContent: message.content, content }),
  deleteTail: (context: CurrentContext, message: Message) => invoke<void>("delete_tail_message", { context, messageId: message.id, expectedContent: message.content }),
  regenerate: (context: CurrentContext, roundId: RecordId) => invoke<ChatRequestSnapshot>("regenerate_last_reply", { context, roundId }),
  listenMessages: (receive: (change: { sessionId: RecordId }) => void) => listen<{ sessionId: RecordId }>("chat-messages-changed", event => receive(event.payload)),
  submit: (context: CurrentContext, input: string, images:import("./types").ChatImageInput[]=[]) => invoke<ChatRequestSnapshot>("submit_chat", {
    personaId: context.personaId, sessionId: context.sessionId, contextRevision: context.contextRevision, input, ...(images.length?{images}:{}),
  }),
  continue: (context: CurrentContext, roundId: RecordId) => invoke<ChatRequestSnapshot>("chat_continue_reply", { context, roundId }),
  lastRound: (context: CurrentContext) => invoke<RoundSnapshot | null>("get_last_chat_round", { context }),
  current: (context: CurrentContext) => invoke<ChatRequestSnapshot | null>("get_chat_request", {
    personaId: context.personaId, sessionId: context.sessionId, contextRevision: context.contextRevision,
  }),
  cancel: (request: Pick<ChatRequestSnapshot, "personaId" | "sessionId" | "requestId" | "roundId">) =>
    invoke<RoundSnapshot>("cancel_chat", request),
  listen: (receive: (event: ChatReplyEvent) => void) => listen<ChatReplyEvent>("chat-reply", event => receive(event.payload)),
};
import type { WeatherView } from "./types";

export const weatherApi = {
  refreshInformation: (contextRevision:number) => invoke<void>("refresh_weather_information",{contextRevision}),
  retryCare: (contextRevision:number,kind:"warnings"|"typhoons") => invoke<void>("retry_weather_care",{contextRevision,kind}),
  listen: (handler: () => void) => listen("weather-changed", handler),
  get: () => invoke<WeatherView>("get_weather"),
  refresh: (contextRevision: number) => invoke<WeatherView>("refresh_weather", { contextRevision }),
  cancel: () => invoke<void>("cancel_weather_refresh"),
};
import type { Reminder, ReminderInput, MissedReminderSummary, ReminderDeliveryState, ReminderOccurrence, ReminderOccurrencePage, ReminderStatus } from "./types";

export const reminderApi = {
  polish: (id: RecordId, personaId: RecordId, contextRevision: number) => invoke<import("./types").ReminderPresentation | null>("polish_reminder_occurrence", { id, personaId, contextRevision }),
  getOccurrence: (id: string) => invoke<ReminderOccurrence>("get_reminder_occurrence", { id }),
  list: (afterId: string | null = null, limit = 50) => invoke<Reminder[]>("list_reminders", { afterId, limit }),
  create: (input: ReminderInput) => invoke<Reminder>("create_reminder", { input }),
  update: (id: string, input: ReminderInput) => invoke<Reminder>("update_reminder", { id, input }),
  delete: (id: string) => invoke<void>("delete_reminder", { id }),
  missed: (afterId: string | null = null) => invoke<MissedReminderSummary>("get_missed_reminder_summary", { afterId }),
  acknowledgeMissed: (ids: string[]) => invoke<void>("ack_missed_reminder_summary", { ids }),
  listen: (receive: () => void) => listen("reminders-changed", receive),
  listenError: (receive: (error: unknown) => void) => listen("reminder-error", event => receive(event.payload)),
  listOccurrences: (options: { reminderId?: string; deliveryState?: ReminderDeliveryState; afterId?: string; limit?: number } = {}) =>
    invoke<ReminderOccurrencePage>("list_reminder_occurrences", { ...options, limit: options.limit ?? 50 }),
  acknowledge: (ids: string[], deliveryState: "queued" | "missed") =>
    invoke<void>("acknowledge_reminder_occurrences", { ids, deliveryState }),
  updateStatus: (id: string, status: ReminderStatus) =>
    invoke<ReminderOccurrence>("update_reminder_occurrence_status", { id, status }),
  complete: (id: string) => invoke<ReminderOccurrence>("complete_reminder_occurrence", { id }),
};
import type { ExpressionLibrary, PetActivity, PetAction } from "./types";
import { listen } from "@tauri-apps/api/event";
import type { CurrentContext, MessagePage, Persona, PersonaInput, Provider, ProviderInput, RecordId, Session, SessionDeleteResult, Settings, WeatherInput, WeatherSettings } from "./types";

export const configurationApi = {
  listenPersonaUpdates: (receive: (update: import("./types").PersonaUpdatedNotice) => void) =>
    listen<import("./types").PersonaUpdatedNotice>("persona-updated", event => receive(event.payload)),
  listenExpressionFailure: (receive: (failure: { personaId: RecordId; error: import("./types").AppError }) => void) =>
    listen<{ personaId: RecordId; error: import("./types").AppError }>("expression_generation_failed", event => receive(event.payload)),
  listenContext: (receive: (context: CurrentContext) => void) => listen<CurrentContext>("window_context_changed", event => receive(event.payload)),
  listenChanges: (receive: () => void) => listen("configuration_changed", receive),
  status: () => invoke<void>("get_configuration_status"),
  getCurrentContext: () => invoke<CurrentContext>("get_current_context"),
  getWeatherSettings: () => invoke<WeatherSettings>("get_weather_settings"),
  saveWeatherSettings: (input: WeatherInput) => invoke<WeatherSettings>("save_weather_settings", { input }),
  getSettings: () => invoke<Settings>("get_settings"),
  getSettingsDefaults: () => invoke<Settings>("get_settings_defaults"),
  saveSettings: (changes: Partial<Settings>) => invoke<Settings>("save_settings", { changes }),
  listProviders: () => invoke<Provider[]>("list_providers"),
  roles: () => invoke<import("./types").ProviderRoles>("get_provider_roles"),
  saveRole: (purpose:import("./types").ProviderRole["purpose"],providerId:string|null,model:string,enabled:boolean,expectedRevision:number,maxOutputTokens?:number,contextWindowOverride?:number|null) => invoke<import("./types").ProviderRoles>("save_provider_role",{purpose,providerId,model,enabled,expectedRevision,maxOutputTokens,contextWindowOverride}),
  testProvider: (id: RecordId) => invoke<void>("test_provider_connection", { id }),
  createProvider: (input: ProviderInput) => invoke<Provider>("create_provider", { input }),
  updateProvider: (id: RecordId, input: ProviderInput) => invoke<Provider>("update_provider", { id, input }),
  deleteProvider: (id: RecordId) => invoke<void>("delete_provider", { id }),
  setChatProvider: (id: RecordId) => invoke<void>("set_active_chat_provider", { id }),
  setMemoryProvider: (id: RecordId | null) => invoke<void>("set_memory_provider", { id }),
  listPersonas: () => invoke<Persona[]>("list_personas"),
  createPersona: (input: PersonaInput) => invoke<Persona>("create_persona", { input }),
  generateExpressions: (id: RecordId) => invoke<ExpressionLibrary>("generate_expressions", { id }),
  saveExpressions: (id: RecordId, expected: Record<string, unknown> | null, library: Record<string, unknown>) => invoke<Persona>("save_persona_expressions", { id, expected, library }),
  cancelExpressionGeneration: (id: RecordId) => invoke<void>("cancel_expression_generation", { id }),
  updatePersona: (id: RecordId, input: PersonaInput) => invoke<Persona>("update_persona", { id, input }),
  deletePersona: (id: RecordId) => invoke<CurrentContext>("delete_persona", { id }),
  setPersona: (id: RecordId) => invoke<CurrentContext>("set_active_persona", { id }),
};

export const petApi = {
  updateWanderLayout: (layout: import("./types").PetWanderLayout) => invoke<void>("update_pet_wander_layout", { layout }),
  listenWander: (receive: (target: import("./types").PetWanderTarget) => void) =>
    listen<import("./types").PetWanderTarget>("pet-wander-target", event => receive(event.payload)),
  get: () => invoke<PetActivity>("get_pet_activity"),
  interact: (view: PetActivity, action: PetAction) => invoke<PetActivity>("interact_pet", { input: {
    personaId: view.personaId, sessionId: view.sessionId, contextRevision: view.contextRevision, action,
  } }),
  dragging: (dragging: boolean) => invoke<void>("set_pet_dragging", { dragging }),
  refreshProvider: () => invoke<void>("refresh_pet_provider"),
  listen: (receive: (view: PetActivity) => void) => listen<PetActivity>("pet-activity-changed", event => receive(event.payload)),
  listenStateBubble: (receive: (notice: import("./types").PetStateNotice) => void) =>
    listen<import("./types").PetStateNotice>("pet-state-bubble", event => receive(event.payload)),
  listenProactiveMessage: (receive: (notice: import("./types").ProactiveMessageNotice) => void) =>
    listen<import("./types").ProactiveMessageNotice>("proactive-message", event => receive(event.payload)),
  listenError: (receive: (error: unknown) => void) => listen("pet-activity-error", event => receive(event.payload)),
};

export const userProfileApi = {
  list: () => invoke<import("./types").UserProfile[]>("list_user_profiles"),
  create: (input: { name: string; content: string }) => invoke<import("./types").UserProfile>("create_user_profile", input),
  update: (input: { id: string; name: string; content: string }) => invoke<import("./types").UserProfile>("update_user_profile", input),
  delete: (id: RecordId) => invoke<void>("delete_user_profile", { id }),
};

export const sessionApi = {
  list: (context: CurrentContext) => invoke<Session[]>("list_sessions", {
    personaId: context.personaId, contextRevision: context.contextRevision,
  }),
  create: (context: CurrentContext, userProfileId: RecordId | null = null) => invoke<Session>("create_session", {
    personaId: context.personaId, contextRevision: context.contextRevision, userProfileId,
  }),
  setUserProfile: (context: CurrentContext, sessionId: RecordId, userProfileId: RecordId | null) => invoke<Session>("set_session_user_profile", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision, userProfileId,
  }),
  select: (context: CurrentContext, sessionId: RecordId) => invoke<CurrentContext>("set_active_session", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision,
  }),
  rename: (context: CurrentContext, sessionId: RecordId, title: string) => invoke<Session>("rename_session", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision, title,
  }),
  setPinned: (context: CurrentContext, sessionId: RecordId, isPinned: boolean) => invoke<Session>("set_session_pinned", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision, isPinned,
  }),
  delete: (context: CurrentContext, sessionId: RecordId) => invoke<SessionDeleteResult>("delete_session", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision,
  }),
  messages: (context: CurrentContext, afterId: RecordId | null = null, limit = 100, backward = false) => invoke<MessagePage>("list_messages", {
    personaId: context.personaId, sessionId: context.sessionId,
    contextRevision: context.contextRevision, afterId, limit, ...(backward ? {backward:true} : {}),
  }),
};

export function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message;
  return typeof error === "string" ? error : "操作失败，请重试";
}

import type { Letter, SendLetterInput, LetterGenerationStatus } from "./types";
/** 所有信件命令跟随当前上下文；分页按ID倒序，界面在14.4接入。 */
export const lettersApi = {
  list: (context: CurrentContext, beforeId: RecordId | null = null, limit = 30) => invoke<Letter[]>("list_letters", { ...context, beforeId, limit }),
  get: (context: CurrentContext, id: RecordId) => invoke<Letter>("get_letter", { ...context, id }),
  send: (context: CurrentContext, input: SendLetterInput) => invoke<Letter>("send_letter", { ...context, input }),
  flags: (context: CurrentContext, id: RecordId, flags: { favorite?: boolean; read?: boolean }) => invoke<Letter>("set_letter_flags", { ...context, id, favorite: flags.favorite ?? null, read: flags.read ?? null }),
  delete: (context: CurrentContext, id: RecordId) => invoke<void>("delete_letter", { ...context, id }),
  status: (context: CurrentContext) => invoke<LetterGenerationStatus>("get_letter_generation_status", { ...context }),
  retry: (context: CurrentContext) => invoke<void>("retry_letter_generation", { ...context }),
  listen: (receive: (event: { sessionId: RecordId }) => void) => listen<{ sessionId: RecordId }>("letters-changed", event => receive(event.payload)),
};

import type { Travel, TravelGenerationStatus } from "./types";
/** 当前出行与历史以服务端状态为准；不提交计时或生成标记。 */
export const travelApi = {
  current: (context: CurrentContext) => invoke<Travel | null>("get_current_travel", {...context}),
  list: (context: CurrentContext, beforeId: RecordId | null = null, limit = 30) => invoke<Travel[]>("list_travels", {...context,beforeId,limit}),
  status: (context: CurrentContext) => invoke<TravelGenerationStatus>("get_travel_generation_status", {...context}),
  retry: (context: CurrentContext) => invoke<void>("retry_travel_generation", {...context}),
  listen: (receive: (event: {sessionId: RecordId}) => void) => listen<{sessionId: RecordId}>("travel-changed", event => receive(event.payload)),
};

import type { Accessory, AccessoryImage, AccessoryDeleteResult, AccessorySellResult } from "./types";
/** 不接收图片路径、售价或会话之外的目标；所有变更都由后端验证。 */
export const accessoriesApi = {
  list: (context: CurrentContext, beforeId: RecordId | null = null, limit = 30) => invoke<Accessory[]>("list_accessories", { context, beforeId, limit }),
  image: (context: CurrentContext, id: RecordId) => invoke<AccessoryImage>("get_accessory_image", { context, id }),
  wear: (context: CurrentContext, id: RecordId) => invoke<Accessory>("wear_accessory", { context, id }),
  unwear: (context: CurrentContext, id: RecordId) => invoke<Accessory>("unwear_accessory", { context, id }),
  sell: (context: CurrentContext, id: RecordId) => invoke<AccessorySellResult>("sell_accessory", { context, id }),
  delete: (context: CurrentContext, id: RecordId) => invoke<AccessoryDeleteResult>("delete_accessory", { context, id }),
  listen: (receive: (event: { sessionId: RecordId }) => void) => listen<{ sessionId: RecordId }>("accessories-changed", event => receive(event.payload)),
};

import type { ImageGenerationConfig, ImageGenerationProviderInput, AccessoryCraftPreview } from "./types";
/** 生图配置检查只在页面完成；此接口没有付费测试调用。 */
export const imageGenerationApi = {
  get: () => invoke<ImageGenerationConfig>("get_image_generation_config"),
  save: (id: RecordId | null, expectedRevision: number, input: ImageGenerationProviderInput) => invoke<ImageGenerationConfig>("save_image_generation_provider", { id, expectedRevision, input }),
  select: (id: RecordId | null, expectedRevision: number) => invoke<ImageGenerationConfig>("select_image_generation_provider", { id, expectedRevision }),
  enable: (enabled: boolean, expectedRevision: number) => invoke<ImageGenerationConfig>("set_image_generation_enabled", { enabled, expectedRevision }),
  delete: (id: RecordId, expectedRevision: number) => invoke<ImageGenerationConfig>("delete_image_generation_provider", { id, expectedRevision }),
  listen: (receive: () => void) => listen("image-generation-config-changed", receive),
};
export const accessoryCraftApi = {
  current: (context: CurrentContext) => invoke<AccessoryCraftPreview | null>("get_current_accessory_craft_preview", { context }),
  get: (context: CurrentContext, previewId: string) => invoke<AccessoryCraftPreview>("get_accessory_craft_preview", { context, previewId }),
  confirm: (context: CurrentContext, previewId: string) => invoke<Accessory>("confirm_accessory_craft", { context, previewId }),
  cancel: (context: CurrentContext, previewId: string) => invoke<void>("cancel_accessory_craft", { context, previewId }),
  listen: (receive: (event: { sessionId: RecordId; previewId: string }) => void) => listen<{ sessionId: RecordId; previewId: string }>("accessory-craft-preview-changed", event => receive(event.payload)),
};

import type { Souvenir, GiftGenerationStatus } from "./types";
export const souvenirsApi = {
  image:(context:CurrentContext,id:RecordId)=>invoke<{dataUrl:string|null;notice:string|null}>("get_souvenir_image",{context,id}),
  imageSettings:()=>invoke<boolean>("get_souvenir_image_settings"),
  setImageSettings:(enabled:boolean)=>invoke<boolean>("set_souvenir_image_settings",{enabled}),
  listenImageSettings:(receive:(enabled:boolean)=>void)=>listen<boolean>("souvenir-image-settings-changed",event=>receive(event.payload)),
  list: (context:CurrentContext,beforeId:RecordId|null=null,limit=30) => invoke<Souvenir[]>("list_souvenirs",{context,beforeId,limit}),
  get: (context:CurrentContext,id:RecordId) => invoke<Souvenir>("get_souvenir",{context,id}),
  favorite: (context:CurrentContext,id:RecordId,favorite:boolean) => invoke<Souvenir>("set_souvenir_favorite",{context,id,favorite}),
  delete: (context:CurrentContext,id:RecordId) => invoke<void>("delete_souvenir",{context,id}),
  status: (context:CurrentContext) => invoke<GiftGenerationStatus>("get_gift_generation_status",{context}),
  retry: (context:CurrentContext) => invoke<void>("retry_gift_generation",{context}),
};

import type { TravelJournal, TravelJournalSummary, TravelJournalExpected, TravelJournalDeleted, JournalGenerationStatus } from "./types";
/** 列表无正文，详情单独读取；编辑用原快照，删除不重置已生成状态。 */
export const travelJournalsApi = {
  list: (context:CurrentContext,beforeId:RecordId|null=null,limit=30) => invoke<TravelJournalSummary[]>("list_travel_journals",{context,beforeId,limit}),
  get: (context:CurrentContext,id:RecordId) => invoke<TravelJournal>("get_travel_journal",{context,id}),
  update: (context:CurrentContext,id:RecordId,travelId:RecordId,expected:TravelJournalExpected,title:string,content:string) => invoke<TravelJournal>("update_travel_journal",{context,id,travelId,expected,title,content}),
  delete: (context:CurrentContext,id:RecordId,travelId:RecordId) => invoke<TravelJournalDeleted>("delete_travel_journal",{context,id,travelId}),
  status: (context:CurrentContext) => invoke<JournalGenerationStatus>("get_journal_generation_status",{context}),
  retry: (context:CurrentContext) => invoke<void>("retry_journal_generation",{context}),
};

import type { FestivalCatalogRecord, CustomFestivalInput, CustomFestivalRecord } from "./types";
/** 母表只读，全局自定义日期修改携带旧记录token，不依赖当前会话。 */
export const festivalsApi = {
  month: (year: number, month: number) => invoke<import("./types").CalendarMonth>("get_calendar_month", { year, month }),
  listen: (receive: (event: { sessionId: RecordId | null }) => void) => listen<{ sessionId: RecordId | null }>("festivals-changed", event => receive(event.payload)),
  catalog: () => invoke<FestivalCatalogRecord[]>("list_festival_catalog"),
  list: (beforeId: RecordId | null = null, limit = 30) => invoke<CustomFestivalRecord[]>("list_custom_festivals", { beforeId, limit }),
  get: (id: RecordId) => invoke<CustomFestivalRecord>("get_custom_festival", { id }),
  create: (input: CustomFestivalInput) => invoke<CustomFestivalRecord>("create_custom_festival", { input }),
  update: (id: RecordId, expected: string, input: CustomFestivalInput) => invoke<CustomFestivalRecord>("update_custom_festival", { id, expected, input }),
  delete: (id: RecordId, expected: string) => invoke<void>("delete_custom_festival", { id, expected }),
};

export type CardKind="travels"|"letters"|"journals"|"accessories"|"souvenirs"|"dates";
export const cardBrowserApi={
  browse:<T,>(kind:CardKind,context:CurrentContext,search:string,order:"newest"|"oldest",page:number,travelIds:string[]=[],favoritesOnly=false)=>invoke<{items:T[];total:number;page:number;pages:number;travels?:import("./types").TravelOrigin[]}>("browse_cards",{input:{kind,context,search,order,page,travelIds,...(favoritesOnly?{favoritesOnly:true}:{})}}),
  listen:(kind:CardKind,receive:()=>void)=>listen(kind==="accessories"?"accessories-changed":kind==="dates"?"festivals-changed":"travel-changed",receive),
};

import type { ReminderEmailView, ReminderEmailInput } from "./types";
export const reminderEmailApi = {
  get: () => invoke<ReminderEmailView>("get_reminder_email"),
  save: (input: ReminderEmailInput) => invoke<ReminderEmailView>("save_reminder_email", { input }),
  sendCode: (input: ReminderEmailInput) => invoke<ReminderEmailView>("send_reminder_email_code", { input }),
  verify: (code: string, revision: number) => invoke<ReminderEmailView>("verify_reminder_email", { code, revision }),
  enable: (enabled: boolean, revision: number) => invoke<ReminderEmailView>("set_reminder_email_enabled", { enabled, revision }),
  retry: (id: string, revision: number, confirmUncertain: boolean) => invoke<ReminderEmailView>("retry_reminder_email", { id, revision, confirmUncertain }),
};

import type { PetCreation, PetArtwork, PetArtworkInput } from "./types";
export const petCreationApi = {
 get:(personaId:string)=>invoke<PetCreation>("get_pet_creation",{personaId}),
 save:(personaId:string,id:string|null,revision:number|null,input:PetArtworkInput)=>invoke<PetArtwork>("save_pet_artwork",{personaId,id,revision,input}),
 enable:(personaId:string,enabled:boolean,revision:number)=>invoke<void>("set_pet_creation_enabled",{personaId,enabled,revision}),
 image:(personaId:string,id:string,preview=false)=>invoke<string>("get_pet_artwork_image",{personaId,id,preview}),
 upload:(personaId:string,id:string,revision:number,data:string)=>invoke<PetArtwork>("upload_pet_artwork",{personaId,id,revision,data}),
 finish:(personaId:string,id:string,revision:number,action:"accept"|"discard"|"delete")=>invoke<void>("finish_pet_artwork",{personaId,id,revision,action}),
 generate:(personaId:string,id:string,revision:number)=>invoke<PetArtwork>("generate_pet_artwork",{personaId,id,revision}),
 cancel:(personaId:string,id:string)=>invoke<void>("cancel_pet_artwork",{personaId,id}),
 listen:(receive:()=>void)=>listen("pet_creation_changed",receive),
};
export const saveAccessoryPlacement=(context:CurrentContext,id:string,x:number|null,y:number|null,scale:number)=>invoke<Accessory>("save_accessory_placement",{context,id,x,y,scale});

export const moveAccessoryLayer=(context:CurrentContext,id:string,plane:"front"|"back",action:"keep"|"up"|"down"|"top"|"bottom")=>invoke<Accessory>("move_accessory_layer",{context,id,plane,action});

export const getPetErrorExpression=(personaId:string)=>invoke<string>("get_pet_error_expression",{personaId});
export const getPetLiftExpression=(personaId:string)=>invoke<string>("get_pet_lift_expression",{personaId});

export const uploadAccessoryCraft=(context:CurrentContext,name:string,description:string,data:number[])=>invoke("upload_accessory_craft",{context,name,description,data});

export const getPetMailStatus=(context:CurrentContext)=>invoke<{travelId:string|null;unread:number}>("get_pet_mail_status",{...context});
