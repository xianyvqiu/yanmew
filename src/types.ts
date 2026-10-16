/** SQLite ID 通过字符串传递，避免 JavaScript 大整数精度损失。 */
export type RecordId = string;
export type PluginJson = null | boolean | number | string | PluginJson[] | { [key: string]: PluginJson };
export type GamePermissionChoice = { permission: "wallet.modify" | "network.fetch"; scope: string; decision: "pending" | "allowed" | "denied" };
export type GamePermission = GamePermissionChoice & { pluginVersion: string; decidedAt: string | null };
export type GamePlugin = {
  pluginId: string; name: string; description: string; author: string; installedVersion: string; apiVersion: string;
  storageVersion: number; packagePath: string | null; packageSha256: string; repository: string | null; homepage: string | null;
  releasePage: string | null; networkOrigins: string[]; isBundled: boolean; packageStatus: "available" | "missing_package";
  isEnabled: boolean; installedAt: string; updatedAt: string;
};
export type PluginSetting = { key: string; title: string; description: string; type: "boolean" | "integer" | "number" | "string" | "enum"; default: PluginJson; required: boolean; minimum: number | null; maximum: number | null; options: {value: PluginJson; label: string}[] };
export type GameManifest = { id: string; name: string; description: string; author: string; version: string; api_version: "1.0.0"; entry: string; storage_version: number; permissions: string[]; network_origins: string[]; settings_schema: PluginSetting[]; repository: string | null; homepage: string | null; release_page: string | null; migration: string | null };
export type PluginUsage = { storage_version: number; settings: Record<string, PluginJson>; data: Record<string, PluginJson> };
export type GamePluginDetail = { plugin: GamePlugin; manifest: GameManifest | null; permissions: GamePermission[]; usage: PluginUsage };
export type GameInstallPreview = { token: string; manifest: GameManifest; installed: GamePlugin | null; permissions: GamePermission[]; migrationRequired: boolean };
/** 一次打开到关闭的运行；长期游戏进度由 PluginUsage.data 独立保存。 */
export type GameRun = { id: string; pluginId: string; pluginName: string; pluginVersion: string; walletPermissionGranted: boolean; status: "running" | "completed" | "cancelled" | "error"; outcome: string | null; details: PluginJson; coinDelta: string; startedAt: string; endedAt: string | null };
export type GamePluginLedger = { id: string; gameRunId: string; pluginId: string; pluginName: string; pluginVersion: string; delta: string; reason: string; requestId: string; createdAt: string };
/** 全局喵币为 SQLite 64 位整数；前端按十进制字符串展示，不参与记账。 */
export type Wallet = { coins: string; updatedAt: string };
/** 日期与是否领取由后端按电脑当前时区判断，页面计时器只能刷新查询。 */
export type SignInStatus = {
  wallet: Wallet; localDate: string; signedIn: boolean; rewardMin: string; rewardMax: string;
  intimacyThresholds: [number, number, number, number];
  festivals: { id: RecordId; scope: "catalog" | "custom"; name: string; blessing: string; isMemorial: boolean }[];
};
export type DailySignInResult = { status: SignInStatus; grantedCoins: string | null };
export type SessionIntimacy = { personaId: RecordId; sessionId: RecordId; contextRevision: number; intimacy: number; maximum: number };
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
  webActivity?: "searching" | "reading" | null;
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
  | { type: "web_activity"; phase: "searching" | "reading" | null }
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
  activity: "normal" | "standby" | "hungry" | "bored" | "sleeping" | "eating" | "interacting" | "thinking" | "speaking" | "away" | "dormant";
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
export type PersonaUpdatedNotice = {
  personaId: RecordId;
  sessionId: RecordId;
  contextRevision: number;
  evolvedAt: string;
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
  purposes?:string[];
  createdAt?:string|null;updatedAt?:string|null;
  id: RecordId; name: string; baseUrl: string | null; model: string;
  hasApiKey: boolean; isActive: boolean; memoryIsActive: boolean;
  maxOutputTokens: number; contextWindowOverride: number | null;
  capacity: { contextWindow: number | null; maxOutputTokens: number | null; source: string; estimated: boolean; reference?: string | null; checkedAt?: string | null };
};
export type EmbeddingProviderInput = { name: string; baseUrl: string | null; model: string; apiKey?: SecretUpdate };
export type ProviderInput = {name:string;baseUrl:string|null;apiKey?:SecretUpdate;model?:string;maxOutputTokens?:number;contextWindowOverride?:number|null};
export type Persona = {
  createdAt?:string|null;updatedAt?:string|null;
  id: RecordId; name: string; prompt: string; initialPrompt: string;
  selfEvolveEnabled: boolean; lastEvolvedAt: string | null; color: string;
  chatBgFile: string | null; chatBgOpacity: number; chatBgBlur: number;
  chatAppearance?: ChatAppearance;
  isActive: boolean; isBuiltin: boolean; expressions: Record<string, unknown> | null;
};
export type PersonaInput = Pick<Persona, "name" | "prompt" | "selfEvolveEnabled" | "color" | "chatBgOpacity" | "chatBgBlur"> & { saveAsInitial?: boolean; initialPrompt?: string };
export type ChatAppearance = { bubbleStyle: "solid" | "transparent" | "outline" | "frosted"; userTextColor: string | null; assistantTextColor: string | null };
export type ChatBackgroundRecord = { personaId: RecordId; file: string | null; opacity: number; blur: number; appearance?: ChatAppearance };
export type ChatBackgroundView = { record: ChatBackgroundRecord; imageDataUrl: string | null; imageError: AppError | null };
export type ChatBackgroundInput = {
  expected: ChatBackgroundRecord; opacity: number; blur: number;
  image: { fileName: string; base64: string } | null; reset: boolean;
  appearance?: ChatAppearance;
};
export type ExpressionLibrary = { click: { happy: string[]; surprised: string[]; shy: string[] }; hungry: string[]; eating: string[]; sleepy: string[]; bored: string[]; thinking: string[]; standby: string[] };
export type CurrentContext = { personaId: RecordId; sessionId: RecordId | null; contextRevision: number };
export type UserProfile = { id: RecordId; name: string; content: string; createdAt: string; updatedAt: string };
export type Session = {
  id: RecordId; personaId: RecordId; userProfileId: RecordId | null; title: string;
  memoryRevision: number; intimacy: number; createdAt: string; updatedAt: string; isCurrent: boolean; isPinned: boolean;
};
/** 删除提交后的权威选择；null 表示当前人设没有可用会话，不隐式创建。 */
export type SessionDeleteResult = { currentSessionId: RecordId | null; contextRevision: number };
export type Message = {
  images?:ChatImageRecord[];
  id: RecordId; sessionId: RecordId; roundId: RecordId | null; role: "user" | "assistant";
  content: string; expression: string | null; createdAt: string;
};
export type MessagePage = { items: Message[]; nextAfterId: RecordId | null };
export type NumericSettingKey =
  | "hungry_minutes" | "standby_minutes" | "idle_event_minutes" | "sleep_min_minutes" | "sleep_max_minutes"
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
export type WebSearchKind = "tavily" | "bocha" | "custom";
export type WebSearchProvider = { id: string; name: string; kind: WebSearchKind; baseUrl: string; hasApiKey: boolean; isActive: boolean };
export type WebSearchSettings = { enabled: boolean; providers: WebSearchProvider[] };
export type WebSearchProviderInput = { name: string; kind: WebSearchKind; baseUrl: string; apiKey: { action: "keep" | "clear" } | { action: "set"; value: string } };

