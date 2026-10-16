//! 桌宠、提醒与记忆共用唯一后台循环；记忆推理异步执行，不阻塞计时。
use crate::{events::PetEventSink, runtime::Runtime};
use std::{sync::Arc, time::Duration};

pub async fn run(
    runtime: Runtime,
    sink: Arc<dyn PetEventSink>,
    reminders: Arc<dyn crate::events::ReminderEventSink>,
    memory_events: Arc<dyn crate::events::MemoryEventSink>,
    proactive_events: Arc<dyn crate::events::ProactiveEventSink>,
    letter_events: Arc<dyn crate::events::LetterEventSink>,
    travel_events: Arc<dyn crate::events::TravelEventSink>,
) {
    let mut revision = None;
    let mut last_error = None;
    let mut reminders_recovered = false;
    let mut reminder_error = None;
    let mut memory = crate::memory::Worker::default();
    let mut memory_index = crate::memory_index::Worker::default();
    let mut state_bubbles = crate::pet::state_bubbles::Tracker::default();
    let mut calendar_date = None;
    let mut calendar_error = None;
    loop {
        if runtime.jobs_cancel.check().is_err() {
            break;
        }
        // 沿用唯一循环；日期变化（含跨年、时区切换和日期回拨）时才提交日历写事务。
        if let Ok(sample) = runtime.clock.sample() {
            let date = sample.utc.with_timezone(&chrono::Local).date_naive();
            if calendar_date != Some(date) {
                let worker = runtime.clone();
                let result = tauri::async_runtime::spawn_blocking(move || worker.read(|store, _| {
                    let current = worker.clock.sample()?.utc.with_timezone(&chrono::Local).date_naive();
                    crate::festivals::synchronize(&mut store.connection, current)?;
                    Ok(current)
                })).await.unwrap_or_else(|_| Err(crate::error::AppError::storage()));
                match result {
                    Ok(current) => { calendar_date = Some(current); calendar_error = None; proactive_events.festivals_changed(); }
                    Err(error) => {
                        // 母表失败独立记录，保持可重试；不归类为模型故障或桌宠休眠。
                        if calendar_error.as_ref() != Some(&error.message) {
                            eprintln!("节日日期更新失败：{}", error.message);
                        }
                        calendar_error = Some(error.message);
                    }
                }
            }
        }
        let worker = runtime.clone();
        let reminder_result = tauri::async_runtime::spawn_blocking(move || {
            worker.read(|store, _| {
                crate::reminders::dispatch(
                    &mut store.connection,
                    worker.clock.sample()?.utc,
                    &chrono::Local,
                    !reminders_recovered,
                )
            })
        })
        .await
        .unwrap_or_else(|_| Err(crate::error::AppError::storage()));
        if runtime.jobs_cancel.check().is_err() {
            break;
        }
        match reminder_result {
            Ok(count) => {
                // 首次成功也通知页面恢复数据库中原有的 queued/missed，事件丢失可主动查询。
                if count > 0 || !reminders_recovered || reminder_error.is_some() {
                    reminders.reminders_changed();
                }
                reminders_recovered = true;
                reminder_error = None;
            }
            Err(error) => {
                if reminder_error.as_ref() != Some(&error.message) {
                    reminders.reminder_failed(&error);
                }
                reminder_error = Some(error.message);
            }
        }
        crate::reminder_email::tick(&runtime);
        let worker = runtime.clone();
        let result = tauri::async_runtime::spawn_blocking(move || worker.pet_view())
            .await
            .unwrap_or_else(|_| Err(crate::error::AppError::storage()));
        if runtime.jobs_cancel.check().is_err() {
            break;
        }
        match result {
            Ok(view) => {
                sink.wander(&view);
                if revision != Some(view.revision) || last_error.is_some() {
                    sink.changed(&view);
                }
                if let Some(bubble) = state_bubbles.observe(&view) {
                    sink.state_bubble(&bubble);
                }
                revision = Some(view.revision);
                last_error = None;
            }
            Err(error) => {
                if last_error.as_ref() != Some(&error.message) {
                    sink.failed(&error);
                }
                last_error = Some(error.message);
            }
        }
        if let Err(error) = memory.tick(&runtime, memory_events.as_ref()).await {
            if error.code != "CANCELLED" {
                crate::memory::Worker::report_scan_error(&runtime, memory_events.as_ref(), error);
            }
        }
        memory_index.tick(&runtime, memory_events.clone());
        runtime.travel_facts.tick(&runtime,travel_events.clone()).await;
        runtime.travel_gifts.tick(&runtime,travel_events.clone()).await;
        runtime.travel_journals.tick(&runtime,travel_events.clone()).await;
        crate::letters::generation::tick(&runtime,letter_events.clone()).await;
        // 只等待本地资格与事务；AI 推理在独立任务中执行。
        crate::proactive::tick(&runtime, proactive_events.clone()).await;
        if runtime
            .jobs_cancel
            .wait(Duration::from_secs(2), async {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {},
                    _ = runtime.memory.wake.notified() => {},
                    _ = runtime.letters.wake.notified() => {},
                    _ = runtime.travel_facts.wake.notified() => {},
                    _ = runtime.travel_gifts.wake.notified() => {},
                    _ = runtime.travel_journals.wake.notified() => {},
                }
                Ok(())
            })
            .await
            .is_err()
        {
            break;
        }
    }
}
