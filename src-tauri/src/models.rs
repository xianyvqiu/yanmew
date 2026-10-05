use serde::Serialize;

/// 删除事务提交后的权威当前选择；没有剩余会话时为 None。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDeleteResult {
    pub current_session_id: Option<String>,
    /// 仅删除当前会话时递增，不相关删除保留当前生成任务。
    pub context_revision: u64,
}

/// 短期记忆的整合状态，不表示 AI 生成任务状态。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryIntegrationState {
    /// 已生成短期记忆，尚未被长期记忆引用。
    Pending,
    /// 已通过 long_memory_sources 归入长期记忆，短期内容仍保留。
    Integrated,
}

impl MemoryIntegrationState {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "integrated" => Some(Self::Integrated),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Integrated => "integrated",
        }
    }
}

/// 一条完整聊天回合的事实概括；SQLite ID 对外统一为字符串。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortMemory {
    /// 短期记忆主键。
    pub id: String,
    /// 所属会话，后续仓储读写必须验证归属。
    pub session_id: String,
    /// 唯一来源完整回合，关联该轮用户和 AI 消息。
    pub round_id: String,
    /// 中性事实叙事正文。
    pub content: String,
    /// pending 或 integrated；已整合记录不因整合而删除。
    pub integration_state: MemoryIntegrationState,
    /// 首次生成时间，UTC RFC3339。
    pub created_at: String,
    /// 最近修改时间，UTC RFC3339。
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LongMemory {
    /// 长期记忆主键。
    pub id: String,
    /// 所属会话。
    pub session_id: String,
    /// 同次整合生成的简短中性标题，不计入正文字数。
    pub title: String,
    /// 来源短期记忆的中性事实整合正文。
    pub content: String,
    /// 本次实际使用的来源数量，不随全局 k 修改而变化。
    pub source_count: i64,
    /// 触发本次整合的第 k+1 回合；不属于本条长期记忆的来源。
    pub activated_by_round_id: String,
    /// 首次生成时间，UTC RFC3339。
    pub created_at: String,
    /// 最近修改时间，UTC RFC3339。
    pub updated_at: String,
}

/// 实际覆盖边界取 source_order 最大项的短期记忆及其来源回合。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LongMemorySource {
    /// 目标长期记忆主键。
    pub long_memory_id: String,
    /// 来源短期记忆主键；同一短期记忆最多整合一次。
    pub short_memory_id: String,
    /// 本次整合来源顺序，从 1 开始，按来源回合排序。
    pub source_order: i64,
}

/// 仅供后端检索的可重建缓存，不作为前端 DTO 或逻辑备份内容。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LongMemoryEmbedding {
    /// 对应长期记忆主键。
    pub long_memory_id: i64,
    /// 会话过滤用归属 ID。
    pub session_id: i64,
    /// 提供商类型、地址、模型及维度组合标识。
    pub model_key: String,
    /// 向量维度，必须为正整数。
    pub dimensions: i64,
    /// 归一化 float32 的原始 BLOB，字节数等于 dimensions × 4。
    pub vector: Vec<u8>,
    /// 标题与正文内容哈希，用于识别过期向量。
    pub content_hash: String,
    /// 最近生成时间，UTC RFC3339。
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    /// 会话即主键，一个会话一份记事本。
    pub session_id: String,
    /// 完整正文；建议字数上限是软限制，不截断。
    pub content: String,
    /// 最近更新时间，RFC3339。
    pub updated_at: String,
}

#[cfg(test)]
mod memory_model_tests {
    use super::*;

    #[test]
    fn memory_contract_preserves_ids_and_integration_states() {
        for state in [
            MemoryIntegrationState::Pending,
            MemoryIntegrationState::Integrated,
        ] {
            assert_eq!(MemoryIntegrationState::parse(state.as_str()), Some(state));
            assert_eq!(serde_json::to_value(state).unwrap(), state.as_str());
        }
        assert_eq!(MemoryIntegrationState::parse("completed"), None);
        assert!(serde_json::from_str::<MemoryIntegrationState>("\"failed\"").is_err());
        let memory = ShortMemory {
            id: "9007199254740993".into(),
            session_id: "1".into(),
            round_id: "2".into(),
            content: "用户提到明天出门。".into(),
            integration_state: MemoryIntegrationState::Integrated,
            created_at: "2026-09-30T00:00:00Z".into(),
            updated_at: "2026-09-30T00:00:00Z".into(),
        };
        let value = serde_json::to_value(memory).unwrap();
        assert_eq!(value["id"], "9007199254740993");
        assert_eq!(value["roundId"], "2");
        assert_eq!(value["integrationState"], "integrated");
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub schedule: Option<crate::reminders::schedule::Schedule>,
    pub content: String,
    pub status: ReminderStatus,
    pub fire_at: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderStatus {
    Pending,
    Done,
    Expired,
}

impl ReminderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Done => "done",
            Self::Expired => "expired",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderDeliveryState {
    Queued,
    Missed,
    Shown,
}

impl ReminderDeliveryState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Missed => "missed",
            Self::Shown => "shown",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderOccurrence {
    pub id: String,
    pub reminder_id: String,
    pub name: String,
    pub content: String,
    pub scheduled_at: String,
    pub status: ReminderStatus,
    pub delivery_state: ReminderDeliveryState,
    pub completed_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderOccurrencePage {
    pub items: Vec<ReminderOccurrence>,
    pub next_after_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub persona_id: String,
    pub user_profile_id: Option<String>,
    pub title: String,
    pub memory_revision: i64,
    pub intimacy: i64,
    pub created_at: String,
    pub updated_at: String,
    pub is_current: bool,
}

/// 全局用户设定资料，可被多个会话引用，删除仅解除引用。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub id: String,
    pub name: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRoundStatus {
    Pending,
    Completed,
    Interrupted,
}

impl ChatRoundStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "completed" => Some(Self::Completed),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRound {
    pub id: String,
    pub session_id: String,
    pub status: ChatRoundStatus,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub stop_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

impl MessageRole {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub round_id: Option<String>,
    pub role: MessageRole,
    pub content: String,
    pub expression: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePage {
    pub items: Vec<Message>,
    pub next_after_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundSnapshot {
    pub round: ChatRound,
    pub user_message: Message,
    pub assistant_message: Option<Message>,
}
