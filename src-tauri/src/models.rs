use serde::Serialize;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoundSnapshot {
    pub round: ChatRound,
    pub user_message: Message,
    pub assistant_message: Option<Message>,
}
