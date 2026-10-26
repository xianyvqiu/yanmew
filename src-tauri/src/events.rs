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
pub const FESTIVALS_CHANGED: &str = "festivals-changed";
pub const MEMORY_GENERATION_CHANGED: &str = "memory-generation-changed";
pub const EMBEDDING_CHANGED: &str = "embedding-state-changed";
pub const PERSONA_UPDATED: &str = "persona-updated";
/// 只在钱包事务提交成功后通知；接收端重新读取，不凭事件直接加减余额。
pub const WALLET_CHANGED: &str = "wallet_changed";
/// 饰品事务提交后仅通知归属，窗口重新查询，不携带图片或私有正文。
pub const ACCESSORIES_CHANGED: &str = "accessories-changed";
pub const ACCESSORY_CRAFT_PREVIEW_CHANGED: &str = "accessory-craft-preview-changed";
pub const IMAGE_GENERATION_CONFIG_CHANGED: &str = "image-generation-config-changed";

/// 仅在完整聊天与人设调整分别提交成功后发出，不携带私人人设正文。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaUpdated {
    pub persona_id: String,
    pub session_id: String,
    pub context_revision: u64,
    pub evolved_at: String,
}
pub trait MemoryEventSink: Send + Sync {
    fn memory_changed(&self);
    fn embedding_changed(&self) {}
}
pub trait ChatToolEventSink: ReminderEventSink {
    fn real_money_changed(&self) {}
    fn real_money_failed(&self, _error: &crate::error::AppError) {}
    fn skills_changed(&self) {}
    fn festivals_changed(&self) {}
    fn accessory_preview_changed(&self, _session: &str, _preview: &str) {}
    fn notebook_changed(&self);
    fn persona_updated(&self, _update: &PersonaUpdated) {}
    fn persona_evolution_failed(&self, _error: &crate::error::AppError) {}
}
pub trait ReminderEventSink: Send + Sync {
    fn chat_messages_changed(&self, _session_id: &str) {}
    fn reminders_changed(&self);
    fn reminder_failed(&self, error: &crate::error::AppError);
}
pub trait PetEventSink: Send + Sync {
    fn weather_changed(&self) {}
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

/// 事件仅提示对应会话的信件/任务状态变化；页面重新查询，不携带信件正文。
pub const LETTERS_CHANGED:&str="letters-changed";
pub trait LetterEventSink:Send+Sync {fn letters_changed(&self,session_id:&str);}

/// 固定行程生成的状态提示；到期/出发继续复用pet-activity-changed。
pub const TRAVEL_CHANGED: &str = "travel-changed";
pub trait TravelEventSink: Send + Sync { fn travel_changed(&self, session_id: &str); fn accessories_changed(&self, _session_id:&str) {} }
