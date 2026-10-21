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
    fn wander(&self, view: &View) { crate::platform::windows::wander::tick(&self.0, view); }
    fn state_bubble(&self, bubble: &crate::events::PetStateBubble) {
        let _ = self.0.emit(crate::events::PET_STATE_BUBBLE, bubble);
    }
    fn changed(&self, view: &View) {
        let _ = self.0.emit(PET_ACTIVITY_CHANGED, view);
    }
    fn failed(&self, error: &AppError) {
        let _ = self.0.emit(PET_ACTIVITY_ERROR, error);
    }
}
impl crate::events::ChatToolEventSink for PetEvents {
    fn real_money_changed(&self) {let _=self.0.emit("real-money-changed",());}
    fn real_money_failed(&self,error:&crate::error::AppError) {let _=self.0.emit("real-money-error",error);}
    fn festivals_changed(&self) { let _ = self.0.emit(crate::events::FESTIVALS_CHANGED, serde_json::json!({"sessionId":null})); }
    fn accessory_preview_changed(&self,session:&str,preview:&str) {
        let _=self.0.emit(crate::events::ACCESSORY_CRAFT_PREVIEW_CHANGED,serde_json::json!({"sessionId":session,"previewId":preview}));
    }
    fn notebook_changed(&self) { let _ = self.0.emit(crate::events::NOTEBOOK_CHANGED, ()); }
}
impl crate::events::ProactiveEventSink for PetEvents {
    fn proactive_failed(&self, error: &AppError) { let _ = self.0.emit(PET_ACTIVITY_ERROR, error); }
    fn proactive_message(&self, value: &crate::events::ProactiveMessage) {
        let _ = self.0.emit("chat-messages-changed", serde_json::json!({"sessionId": value.session_id}));
        let _ = self.0.emit("proactive-message", value);
    }
}

impl crate::events::LetterEventSink for PetEvents {
    fn letters_changed(&self,session_id:&str){let _=self.0.emit(crate::events::LETTERS_CHANGED,serde_json::json!({"sessionId":session_id}));}
}

impl crate::events::TravelEventSink for PetEvents {
    fn accessories_changed(&self, session_id:&str) {
        let _=self.0.emit(crate::events::ACCESSORIES_CHANGED,serde_json::json!({"sessionId":session_id}));
    }
    fn travel_changed(&self, session_id: &str) {
        let _ = self.0.emit(crate::events::TRAVEL_CHANGED,serde_json::json!({"sessionId":session_id}));
    }
}