export type LetterKind = "user" | "midway" | "return" | "reply";
export interface Letter {
  id: RecordId; sessionId: RecordId; travelId: RecordId | null;
  sender: "user" | "ai"; content: string; replyToId: RecordId | null;
  hasBeenReplied: boolean; replyDueAt: string | null;
  replyState: "none" | "pending" | "completed" | "cancelled";
  letterKind: LetterKind; isFavorite: boolean; isRead: boolean; createdAt: string;
}
export interface SendLetterInput { travelId: RecordId; content: string; replyToId: RecordId | null }
export interface LetterTask { travelId: RecordId; letterId: RecordId | null; kind: LetterKind }
export interface LetterGenerationStatus {
  notice: string | null;
  active: LetterTask | null;
  failures: { task: LetterTask; error: AppError; retryAfterSeconds: number }[];
}

export type TravelTaskState = "pending" | "done" | "cancelled";
export type Travel = {
  id: RecordId; sessionId: RecordId; status: "away" | "returned" | "cancelled";
  startedAt: string; plannedReturnAt: string; actualReturnAt: string | null; cancelledAt: string | null;
  midwayLetterAt: string; destination: string | null; factsState: "pending" | "ready" | "cancelled";
  midwayLetterState: TravelTaskState; returnLetterState: TravelTaskState; giftState: TravelTaskState;
  journalState: TravelTaskState; giftKind: "text" | "accessory" | null;
};
export type TravelGenerationStatus = { active: RecordId | null; failures: {travelId: RecordId; error: AppError; retryAfterSeconds: number}[] };
export type LetterTarget = CurrentContext & { travelId: RecordId; replyToId: RecordId | null };

