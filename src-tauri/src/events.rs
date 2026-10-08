//! 平台无关的桌宠事件出口；业务不依赖 Tauri 窗口。
pub const PET_ACTIVITY_CHANGED: &str = "pet-activity-changed";
pub const PET_ACTIVITY_ERROR: &str = "pet-activity-error";
pub const PET_STATE_BUBBLE: &str = "pet-state-bubble";

/// 临时状态提示，不是聊天消息；沿用活动版本防止迟到事件串会话。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetStateBubble {
    pub persona_id: String,
    pub session_id: String,
    pub context_revision: u64,
    pub revision: u64,
    pub activity: crate::pet::controller::Activity,
    pub text: &'static str,
}
pub const REMINDERS_CHANGED: &str = "reminders-changed";
pub const REMINDER_ERROR: &str = "reminder-error";
pub const NOTEBOOK_CHANGED: &str = "notebook-changed";
pub const MEMORY_GENERATION_CHANGED: &str = "memory-generation-changed";
pub const EMBEDDING_CHANGED: &str = "embedding-state-changed";
pub trait MemoryEventSink: Send + Sync {
    fn memory_changed(&self);
    fn embedding_changed(&self) {}
}
pub trait ChatToolEventSink: ReminderEventSink {
    fn notebook_changed(&self);
}
pub trait ReminderEventSink: Send + Sync {
    fn reminders_changed(&self);
    fn reminder_failed(&self, error: &crate::error::AppError);
}
pub trait PetEventSink: Send + Sync {
    fn changed(&self, view: &crate::pet::controller::View);
    fn failed(&self, error: &crate::error::AppError);
    fn state_bubble(&self, _bubble: &PetStateBubble) {}
    fn wander(&self, _view: &crate::pet::controller::View) {}
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProactiveMessage {
    pub persona_id: String,
    pub session_id: String,
    pub context_revision: u64,
    pub message: crate::models::Message,
}
pub trait ProactiveEventSink: ChatToolEventSink {
    fn proactive_message(&self, message: &ProactiveMessage);
    fn proactive_failed(&self, error: &crate::error::AppError);
}
