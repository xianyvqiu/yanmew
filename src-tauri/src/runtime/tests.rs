use super::*;
use crate::{
    pet::{
        controller::{Action, Activity, InteractionInput},
        RandomSource,
    },
    platform::contracts::{Clock, ClockSample},
};
use chrono::{DateTime, Utc};
use std::time::Duration;

struct TestClock(Mutex<ClockSample>);
impl Clock for TestClock {
    fn sample(&self) -> AppResult<ClockSample> {
        Ok(*self.0.lock().unwrap())
    }
}
impl TestClock {
    fn advance(&self, seconds: u64) {
        let mut s = self.0.lock().unwrap();
        s.running += Duration::from_secs(seconds);
        s.utc += chrono::Duration::seconds(seconds as i64);
    }
    fn offline(&self, seconds: i64) {
        self.0.lock().unwrap().utc += chrono::Duration::seconds(seconds);
    }
}
struct Fixed(u64);
impl RandomSource for Fixed {
    fn below(&mut self, upper: u64) -> AppResult<u64> {
        Ok(self.0 % upper)
    }
}
fn setup(random: u64) -> (Runtime, Arc<TestClock>) {
    let mut runtime = test_runtime("http://127.0.0.1:9/v1");
    let clock = Arc::new(TestClock(Mutex::new(ClockSample {
        utc: DateTime::parse_from_rfc3339("2026-09-20T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        running: Duration::ZERO,
    })));
    runtime.clock = clock.clone();
    runtime.random = Arc::new(Mutex::new(Box::new(Fixed(random))));
    (runtime, clock)
}
fn snapshot(runtime: &Runtime, id: i64) -> crate::pet_store::PetSnapshot {
    runtime
        .read(|s, _| crate::pet_store::read(&s.connection, id))
        .unwrap()
}
fn input(action: Action) -> InteractionInput {
    InteractionInput {
        persona_id: "1".into(),
        session_id: "1".into(),
        context_revision: 0,
        action,
    }
}

#[test]
fn failed_visibility_checkpoint_still_blocks_idle_until_recovery() {
    let (runtime, clock) = setup(0);
    runtime.pet_view().unwrap();
    runtime.read(|s,_| {s.connection.execute_batch("CREATE TRIGGER reject_pet BEFORE UPDATE ON session_pet_states BEGIN SELECT RAISE(ABORT,'test'); END;")?;Ok(())}).unwrap();
    clock.advance(60);
    assert!(runtime.pet_visibility(Some(true), None).is_err());
    clock.advance(3600);
    assert!(runtime.pet_visibility(Some(false), None).is_err());
    runtime
        .read(|s, _| {
            s.connection.execute_batch("DROP TRIGGER reject_pet")?;
            Ok(())
        })
        .unwrap();
    runtime.pet_view().unwrap();
    assert_eq!(snapshot(&runtime, 1).idle_elapsed_ms, 0);
    assert_eq!(snapshot(&runtime, 1).hunger_elapsed_ms, 3660000);
    clock.advance(60);
    runtime.pet_view().unwrap();
    assert_eq!(snapshot(&runtime, 1).idle_elapsed_ms, 60000);
}

#[test]
fn running_timer_reaches_bored_once_and_interaction_is_capped() {
    let (runtime, clock) = setup(0);
    runtime
        .read(|s, _| {
            s.connection
                .execute("UPDATE sessions SET intimacy=998", [])?;
            Ok(())
        })
        .unwrap();
    runtime.pet_view().unwrap();
    clock.advance(44 * 60);
    assert_eq!(runtime.pet_view().unwrap().activity, Activity::Normal);
    clock.advance(60);
    let bored = runtime.pet_view().unwrap();
    assert_eq!(bored.activity, Activity::Bored);
    clock.advance(60);
    assert_eq!(runtime.pet_view().unwrap().revision, bored.revision);
    assert_eq!(
        runtime.pet_interact(input(Action::Feed)).unwrap().intimacy,
        Some(999)
    );
    assert_eq!(snapshot(&runtime, 1).hunger_elapsed_ms, 0);
    assert_eq!(
        runtime
            .pet_interact(input(Action::Stroke))
            .unwrap()
            .intimacy,
        Some(999)
    );
    clock.advance(3);
    assert_eq!(runtime.pet_view().unwrap().activity, Activity::Normal);
}

#[test]
fn paused_system_only_advances_sleep_deadline() {
    let (runtime, clock) = setup(1);
    runtime.pet_view().unwrap();
    clock.advance(45 * 60);
    let sleeping = runtime.pet_view().unwrap();
    assert_eq!(sleeping.activity, Activity::Sleeping);
    let saved = snapshot(&runtime, 1);
    clock.offline(3600);
    assert_eq!(
        runtime.pet_view().unwrap().sleep_until,
        sleeping.sleep_until
    );
    clock.offline(120);
    let awake = runtime.pet_view().unwrap();
    assert_eq!(awake.activity, Activity::Normal);
    let after = snapshot(&runtime, 1);
    assert_eq!(after.hunger_elapsed_ms, saved.hunger_elapsed_ms);
    assert_eq!(after.idle_elapsed_ms, 0);
}

#[test]
fn hidden_dragging_and_chat_pause_idle_and_never_start_background_ai() {
    let (runtime, clock) = setup(0);
    runtime.pet_view().unwrap();
    clock.advance(60);
    runtime.pet_visibility(Some(true), None).unwrap();
    clock.advance(3600);
    runtime.pet_view().unwrap();
    assert_eq!(snapshot(&runtime, 1).idle_elapsed_ms, 60_000);
    runtime.pet_visibility(Some(false), Some(true)).unwrap();
    clock.advance(60);
    assert!(!runtime.pet_view().unwrap().can_wander);
    runtime.pet_visibility(None, Some(false)).unwrap();
    let request = runtime.requests.start("chat:1".into(), 1).unwrap();
    runtime.pet_view().unwrap();
    clock.advance(3600);
    assert_eq!(runtime.pet_view().unwrap().activity, Activity::Thinking);
    assert_eq!(snapshot(&runtime, 1).idle_elapsed_ms, 60_000);
    drop(request);
    assert_ne!(runtime.pet_view().unwrap().activity, Activity::Thinking);
}

#[test]
fn failed_checkpoint_retries_once_and_shutdown_flushes_once() {
    let (runtime, clock) = setup(0);
    runtime.pet_view().unwrap();
    clock.advance(60);
    runtime.read(|s,_| {s.connection.execute_batch("CREATE TRIGGER reject_tick BEFORE UPDATE ON session_pet_states BEGIN SELECT RAISE(ABORT,'test'); END;")?;Ok(())}).unwrap();
    assert!(runtime.pet_view().is_err());
    assert_eq!(snapshot(&runtime, 1).hunger_elapsed_ms, 0);
    runtime
        .read(|s, _| {
            s.connection.execute_batch("DROP TRIGGER reject_tick;")?;
            Ok(())
        })
        .unwrap();
    clock.advance(60);
    runtime.pet_view().unwrap();
    runtime.pet_view().unwrap();
    assert_eq!(snapshot(&runtime, 1).hunger_elapsed_ms, 120_000);
    clock.advance(5);
    runtime.shutdown().unwrap();
    clock.advance(20);
    runtime.shutdown().unwrap();
    assert_eq!(snapshot(&runtime, 1).hunger_elapsed_ms, 125_000);
    assert!(runtime.pet_interact(input(Action::Click)).is_err());
}

#[test]
fn switching_restores_own_state_and_restart_does_not_count_offline_time() {
    let (runtime, clock) = setup(0);
    runtime.pet_view().unwrap();
    clock.advance(60);
    runtime.read(|s,p| {
        runtime.settle_pet(s,p)?;
        s.connection.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(2,1,'B','a','a')",[])?;
        s.connection.execute("UPDATE settings SET value='2' WHERE key='current_session_id'",[])?;
        s.context_revision+=1;s.pet.reset_context();runtime.settle_pet(s,p)?;Ok(())
    }).unwrap();
    clock.advance(30);
    runtime.pet_view().unwrap();
    assert_eq!(snapshot(&runtime, 1).hunger_elapsed_ms, 60_000);
    assert_eq!(snapshot(&runtime, 2).hunger_elapsed_ms, 30_000);
    assert!(runtime.pet_interact(input(Action::Feed)).is_err());
    runtime
        .read(|s, p| {
            s.pet = Default::default();
            clock.offline(86400);
            runtime.settle_pet(s, p)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(snapshot(&runtime, 2).hunger_elapsed_ms, 30_000);
}

#[test]
fn provider_error_survives_ticks_context_switch_and_stale_success() {
    let (runtime, clock) = setup(0);
    runtime.pet_view().unwrap();
    let old = runtime.read(|s, p| s.pet.health(&s.connection, p)).unwrap();
    runtime
        .read(|s, _| {
            s.pet.complete_health(
                &old,
                &Err(AppError::new("PROVIDER_UNAVAILABLE", "连接中断")),
            );
            s.context_revision += 1;
            s.pet.reset_context();
            Ok(())
        })
        .unwrap();
    clock.advance(3600);
    let dormant = runtime.pet_view().unwrap();
    assert_eq!(dormant.activity, Activity::Dormant);
    assert!(!dormant.can_wander);
    runtime
        .read(|s, _| {
            s.pet.complete_health(&old, &Ok(()));
            Ok(())
        })
        .unwrap();
    assert_eq!(runtime.pet_view().unwrap().activity, Activity::Dormant);
    runtime
        .read(|s, p| {
            let current = s.pet.health(&s.connection, p)?;
            s.pet.complete_health(&current, &Ok(()));
            Ok(())
        })
        .unwrap();
    assert_eq!(runtime.pet_view().unwrap().error, None);
}

#[test]
fn no_session_never_creates_one_and_away_never_rewards_or_counts() {
    let (runtime, clock) = setup(0);
    runtime.pet_view().unwrap();
    runtime.read(|s,_|{s.connection.execute("INSERT INTO travels(session_id,status,started_at,planned_return_at,midway_letter_at,created_at) VALUES(1,'away','2026-09-20T00:00:00Z','2026-09-20T01:00:00Z','2026-09-20T00:30:00Z','2026-09-20T00:00:00Z')",[])?;Ok(())}).unwrap();
    assert_eq!(runtime.pet_view().unwrap().activity, Activity::Away);
    clock.advance(60);
    assert!(!runtime.pet_view().unwrap().can_interact);
    assert!(runtime.pet_interact(input(Action::Feed)).is_err());
    assert_eq!(snapshot(&runtime, 1).hunger_elapsed_ms, 0);
    runtime
        .read(|s, _| {
            s.connection
                .execute("DELETE FROM settings WHERE key='current_session_id'", [])?;
            s.context_revision += 1;
            Ok(())
        })
        .unwrap();
    clock.advance(60);
    let empty = runtime.pet_view().unwrap();
    assert!(empty.session_id.is_none());
    assert!(!empty.can_interact);
    assert_eq!(
        runtime
            .read(|s, _| Ok(s
                .connection
                .query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, i64>(0))?))
            .unwrap(),
        1
    );
}

#[test]
fn task_errors_do_not_sleep_but_provider_transport_failures_do() {
    for code in ["AI_PROTOCOL_ERROR", "AI_TRUNCATED", "VALIDATION_ERROR", "CONTEXT_TOO_LARGE",
        "CHAT_REPETITION", "CHAT_INVALID_ENCODING", "CHAT_NO_PROGRESS", "CANCELLED", "STORAGE_ERROR"] {
        let (runtime, _) = setup(0);
        runtime.read(|store, secrets| {
            let token = store.pet.health(&store.connection, secrets)?;
            store.pet.report_ai_failure(&token, &AppError::new(code, "本次任务失败"));
            Ok(())
        }).unwrap();
        assert_ne!(runtime.pet_view().unwrap().activity, Activity::Dormant, "{code}");
    }
    for code in ["PROVIDER_UNAVAILABLE", "AI_TIMEOUT", "AI_STREAM_INTERRUPTED"] {
        let (runtime, clock) = setup(0);
        runtime.read(|store, secrets| {
            let token = store.pet.health(&store.connection, secrets)?;
            store.pet.report_ai_failure(&token, &AppError::new(code, "连接失败"));
            Ok(())
        }).unwrap();
        clock.advance(3600);
        assert_eq!(runtime.pet_view().unwrap().activity, Activity::Dormant, "{code}");
    }
}

#[test]
fn only_effective_chat_configuration_changes_clear_provider_failure() {
    let (runtime, _) = setup(0);
    runtime.read(|store, secrets| {
        let token = store.pet.health(&store.connection, secrets)?;
        store.pet.report_ai_failure(&token, &AppError::new("PROVIDER_UNAVAILABLE", "连接失败"));
        store.connection.execute("UPDATE providers SET name='改名' WHERE id=(SELECT provider_id FROM provider_roles WHERE purpose='chat')", [])?;
        store.connection.execute("UPDATE provider_models SET max_output_tokens=4096 WHERE purpose='chat' AND provider_id=(SELECT provider_id FROM provider_roles WHERE purpose='chat')", [])?;
        Ok(())
    }).unwrap();
    assert_eq!(runtime.pet_view().unwrap().activity, Activity::Dormant);
    runtime.read(|store, _| {
        store.connection.execute("UPDATE provider_models SET model='another-model' WHERE purpose='chat' AND provider_id=(SELECT provider_id FROM provider_roles WHERE purpose='chat')", [])?;
        Ok(())
    }).unwrap();
    assert_eq!(runtime.pet_view().unwrap().error, None);
}

fn travel_wait(runtime: &Runtime) -> Option<i64> {
    runtime.read(|s, _| Ok(s.connection.query_row("SELECT travel_trigger_remaining_seconds FROM sessions WHERE id=1", [], |r| r.get(0))?)).unwrap()
}
#[test]
fn travel_wait_pauses_for_hidden_chat_and_sleep() {
    let (runtime, clock) = setup(0);
    runtime.pet_view().unwrap();
    let initial = travel_wait(&runtime).unwrap();
    clock.advance(10); runtime.pet_view().unwrap();
    assert_eq!(travel_wait(&runtime), Some(initial-10));
    runtime.pet_visibility(Some(true), None).unwrap();
    clock.advance(3600); runtime.pet_view().unwrap();
    assert_eq!(travel_wait(&runtime), Some(initial-10));
    runtime.pet_visibility(Some(false), None).unwrap();
    let request = runtime.requests.start("chat:1".into(), 1).unwrap();
    runtime.pet_view().unwrap(); clock.advance(3600); runtime.pet_view().unwrap();
    assert_eq!(travel_wait(&runtime), Some(initial-10));
    drop(request); runtime.pet_view().unwrap();
    clock.advance(5); runtime.pet_view().unwrap();
    assert_eq!(travel_wait(&runtime), Some(initial-15));
    let (runtime, clock) = setup(1);
    runtime.pet_view().unwrap(); let initial=travel_wait(&runtime).unwrap();
    clock.advance(2700); assert_eq!(runtime.pet_view().unwrap().activity, Activity::Sleeping);
    assert_eq!(travel_wait(&runtime), Some(initial-2700));
    clock.advance(60); runtime.pet_view().unwrap();
    assert_eq!(travel_wait(&runtime), Some(initial-2700));
    clock.offline(7200); runtime.pet_view().unwrap();
    assert_eq!(travel_wait(&runtime), Some(initial-2700));
}
#[test]
fn frequent_queries_keep_fractional_travel_time_and_restart_does_not_backfill() {
    let (runtime, clock)=setup(0);
    runtime.pet_view().unwrap();let initial=travel_wait(&runtime).unwrap();
    for _ in 0..4 {
        {let mut sample=clock.0.lock().unwrap(); sample.running+=Duration::from_millis(250); sample.utc+=chrono::Duration::milliseconds(250);}
        runtime.pet_view().unwrap();
    }
    assert_eq!(travel_wait(&runtime),Some(initial-1));
    runtime.read(|s,_|{s.pet.reset_context();Ok(())}).unwrap();
    clock.offline(86400);runtime.pet_view().unwrap();
    assert_eq!(travel_wait(&runtime),Some(initial-1));
}
#[test]
fn travel_departure_blocks_chat_and_interaction_without_reward() {
    let (runtime, clock)=setup(0);
    runtime.read(|s,_|{s.connection.execute("UPDATE sessions SET travel_trigger_remaining_seconds=5 WHERE id=1",[])?;Ok(())}).unwrap();
    runtime.pet_view().unwrap();clock.advance(5);
    let away=runtime.pet_view().unwrap();assert_eq!(away.activity,Activity::Away);
    assert!(!away.can_interact);assert!(!away.can_wander);assert_eq!(travel_wait(&runtime),None);
    assert_eq!(runtime.pet_interact(input(Action::Feed)).unwrap_err().code,"PET_AWAY");
    assert_eq!(runtime.pet_view().unwrap().intimacy,Some(0));
    clock.offline(7200); assert_ne!(runtime.pet_view().unwrap().activity,Activity::Away);
    assert!(travel_wait(&runtime).unwrap()>=7200);
}