/** 会话级饰品；显示字段始终为图标字符、Emoji、颜文字或符号组合。 */
export type Accessory = {
  id: RecordId; sessionId: RecordId; name: string; displayText: string; imageFile: string | null;
  description: string; cost: string; source: "crafted" | "travel"; travelId: RecordId | null;
  isWorn: boolean; createdAt: string; positionX?: number | null; positionY?: number | null; scale?: number;
};
export type AccessoryImage = { dataUrl: string | null; notice: string | null };
export type AccessoryDeleteResult = { deletedId: RecordId };
export type AccessorySellResult = AccessoryDeleteResult & { wallet: Wallet };

/** 生图专用配置；只有hasApiKey，不能取得已保存的密钥。 */
export type ImageGenerationProvider = { id: RecordId; name: string; protocol: "openai-compatible"; baseUrl: string; model: string; hasApiKey: boolean };
export type ImageGenerationConfig = { version: 1; revision: number; nextId: RecordId; enabled: boolean; activeId: RecordId | null; providers: ImageGenerationProvider[] };
export type ImageGenerationProviderInput = { name: string; protocol: "openai-compatible"; baseUrl: string; model: string; apiKey: ProviderInput["apiKey"] };
export type AccessoryCraftPreview = {
  previewId: string; context: { personaId: RecordId; sessionId: RecordId; contextRevision: number }; tier: number; cost: string;
  name: string; displayText: string; description: string; status: "preparing" | "ready"; appearance: "image" | "text";
  imageDataUrl: string | null; notice: string | null;
};

/** 普通文字伴手礼，旅行饰品在Accessory表和管理页中展示。 */
export type Souvenir = { id:RecordId;sessionId:RecordId;travelId:RecordId|null;content:string;note:string;isFavorite:boolean;createdAt:string };
export type GiftGenerationStatus = { active:RecordId|null;pendingCount:number;failures:TravelGenerationStatus["failures"];notice:string|null };

export type TravelJournalSummary = { id:RecordId;sessionId:RecordId;travelId:RecordId;title:string;createdAt:string;updatedAt:string };
export type TravelJournal = TravelJournalSummary & {content:string};
export type TravelJournalExpected = Pick<TravelJournal,"title"|"content"|"updatedAt">;
export type TravelJournalDeleted = {deletedId:RecordId;sessionId:RecordId;travelId:RecordId};
export type JournalGenerationStatus = {active:RecordId|null;pendingCount:number;failures:TravelGenerationStatus["failures"];notice:string|null};
/** ZIP逻辑备份协议；manifest/data保留协议字段命名，不直接作为业务写入DTO。 */
export type BackupKind = "full" | "session" | "user_profile" | "persona_profile" | "persona_full" | "plugin_data";
export type BackupRequest = { kind: BackupKind; ids: string[]; includeSessions: boolean };
export type BackupWarning = { code: string; table: string; record_id: string; message: string };
export type BackupManifest = {
  format_version: number; schema_version: number; app_version: string; package_type: BackupKind;
  exported_at: string; selection: BackupRequest; record_counts: Record<string, number>;
  data_size: number; data_sha256: string; files: { path: string; size: number; sha256: string }[];
  warnings: BackupWarning[];
};
/** 系统窗口已保存正式文件；仅返回摘要，不传输ZIP内容。 */
export type BackupArchive = { filename: string; size: number; sha256: string; manifest: BackupManifest };

export type BackupExportOption = {id:string;name:string};
export type BackupExportOptions = {personas:BackupExportOption[];sessions:BackupExportOption[];userProfiles:BackupExportOption[];plugins:BackupExportOption[]};

