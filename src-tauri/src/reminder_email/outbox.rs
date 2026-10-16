use super::*;
use std::sync::{atomic::Ordering, Arc};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryView {
    pub id: String,
    pub recipient: String,
    pub subject: String,
    pub state: String,
    pub attempts: u32,
    pub next_retry_at: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub sent_at: Option<String>,
}
pub(super) fn list(c: &Connection) -> AppResult<Vec<DeliveryView>> {
    let mut query=c.prepare("SELECT id,recipient,subject,state,attempts,next_retry_at,error,created_at,sent_at FROM reminder_email_deliveries ORDER BY id DESC LIMIT 20")?;
    let result = query
        .query_map([], |r| {
            Ok(DeliveryView {
                id: r.get::<_, i64>(0)?.to_string(),
                recipient: r.get(1)?,
                subject: r.get(2)?,
                state: r.get(3)?,
                attempts: r.get(4)?,
                next_retry_at: r.get(5)?,
                error: r.get(6)?,
                created_at: r.get(7)?,
                sent_at: r.get(8)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(result)
}
pub(super) fn cancel(c: &Connection) -> AppResult<()> {
    c.execute("UPDATE reminder_email_deliveries SET state='cancelled',next_retry_at=NULL,error=NULL WHERE state IN ('pending','retry_wait','failed','uncertain')",[])?;
    Ok(())
}
/// 仅在提醒dispatch的事务中登记本次新触发ID，不扫描历史或由UI刷新产生邮件。
pub(crate) fn enqueue(
    c: &Connection,
    ids: &[i64],
    offline: bool,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let config = load(c)?;
    if !config.enabled || !config.verified || encrypted_secret(c)?.is_none() {
        return Ok(());
    }
    let since = config
        .enabled_since
        .as_deref()
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .map(|v| v.with_timezone(&Utc));
    let Some(since) = since else {
        return Ok(());
    };
    let mut items = Vec::new();
    for id in ids {
        let row:Option<(String,String,String)>=c.query_row("SELECT r.name,r.content,o.scheduled_at FROM reminder_occurrences o JOIN reminders r ON r.id=o.reminder_id WHERE o.id=?1 AND NOT EXISTS(SELECT 1 FROM reminder_email_items WHERE occurrence_id=o.id)",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((name, content, scheduled)) = row {
            let at = DateTime::parse_from_rfc3339(&scheduled).map_err(|_| AppError::storage())?;
            if at.with_timezone(&Utc) >= since {
                items.push((
                    *id,
                    name,
                    content,
                    at.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S %:z")
                        .to_string(),
                ));
            }
        }
    }
    if offline {
        insert(c, &config, &items, true, now)?;
    } else {
        for item in items {
            insert(c, &config, &[item], false, now)?;
        }
    }
    Ok(())
}
fn insert(
    c: &Connection,
    config: &Config,
    items: &[(i64, String, String, String)],
    offline: bool,
    now: DateTime<Utc>,
) -> AppResult<()> {
    if items.is_empty() {
        return Ok(());
    }
    let kind = if offline { "offline_summary" } else { "due" };
    c.execute("INSERT INTO reminder_email_deliveries(kind,recipient,subject,body,config_revision,created_at) VALUES(?1,?2,'','',?3,?4)",params![kind,config.connection.to,config.revision,stamp(now)])?;
    let delivery = c.last_insert_rowid();
    for (id, name, content, time) in items {
        c.execute("INSERT INTO reminder_email_items(delivery_id,occurrence_id,name,content,scheduled_time) VALUES(?1,?2,?3,?4,?5)",params![delivery,id,name,content,time])?;
    }
    rebuild(c, delivery, offline)?;
    Ok(())
}
fn rebuild(c: &Connection, id: i64, offline: bool) -> AppResult<bool> {
    let mut query=c.prepare("SELECT i.name,i.content,i.scheduled_time,o.reminder_id FROM reminder_email_items i JOIN reminder_occurrences o ON o.id=i.occurrence_id WHERE i.delivery_id=?1 ORDER BY o.scheduled_at,o.id")?;
    let items = query
        .query_map([id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if items.is_empty() {
        c.execute(
            "UPDATE reminder_email_deliveries SET state='cancelled',next_retry_at=NULL WHERE id=?1",
            [id],
        )?;
        return Ok(false);
    }
    let subject = if offline {
        "[颜喵] 你离开期间有事项过期".to_owned()
    } else {
        format!("[颜喵] 提醒：{}", items[0].0)
    };
    let mut body = if offline {
        "你离开这段时间，以下事项过期了：\n\n".to_owned()
    } else {
        String::new()
    };
    let mut seen = std::collections::HashSet::new();
    for (name, content, time, rule) in items {
        if seen.insert(rule) {
            body.push_str(&format!("{name}\n详情：{content}\n原定时间：{time}\n\n"));
        }
    }
    body.push_str("由本机颜喵提醒发送。应用关闭期间无法实时发送邮件。");
    c.execute(
        "UPDATE reminder_email_deliveries SET subject=?1,body=?2 WHERE id=?3",
        params![subject, body, id],
    )?;
    Ok(true)
}
pub(crate) fn recover(c: &Connection) -> AppResult<()> {
    c.execute("UPDATE reminder_email_deliveries SET state='uncertain',next_retry_at=NULL,error='上次发送被应用退出或中断，无法确定是否送达；请检查收件箱后决定是否重新发送' WHERE state='sending'",[])?;
    Ok(())
}
struct Pending {
    id: i64,
    config: Config,
    secret: String,
    attempts: u32,
}
fn claim(runtime: &Runtime) -> AppResult<Option<Pending>> {
    runtime.read(|store,secrets| {
        let tx=store.connection.transaction()?;
        let config=load(&tx)?;
        if !config.enabled || !config.verified {cancel(&tx)?;tx.commit()?;return Ok(None);}
        tx.execute("UPDATE reminder_email_deliveries SET state='cancelled',next_retry_at=NULL WHERE state IN ('pending','retry_wait','failed','uncertain') AND (config_revision<>?1 OR recipient<>?2)",params![config.revision,config.connection.to])?;
        let now=stamp(runtime.clock.sample()?.utc);
        let task:Option<(i64,String,u32)>=tx.query_row("SELECT id,kind,attempts FROM reminder_email_deliveries WHERE state='pending' OR (state='retry_wait' AND next_retry_at<=?1) ORDER BY id LIMIT 1",[&now],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        let Some((id,kind,attempts))=task else {tx.commit()?;return Ok(None);};
        if !rebuild(&tx,id,kind=="offline_summary")? {tx.commit()?;return Ok(None);}
        let secret=encrypted_secret(&tx)?.ok_or_else(||AppError::new("EMAIL_NOT_VERIFIED","缺少邮箱授权码，请重新验证"))?;
        let secret=secrets.unprotect(&secret)?;
        tx.execute("UPDATE reminder_email_deliveries SET state='sending',attempts=attempts+1,next_retry_at=NULL,error=NULL WHERE id=?1",[id])?;
        tx.commit()?;Ok(Some(Pending{id,config,secret,attempts:attempts+1}))
    })
}
fn finish(
    c: &Connection,
    id: i64,
    revision: u64,
    attempts: u32,
    result: Result<(), transport::Failure>,
    now: DateTime<Utc>,
) -> AppResult<()> {
    // 全量导入会删除队列；条件更新防止异步旧结果污染导入后的新ID。
    let (state, error, next, sent) = match result {
        Ok(()) => ("sent", None, None, Some(stamp(now))),
        Err(error) => {
            let state = if error.uncertain {
                "uncertain"
            } else if error.retryable && attempts < 3 {
                "retry_wait"
            } else {
                "failed"
            };
            let next = (state == "retry_wait").then(|| {
                stamp(now + chrono::Duration::seconds(if attempts == 1 { 60 } else { 300 }))
            });
            (state, Some(error.message), next, None)
        }
    };
    c.execute("UPDATE reminder_email_deliveries SET state=?1,error=?2,next_retry_at=?3,sent_at=?4 WHERE id=?5 AND config_revision=?6 AND state='sending'",params![state,error,next,sent,id,revision])?;
    Ok(())
}
/// 人工重发必须显式确认uncertain，不能由页面刷新或自动恢复触发。
pub fn retry(
    c: &mut Connection,
    id: i64,
    revision: u64,
    confirm_uncertain: bool,
) -> AppResult<View> {
    let tx = c.transaction()?;
    let config = load(&tx)?;
    if !config.enabled || !config.verified || config.revision != revision {
        return Err(AppError::new(
            "EMAIL_NOT_VERIFIED",
            "请先验证并开启当前邮件配置",
        ));
    }
    let (state, task_revision): (String, u64) = tx
        .query_row(
            "SELECT state,config_revision FROM reminder_email_deliveries WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if task_revision != revision || !["failed", "uncertain"].contains(&state.as_str()) {
        return Err(AppError::new(
            "EMAIL_RETRY_INVALID",
            "此任务不支持重发，旧配置下的邮件不会恢复",
        ));
    }
    if state == "uncertain" && !confirm_uncertain {
        return Err(AppError::new(
            "EMAIL_CONFIRM_RESEND",
            "请先确认收件箱；重发可能收到重复邮件",
        ));
    }
    tx.execute("UPDATE reminder_email_deliveries SET state='pending',attempts=0,error=NULL,next_retry_at=NULL WHERE id=?1",[id])?;
    let result = view(&tx)?;
    tx.commit()?;
    Ok(result)
}
struct Running(Arc<Service>);
impl Drop for Running {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::Release);
    }
}
pub(crate) fn tick(runtime: &Runtime) {
    if runtime
        .reminder_email
        .running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let runtime = runtime.clone();
    tauri::async_runtime::spawn(async move {
        let _running = Running(runtime.reminder_email.clone());
        let result:AppResult<()>=async {
            if !runtime.reminder_email.recovered.load(Ordering::Acquire) {
                runtime.read(|store,_|recover(&store.connection))?;
                runtime.reminder_email.recovered.store(true,Ordering::Release);
            }
            if runtime.jobs_cancel.check().is_err(){return Ok(());}
            if let Some(task)=claim(&runtime)? {
                // 出锁后立即复核配置；网络期间关闭不能撤回已提交给SMTP的邮件。
                let valid=runtime.read(|store,_|{let c=load(&store.connection)?;Ok(c.enabled && c.verified && c.revision==task.config.revision && c.connection==task.config.connection)})?;
                if !valid {runtime.read(|store,_|{store.connection.execute("UPDATE reminder_email_deliveries SET state='cancelled' WHERE id=?1 AND config_revision=?2 AND state='sending'",params![task.id,task.config.revision])?;Ok(())})?;return Ok(());}
                let result=transport::send(&task.config.connection,&task.secret,|| {
                    runtime.read(|store,_| {
                        let tx=store.connection.transaction()?;
                        let current=load(&tx)?;
                        let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM reminder_email_deliveries WHERE id=?1 AND config_revision=?2 AND state='sending')",params![task.id,task.config.revision],|r|r.get(0))?;
                        let valid=active && current.enabled && current.verified && current.revision==task.config.revision && current.connection==task.config.connection;
                        if !valid {
                            tx.execute("UPDATE reminder_email_deliveries SET state='cancelled' WHERE id=?1 AND config_revision=?2 AND state='sending'",params![task.id,task.config.revision])?;
                            tx.commit()?;return Err(AppError::new("EMAIL_CANCELLED","配置已更改，邮件任务已取消"));
                        }
                        let kind:String=tx.query_row("SELECT kind FROM reminder_email_deliveries WHERE id=?1",[task.id],|r|r.get(0))?;
                        if !rebuild(&tx,task.id,kind=="offline_summary")? {tx.commit()?;return Err(AppError::new("EMAIL_CANCELLED","提醒已删除，邮件任务已取消"));}
                        let message=tx.query_row("SELECT subject,body FROM reminder_email_deliveries WHERE id=?1",[task.id],|r|Ok((r.get(0)?,r.get(1)?)))?;
                        tx.commit()?;Ok(message)
                    }).map_err(|e|transport::Failure::permanent(&e.message))
                }).await;
                runtime.read(|store,_|finish(&store.connection,task.id,task.config.revision,task.attempts,result,runtime.clock.sample()?.utc))?;
            }
            Ok(())
        }.await;
        if let Err(error) = result {
            eprintln!("邮件提醒后台处理失败：{}", error.message);
        }
    });
}

#[cfg(test)]
pub(super) fn test_claim(runtime: &Runtime) -> AppResult<Option<(i64, String)>> {
    let Some(task) = claim(runtime)? else {
        return Ok(None);
    };
    let body = runtime.read(|store, _| {
        Ok(store.connection.query_row(
            "SELECT body FROM reminder_email_deliveries WHERE id=?1",
            [task.id],
            |r| r.get(0),
        )?)
    })?;
    Ok(Some((task.id, body)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_stop_after_three_and_uncertain_never_auto_retries() {
        let runtime = super::super::tests::verified();
        runtime.read(|s,_| {
            let now=Utc::now()+chrono::Duration::seconds(1);
            let occurrence=super::super::tests::occurrence(&s.connection,"提醒",&stamp(now));
            enqueue(&s.connection,&[occurrence],false,now)?;
            let id=s.connection.last_insert_rowid();
            for attempts in 1..=3 {
                s.connection.execute("UPDATE reminder_email_deliveries SET state='sending',attempts=?1 WHERE id=?2",params![attempts,id])?;
                finish(&s.connection,id,1,attempts,Err(transport::Failure{message:"连接失败".into(),retryable:true,uncertain:false}),now)?;
                let status=view(&s.connection)?;
                assert_eq!(status.deliveries[0].state,if attempts<3 {"retry_wait"} else {"failed"});
                if attempts<3 {assert_eq!(status.deliveries[0].next_retry_at,Some(stamp(now+chrono::Duration::seconds(if attempts==1 {60}else{300}))));}
            }
            s.connection.execute("UPDATE reminder_email_deliveries SET state='sending' WHERE id=?1",[id])?;
            finish(&s.connection,id,1,3,Err(transport::Failure{message:"结果不确定".into(),retryable:false,uncertain:true}),now)?;
            assert_eq!(view(&s.connection)?.deliveries[0].state,"uncertain");Ok(())
        }).unwrap();
    }
}
