//! 唯一后台循环的临时提示去重；计时与睡眠事实仍由现有会话仓储维护。
use super::controller::{Activity, View};
use crate::events::PetStateBubble;

#[derive(Default)]
pub struct Tracker {
    context: Option<(String, Option<String>, u64)>,
    activity: Option<Activity>,
}

impl Tracker {
    pub fn observe(&mut self, view: &View) -> Option<PetStateBubble> {
        let context = (
            view.persona_id.clone(),
            view.session_id.clone(),
            view.context_revision,
        );
        if self.context.as_ref() != Some(&context) {
            self.context = Some(context);
            self.activity = None;
        }
        // 短时互动与聊天表情不代表原饥饿周期结束，避免返回原状态时重复提示。
        if matches!(
            view.activity,
            Activity::Eating | Activity::Interacting | Activity::Thinking | Activity::Speaking
        ) {
            return None;
        }
        if self.activity == Some(view.activity) {
            return None;
        }
        self.activity = Some(view.activity);
        let text = match view.activity {
            Activity::Hungry => "好饿…",
            Activity::Bored => "好无聊…",
            _ => return None,
        };
        // 隐藏或拖动时消费本次提示但不唤醒宠窗、不排队补弹。
        if !view.can_wander {
            return None;
        }
        Some(PetStateBubble {
            persona_id: view.persona_id.clone(),
            session_id: view.session_id.clone()?,
            context_revision: view.context_revision,
            revision: view.revision,
            activity: view.activity,
            text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(activity: Activity) -> View {
        View {
            persona_id: "1".into(),
            session_id: Some("1".into()),
            context_revision: 0,
            revision: 1,
            activity,
            expression: "(・ω・)".into(),
            intimacy: Some(1),
            sleep_until: None,
            error: None,
            can_interact: true,
            can_wander: true,
        }
    }

    #[test]
    fn continuous_state_and_temporary_expressions_do_not_repeat() {
        let mut tracker = Tracker::default();
        let hungry = view(Activity::Hungry);
        assert_eq!(tracker.observe(&hungry).unwrap().text, "好饿…");
        assert!(tracker.observe(&hungry).is_none());
        for activity in [
            Activity::Interacting,
            Activity::Eating,
            Activity::Thinking,
            Activity::Speaking,
        ] {
            assert!(tracker.observe(&view(activity)).is_none());
            assert!(tracker.observe(&hungry).is_none());
        }
    }

    #[test]
    fn stable_state_change_starts_new_notice() {
        let mut tracker = Tracker::default();
        assert_eq!(
            tracker.observe(&view(Activity::Bored)).unwrap().text,
            "好无聊…"
        );
        assert!(tracker.observe(&view(Activity::Normal)).is_none());
        assert!(tracker.observe(&view(Activity::Bored)).is_some());
        assert!(tracker.observe(&view(Activity::Hungry)).is_some());
    }

    #[test]
    fn hidden_or_dragging_does_not_queue_or_replay() {
        let mut tracker = Tracker::default();
        let mut hungry = view(Activity::Hungry);
        hungry.can_wander = false;
        assert!(tracker.observe(&hungry).is_none());
        hungry.can_wander = true;
        assert!(tracker.observe(&hungry).is_none());
    }

    #[test]
    fn context_change_uses_target_session_and_discards_empty_context() {
        let mut tracker = Tracker::default();
        let mut bored = view(Activity::Bored);
        assert!(tracker.observe(&bored).is_some());
        bored.context_revision = 1;
        bored.session_id = Some("2".into());
        assert_eq!(tracker.observe(&bored).unwrap().session_id, "2");
        bored.context_revision = 2;
        bored.session_id = None;
        assert!(tracker.observe(&bored).is_none());
    }
}
