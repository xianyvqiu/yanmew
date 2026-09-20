import { invoke } from "@tauri-apps/api/core";
import type { ExpressionLibrary } from "./types";
import type { CurrentContext, Persona, PersonaInput, Provider, ProviderInput, RecordId, Settings, WeatherInput, WeatherSettings } from "./types";

export const configurationApi = {
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

export function errorMessage(error: unknown): string {
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message;
  return typeof error === "string" ? error : "操作失败，请重试";
}
