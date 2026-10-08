import { invoke } from "@tauri-apps/api/core";
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
  edit: (context: CurrentContext, message: Message, content: string) => invoke<Message>("update_chat_message", { context, messageId: message.id, expectedContent: message.content, content }),
  deleteTail: (context: CurrentContext, message: Message) => invoke<void>("delete_tail_message", { context, messageId: message.id, expectedContent: message.content }),
  regenerate: (context: CurrentContext, roundId: RecordId) => invoke<ChatRequestSnapshot>("regenerate_last_reply", { context, roundId }),
  listenMessages: (receive: (change: { sessionId: RecordId }) => void) => listen<{ sessionId: RecordId }>("chat-messages-changed", event => receive(event.payload)),
  submit: (context: CurrentContext, input: string) => invoke<ChatRequestSnapshot>("submit_chat", {
    personaId: context.personaId, sessionId: context.sessionId, contextRevision: context.contextRevision, input,
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
  get: () => invoke<WeatherView>("get_weather"),
  refresh: (contextRevision: number) => invoke<WeatherView>("refresh_weather", { contextRevision }),
  cancel: () => invoke<void>("cancel_weather_refresh"),
};
import type { Reminder, ReminderInput, MissedReminderSummary, ReminderDeliveryState, ReminderOccurrence, ReminderOccurrencePage, ReminderStatus } from "./types";

export const reminderApi = {
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
  testProvider: (id: RecordId) => invoke<void>("test_provider_connection", { id }),
  createProvider: (input: ProviderInput) => invoke<Provider>("create_provider", { input }),
  updateProvider: (id: RecordId, input: ProviderInput) => invoke<Provider>("update_provider", { id, input }),
  deleteProvider: (id: RecordId) => invoke<void>("delete_provider", { id }),
  setChatProvider: (id: RecordId) => invoke<void>("set_active_chat_provider", { id }),
  setMemoryProvider: (id: RecordId | null) => invoke<void>("set_memory_provider", { id }),
  listPersonas: () => invoke<Persona[]>("list_personas"),
  createPersona: (input: PersonaInput) => invoke<Persona>("create_persona", { input }),
  generateExpressions: (id: RecordId) => invoke<ExpressionLibrary>("generate_expressions", { id }),
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
  delete: (context: CurrentContext, sessionId: RecordId) => invoke<SessionDeleteResult>("delete_session", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision,
  }),
  messages: (context: CurrentContext, afterId: RecordId | null = null, limit = 100) => invoke<MessagePage>("list_messages", {
    personaId: context.personaId, sessionId: context.sessionId,
    contextRevision: context.contextRevision, afterId, limit,
  }),
};

export function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message;
  return typeof error === "string" ? error : "操作失败，请重试";
}
