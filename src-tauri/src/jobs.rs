//! 桌宠计时与提醒共用唯一后台循环，提醒先完成离线补偿再在线调度。
use crate::{events::PetEventSink, runtime::Runtime};
use std::{sync::Arc, time::Duration};

pub async fn run(
    runtime: Runtime,
    sink: Arc<dyn PetEventSink>,
    reminders: Arc<dyn crate::events::ReminderEventSink>,
) {
    let mut revision = None;
    let mut last_error = None;
    let mut reminders_recovered = false;
    let mut reminder_error = None;
    loop {
        if runtime.jobs_cancel.check().is_err() {
            break;
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
        let worker = runtime.clone();
        let result = tauri::async_runtime::spawn_blocking(move || worker.pet_view())
            .await
            .unwrap_or_else(|_| Err(crate::error::AppError::storage()));
        if runtime.jobs_cancel.check().is_err() {
            break;
        }
        match result {
            Ok(view) => {
                if revision != Some(view.revision) || last_error.is_some() {
                    sink.changed(&view);
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
        if runtime
            .jobs_cancel
            .wait(Duration::from_secs(2), async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok(())
            })
            .await
            .is_err()
        {
            break;
        }
    }
}
