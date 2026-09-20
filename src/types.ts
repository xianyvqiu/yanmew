/** SQLite ID 通过字符串传递，避免 JavaScript 大整数精度损失。 */
export type RecordId = string;
export type AppError = { code: string; message: string; field?: string };
export type SecretUpdate = { action: "keep" } | { action: "clear" } | { action: "set"; value: string };
export type WeatherSettings = { apiHost: string | null; hasApiKey: boolean };
export type WeatherInput = { apiHost: string | null; apiKey?: SecretUpdate };
export type Provider = {
  id: RecordId; name: string; baseUrl: string | null; model: string;
  hasApiKey: boolean; isActive: boolean; memoryIsActive: boolean;
};
export type ProviderInput = { name: string; baseUrl: string | null; model: string; apiKey?: SecretUpdate };
export type Persona = {
  id: RecordId; name: string; prompt: string; initialPrompt: string;
  selfEvolveEnabled: boolean; lastEvolvedAt: string | null; color: string;
  chatBgFile: string | null; chatBgOpacity: number; chatBgBlur: number;
  isActive: boolean; isBuiltin: boolean; expressions: Record<string, unknown> | null;
};
export type PersonaInput = Pick<Persona, "name" | "prompt" | "selfEvolveEnabled" | "color" | "chatBgOpacity" | "chatBgBlur"> & { saveAsInitial?: boolean };
export type ExpressionLibrary = { click: { happy: string[]; surprised: string[]; shy: string[] }; hungry: string[]; eating: string[]; sleepy: string[]; bored: string[]; thinking: string[] };
export type CurrentContext = { personaId: RecordId; sessionId: RecordId | null; contextRevision: number };
export type NumericSettingKey =
  | "hungry_minutes" | "bored_minutes" | "sleep_idle_minutes" | "sleep_min_minutes" | "sleep_max_minutes"
  | "short_memory_min_chars" | "short_memory_max_chars" | "long_memory_min_chars" | "long_memory_max_chars"
  | "memory_merge_count" | "recent_message_count" | "recent_long_memory_count"
  | "travel_journal_min_chars" | "travel_journal_max_chars" | "tool_call_rounds" | "notebook_max_chars"
  | "intimacy_threshold_1" | "intimacy_threshold_2" | "intimacy_threshold_3" | "intimacy_threshold_4"
  | "accessory_cost_1" | "accessory_cost_2" | "accessory_cost_3" | "accessory_cost_4"
  | "sign_in_min_coins" | "sign_in_max_coins" | "travel_min_online_hours" | "travel_max_online_hours"
  | "travel_min_duration_minutes" | "travel_max_duration_minutes" | "travel_accessory_chance_percent"
  | "letter_reply_min_minutes" | "letter_reply_max_minutes";
/** 持久化数值以规范十进制字符串传输，由 Rust 校验范围和组合。 */
export type Settings = Record<NumericSettingKey, string> & { theme_mode: "system" | "light" | "dark" };
