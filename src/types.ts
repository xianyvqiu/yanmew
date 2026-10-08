/** SQLite ID 通过字符串传递，避免 JavaScript 大整数精度损失。 */
export type RecordId = string;
/** 记忆记录属于会话；以下契约由后续记忆仓储和管理页面使用。 */
export type MemoryIntegrationState = "pending" | "integrated";
export type MemoryKind = "short" | "long";
export type MemoryRecord = {
  id: RecordId; sessionId: RecordId; kind: MemoryKind; roundId: RecordId | null;
  integrationState: MemoryIntegrationState | null; title: string | null; content: string;
  sourceCount: number | null; activatedByRoundId: RecordId | null; coveredThroughRoundId: RecordId | null;
  coveredThroughMessageId: RecordId | null;
  createdAt: string; updatedAt: string;
};
export type MemorySource = {
  shortMemoryId: RecordId; roundId: RecordId; sourceOrder: number; content: string; updatedAt: string;
};
export type MemoryDetail = {
  record: MemoryRecord; memoryRevision: number; sources: MemorySource[];
  relatedLongMemories: { id: RecordId; title: string }[];
};
export type MemoryPage = { items: MemoryRecord[]; nextAfterId: RecordId | null };
export type MemoryPreview = { previewId: string; detail: MemoryDetail; title: string | null; content: string };
export type MemoryGenerationView = {
  sessionId: RecordId; completedCount: number; pendingCount: number;
  activeRoundId: RecordId | null; failedRoundId: RecordId | null;
  error: AppError | null; autoRetryPending: boolean;
  longCompletedCount: number; longPendingCount: number;
  longActiveRoundId: RecordId | null; longFailedRoundId: RecordId | null;
  longError: AppError | null; longAutoRetryPending: boolean;
};
export type ShortMemory = {
  id: RecordId; sessionId: RecordId; roundId: RecordId; content: string;
  integrationState: MemoryIntegrationState; createdAt: string; updatedAt: string;
};
export type LongMemory = {
  id: RecordId; sessionId: RecordId; title: string; content: string;
  sourceCount: number; activatedByRoundId: RecordId; createdAt: string; updatedAt: string;
};
/** 覆盖边界从按 sourceOrder 排序的最后一项推导，不额外保存消息边界。 */
export type LongMemorySource = { longMemoryId: RecordId; shortMemoryId: RecordId; sourceOrder: number };
/** 一个会话一份完整正文，建议字数上限不截断内容。 */
export type Notebook = { sessionId: RecordId; content: string; updatedAt: string };
export type NotebookView = { notebook: Notebook | null; limitChars: number };
export type ChatRound = {
  id: RecordId; sessionId: RecordId; status: "pending" | "completed" | "interrupted";
  createdAt: string; completedAt: string | null; stopReason: string | null;
};
export type RoundSnapshot = { round: ChatRound; userMessage: Message; assistantMessage: Message | null };
export type ChatContextBudget = {
  contextWindow: number | null; configuredMaxOutputTokens: number; effectiveMaxOutputTokens: number;
  estimatedInputTokens: number; capacitySource: string; estimated: boolean; reduced: boolean;
};
/** partialContent 包含续写时已保存的前缀及未保存增量；round 中的消息是数据库记录。 */
export type ChatRequestSnapshot = {
  regenerating?: boolean;
  requestId: string; personaId: RecordId; sessionId: RecordId; contextRevision: number;
  roundId: RecordId; seq: number; partialContent: string; expression: string | null; round: RoundSnapshot;
  notice?: string | null; budget?: ChatContextBudget | null;
};
export type ChatReplyEvent = {
  requestId: string; personaId: RecordId; sessionId: RecordId; contextRevision: number;
  roundId: RecordId; seq: number; saved: RoundSnapshot | null;
} & (
  | { type: "expression"; expression: string }
  | { type: "text"; text: string }
  | { type: "context_notice"; message: string; budget: ChatContextBudget }
  | { type: "finished"; finish: "stop" | "length" | "content_filter" | "tool_calls" }
  | { type: "cancelled" }
  | { type: "failed"; code: string; message: string }
);
export type ReminderStatus = "pending" | "done" | "expired";
export type ReminderDeliveryState = "queued" | "missed" | "shown";
export type ReminderOccurrence = {
  id: RecordId; reminderId: RecordId; name: string; content: string; scheduledAt: string;
  status: ReminderStatus; deliveryState: ReminderDeliveryState;
  completedAt: string | null; createdAt: string;
};
export type ReminderOccurrencePage = { items: ReminderOccurrence[]; nextAfterId: RecordId | null };
export type PetActivity = {
  personaId: RecordId; sessionId: RecordId | null; contextRevision: number; revision: number;
  activity: "normal" | "hungry" | "bored" | "sleeping" | "eating" | "interacting" | "thinking" | "speaking" | "away" | "dormant";
  expression: string; intimacy: number | null; sleepUntil: string | null; error: string | null;
  canInteract: boolean; canWander: boolean;
};
export type PetAction = "click" | "stroke" | "feed";
export type PetWanderPoint = { x: number; y: number };
export type PetWanderLayout = {
  personaId: RecordId; sessionId: RecordId; contextRevision: number; layoutRevision: number;
  area: PetWanderPoint & { width: number; height: number };
  body: PetWanderPoint & { width: number; height: number };
  offset: PetWanderPoint; paused: boolean;
};
export type PetWanderTarget = Pick<PetWanderLayout, "personaId" | "sessionId" | "contextRevision" | "layoutRevision" | "offset">;
export type PetStateNotice = {
  personaId: RecordId; sessionId: RecordId; contextRevision: number; revision: number;
  activity: "hungry" | "bored"; text: string;
};
export type ProactiveMessageNotice = {
  personaId: RecordId; sessionId: RecordId; contextRevision: number; message: Message;
};
export type AppError = { code: string; message: string; field?: string };
export type SecretUpdate = { action: "keep" } | { action: "clear" } | { action: "set"; value: string };
export type WeatherSettings = { apiHost: string | null; hasApiKey: boolean };
export type WeatherView = {
  contextRevision: number;
  availability: "available" | "cached" | "stale" | "unavailable";
  data: {
    position: { latitude: number; longitude: number; locatedAt: string };
    observation: { observedAt: string; description: string; temperatureCelsius: number | null; source: "openMeteo" | "qweather" };
    fetchedAt: string;
  } | null;
  error: { code: string; message: string; field?: string } | null;
};
export type WeatherInput = { apiHost: string | null; apiKey?: SecretUpdate };
export type Provider = {
  id: RecordId; name: string; baseUrl: string | null; model: string;
  hasApiKey: boolean; isActive: boolean; memoryIsActive: boolean;
  maxOutputTokens: number; contextWindowOverride: number | null;
  capacity: { contextWindow: number | null; maxOutputTokens: number | null; source: string; estimated: boolean; reference?: string | null; checkedAt?: string | null };
};
export type EmbeddingProviderInput = { name: string; baseUrl: string | null; model: string; apiKey?: SecretUpdate };
export type ProviderInput = EmbeddingProviderInput & { maxOutputTokens?: number; contextWindowOverride?: number | null };
export type Persona = {
  id: RecordId; name: string; prompt: string; initialPrompt: string;
  selfEvolveEnabled: boolean; lastEvolvedAt: string | null; color: string;
  chatBgFile: string | null; chatBgOpacity: number; chatBgBlur: number;
  isActive: boolean; isBuiltin: boolean; expressions: Record<string, unknown> | null;
};
export type PersonaInput = Pick<Persona, "name" | "prompt" | "selfEvolveEnabled" | "color" | "chatBgOpacity" | "chatBgBlur"> & { saveAsInitial?: boolean };
export type ExpressionLibrary = { click: { happy: string[]; surprised: string[]; shy: string[] }; hungry: string[]; eating: string[]; sleepy: string[]; bored: string[]; thinking: string[] };
export type CurrentContext = { personaId: RecordId; sessionId: RecordId | null; contextRevision: number };
export type UserProfile = { id: RecordId; name: string; content: string; createdAt: string; updatedAt: string };
export type Session = {
  id: RecordId; personaId: RecordId; userProfileId: RecordId | null; title: string;
  memoryRevision: number; intimacy: number; createdAt: string; updatedAt: string; isCurrent: boolean;
};
/** 删除提交后的权威选择；null 表示当前人设没有可用会话，不隐式创建。 */
export type SessionDeleteResult = { currentSessionId: RecordId | null; contextRevision: number };
export type Message = {
  id: RecordId; sessionId: RecordId; roundId: RecordId | null; role: "user" | "assistant";
  content: string; expression: string | null; createdAt: string;
};
export type MessagePage = { items: Message[]; nextAfterId: RecordId | null };
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
/** 循环规则中的 next_local_at 由后端维护；保存时不作为用户设置采纳。 */
export interface ReminderSchedule {
  version: 1;
  repeat: "daily" | "weekly" | "monthly";
  time: string;
  weekdays?: number[];
  days?: number[];
  next_local_at?: string;
}
export interface ReminderInput {
  name: string;
  content: string;
  schedule: ReminderSchedule | null;
  fireAt: string | null;
}
export interface Reminder extends ReminderInput {
  id: string;
  kind: "once" | "daily" | "weekly" | "monthly";
  status: ReminderStatus;
  source: "manual" | "chat";
  createdAt: string;
}
export interface MissedReminderSummary {
  occurrences: ReminderOccurrence[];
  nextAfterId: string | null;
  text: string;
}
export type EmbeddingProvider = {
  id: RecordId;
  name: string;
  kind: "local" | "online";
  baseUrl: string | null;
  model: string;
  hasApiKey: boolean;
  isBuiltin: boolean;
  isActive: boolean;
};
export type EmbeddingStatus = {
  total: number;
  ready: number;
  pending: number;
  unavailable: number;
  running: boolean;
  error: AppError | null;
};
export type AutostartStatus = {
  supported: boolean; enabled: boolean; needsRepair: boolean; blockedBySystem: boolean;
};
