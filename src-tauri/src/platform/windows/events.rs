use crate::{
    error::AppError,
    events::{PetEventSink, PET_ACTIVITY_CHANGED, PET_ACTIVITY_ERROR},
    pet::controller::View,
};
use tauri::Emitter;
pub struct PetEvents(pub tauri::AppHandle);
impl crate::events::MemoryEventSink for PetEvents {
    fn embedding_changed(&self) { let _ = self.0.emit(crate::events::EMBEDDING_CHANGED, ()); }
    fn memory_changed(&self) {
        let _ = self.0.emit(crate::events::MEMORY_GENERATION_CHANGED, ());
    }
}
impl crate::events::ReminderEventSink for PetEvents {
    fn reminders_changed(&self) {
        let _ = self.0.emit(crate::events::REMINDERS_CHANGED, ());
    }
    fn reminder_failed(&self, error: &AppError) {
        let _ = self.0.emit(crate::events::REMINDER_ERROR, error);
    }
}
impl PetEventSink for PetEvents {
    fn changed(&self, view: &View) {
        let _ = self.0.emit(PET_ACTIVITY_CHANGED, view);
    }
    fn failed(&self, error: &AppError) {
        let _ = self.0.emit(PET_ACTIVITY_ERROR, error);
    }
}
