import { invoke } from "@tauri-apps/api/core";
import type { ChatReplyEvent, ChatRequestSnapshot, RoundSnapshot } from "./types";

export const chatApi = {
  submit: (context: CurrentContext, input: string) => invoke<ChatRequestSnapshot>("submit_chat", {
    personaId: context.personaId, sessionId: context.sessionId, contextRevision: context.contextRevision, input,
  }),
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
import type { CurrentContext, MessagePage, Persona, PersonaInput, Provider, ProviderInput, RecordId, Session, Settings, WeatherInput, WeatherSettings } from "./types";

export const configurationApi = {
  listenContext: (receive: (context: CurrentContext) => void) => listen<CurrentContext>("window_context_changed", event => receive(event.payload)),
  listenChanges: (receive: () => void) => listen("configuration_changed", receive),
  status: () => invoke<void>("get_configuration_status"),
  getCurrentContext: () => invoke<CurrentContext>("get_current_context"),
  getWeatherSettings: () => invoke<WeatherSettings>("get_weather_settings"),
  saveWeatherSettings: (input: WeatherInput) => invoke<WeatherSettings>("save_weather_settings", { input }),
  getSettings: () => invoke<Settings>("get_settings"),
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
  get: () => invoke<PetActivity>("get_pet_activity"),
  interact: (view: PetActivity, action: PetAction) => invoke<PetActivity>("interact_pet", { input: {
    personaId: view.personaId, sessionId: view.sessionId, contextRevision: view.contextRevision, action,
  } }),
  dragging: (dragging: boolean) => invoke<void>("set_pet_dragging", { dragging }),
  refreshProvider: () => invoke<void>("refresh_pet_provider"),
  listen: (receive: (view: PetActivity) => void) => listen<PetActivity>("pet-activity-changed", event => receive(event.payload)),
  listenError: (receive: (error: unknown) => void) => listen("pet-activity-error", event => receive(event.payload)),
};

export const sessionApi = {
  list: (context: CurrentContext) => invoke<Session[]>("list_sessions", {
    personaId: context.personaId, contextRevision: context.contextRevision,
  }),
  create: (context: CurrentContext) => invoke<Session>("create_session", {
    personaId: context.personaId, contextRevision: context.contextRevision,
  }),
  select: (context: CurrentContext, sessionId: RecordId) => invoke<CurrentContext>("set_active_session", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision,
  }),
  rename: (context: CurrentContext, sessionId: RecordId, title: string) => invoke<Session>("rename_session", {
    personaId: context.personaId, sessionId, contextRevision: context.contextRevision, title,
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
