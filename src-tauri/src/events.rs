//! 平台无关的桌宠事件出口；业务不依赖 Tauri 窗口。
pub const PET_ACTIVITY_CHANGED: &str = "pet-activity-changed";
pub const PET_ACTIVITY_ERROR: &str = "pet-activity-error";
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
}