/** 导入预览只包含对象摘要；备份文件与完整历史由后端逐条读取。 */
export type BackupImportCandidate = { id: string; name: string; stamp: string; different: boolean };
export type BackupImportNode = { sourceId: string; name: string; isBuiltin: boolean; candidates: BackupImportCandidate[]; suggestedTargetId: string | null; requiresChoice: boolean };
export type BackupImportPreview = { previewId: string; kind: BackupKind; exportedAt: string; counts: Record<string, number>; warnings: string[]; personas: BackupImportNode[]; profiles: BackupImportNode[]; plugins: string[] };
export type BackupImportBinding = { sourceId: string; targetId: string | null };
export type BackupImportConfirm = { previewId: string; personas: BackupImportBinding[]; profiles: BackupImportBinding[]; confirmGlobalRestore: boolean };
export type BackupImported = { sessions: number; personasCreated: number; personasReused: number; warnings: string[] };

/** 会话自己的年度月日；新增不复制或关联母表。 */
export type CustomFestivalInput = { kind: "festival" | "birthday" | "anniversary" | "custom"; name: string; calendarType: "solar" | "lunar"; month: number; day: number; isLeapMonth: boolean; blessing: string; note: string | null };
export type FestivalCatalogRecord = { id: RecordId; code: string; name: string; year: number; calendarType: "solar" | "lunar" | "solar_term"; solarMonth: number | null; solarDay: number | null; lunarMonth: number | null; lunarDay: number | null; isLeapMonth: boolean; solarTerm: string | null; blessing: string; isMemorial: boolean; note: string | null };
export type CustomFestivalRecord = { id: RecordId; festivalId: RecordId | null; kind: CustomFestivalInput["kind"]; name: string; year: number; calendarType: "solar" | "lunar"; solarMonth: number | null; solarDay: number | null; lunarMonth: number | null; lunarDay: number | null; isLeapMonth: boolean; blessing: string; note: string | null; source: "ai" | "user"; createdAt: string; updatedAt: string; isMemorial: boolean; revisionToken: string; occurrences: string[] };

/** 只用于宠窗显示；原提醒详情与状态不被润色覆盖。 */
export type ReminderPresentation = { occurrenceId: RecordId; personaId: RecordId; contextRevision: number; text: string | null };

export type CalendarDay = { date: string; lunar: string; solarTerm: string | null; events: {id:string;scope:"catalog"|"custom";name:string;blessing:string;isMemorial:boolean}[] };
export type CalendarMonth = {year:number;month:number;days:CalendarDay[]};

export type ProviderRole={purpose:"chat"|"memory"|"image"|"vision";providerId:RecordId;model:string;enabled:boolean};
export type ProviderModelConfig=Pick<ProviderRole,"purpose"|"providerId"|"model">&{maxOutputTokens:number;contextWindowOverride:number|null};
export type ProviderRoles={revision:number;roles:ProviderRole[];models?:ProviderModelConfig[]};

export type ChatImageInput={filename:string;dataUrl:string};
export type ChatImageRecord={id:RecordId;filename:string;imageFile:string;mediaType:string;caption:string|null};

/** SMTP连接与模型提供商独立，授权码不从后端回传。 */
export interface ReminderEmailConnection { email: string; host: string; port: number; encryption: "tls" | "starttls"; username: string; from: string; to: string }
export interface ReminderEmailDelivery { id: string; recipient: string; subject: string; state: "pending" | "sending" | "retry_wait" | "sent" | "failed" | "uncertain" | "cancelled"; attempts: number; nextRetryAt: string | null; error: string | null; createdAt: string; sentAt: string | null }
export interface ReminderEmailView { connection: ReminderEmailConnection; revision: number; enabled: boolean; verified: boolean; hasSecret: boolean; deliveries: ReminderEmailDelivery[] }
export interface ReminderEmailInput { connection: ReminderEmailConnection; revision: number; secret: { action: "keep" | "clear" } | { action: "set"; value: string } }

export type PetArtCategory = "normal"|"standby"|"click"|"hungry"|"eating"|"sleepy"|"bored"|"thinking"|"chat";
export type PetArtworkInput = {kind:"base"|"expression";name:string;prompt:string;description:string;category:PetArtCategory;enabled:boolean};
export type PetArtwork = PetArtworkInput & {id:string;personaId:string;imageFile:string|null;previewFile:string|null;revision:number;createdAt:string;updatedAt:string};
export type PetCreation = {enabled:boolean;revision:number;items:PetArtwork[]};
