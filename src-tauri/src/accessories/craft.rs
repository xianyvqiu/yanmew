//! 制作候选只在本次运行中存在；确认才持久化饰品、钱包、流水与档位。
use crate::{
    ai::provider::ToolCall,
    currency::{self, CoinChange},
    error::{parse_id, AppError, AppResult},
    events::ChatToolEventSink,
    image_generation,
    memory::management::Context,
    models::{Accessory, CurrencyReason},
    runtime::{Runtime, Store},
    state::{Cancellation, RequestLease},
};
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg(test)]
mod tests;
pub const RULES:&str="饰品制作只在适合互动且达到当前可用档位时按需发起，不必每轮调用。先使用尚未制作的最低档，前三档各一次，第四档可重复。request_accessory_craft只准备候选，用户确认后才会购买，不能声称已经扣币、制作完成或佩戴。display_text必须是少量实际图标字符、Emoji、颜文字或符号组合，不能填写普通名称/说明，name保存名称，description用当前人设口吻写简介。生图可在准备预览时消耗用户服务商额度，即使未购买；不能承诺取消会撤销费用。不要擅自替用户确认或取消，不得请求任意币数、路径或换用其他生图服务。";
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Text {
    pub(crate) name: String,
    pub(crate) display_text: String,
    pub(crate) description: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Preparing,
    Ready,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Appearance {
    Image,
    Text,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Quote {
    tier: u8,
    cost: i64,
}
#[derive(Clone)]
struct Candidate {
    id: String,
    context: Context,
    quote: Quote,
    text: Text,
    status: Status,
    image_file: Option<String>,
    notice: Option<String>,
    image_revision: Option<u64>,
    uploaded:bool,
    requires_reconfirm: bool,
    cancel: Cancellation,
    events: Arc<dyn ChatToolEventSink>,
}
#[derive(Default)]
pub(crate) struct State {
    candidate: Option<Candidate>,
    next: u64,
    request_id: Option<u64>,
    calls: HashMap<String, (Text, String)>,
    refused_until: HashMap<i64, Instant>,
}
impl State {
    pub(crate) fn references(&self, path: &str) -> bool {
        self.candidate
            .as_ref()
            .is_some_and(|c| c.image_file.as_deref() == Some(path))
    }
    pub(crate) fn discard(&mut self) {
        if let Some(candidate) = self.candidate.take() {
            candidate.cancel.cancel();
        }
        self.calls.clear();
        self.request_id = None;
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub preview_id: String,
    pub context: Context,
    pub tier: u8,
    pub cost: String,
    pub name: String,
    pub display_text: String,
    pub description: String,
    pub status: Status,
    pub appearance: Appearance,
    pub uploaded:bool,
    pub image_data_url: Option<String>,
    pub notice: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub preview_id: String,
    pub status: Status,
}
fn expired() -> AppError {
    AppError::new(
        "ACCESSORY_PREVIEW_EXPIRED",
        "饰品预览已失效，请根据当前会话和档位重新准备",
    )
}
fn context_ids(context: &Context) -> AppResult<(i64, i64)> {
    Ok((
        parse_id(&context.persona_id)?,
        parse_id(&context.session_id)?,
    ))
}
fn quote(c: &Connection, session: i64) -> AppResult<Option<Quote>> {
    let (intimacy,a,b,d):(i64,bool,bool,bool)=c.query_row("SELECT intimacy,craft_tier_1_used,craft_tier_2_used,craft_tier_3_used FROM sessions WHERE id=?1",[session],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    let settings = crate::config::get(c)?;
    for (i, used) in [a, b, d, false].iter().enumerate() {
        let tier = i + 1;
        let threshold = settings[&format!("intimacy_threshold_{tier}")]
            .parse::<i64>()
            .map_err(|_| AppError::storage())?;
        if !used && intimacy >= threshold {
            let cost = settings[&format!("accessory_cost_{tier}")]
                .parse::<i64>()
                .map_err(|_| AppError::storage())?;
            return Ok(Some(Quote {
                tier: tier as u8,
                cost,
            }));
        }
    }
    Ok(None)
}
pub(crate) fn validate_text(text: &Text) -> AppResult<()> {
    if text.name.trim().is_empty() || text.name.chars().count() > 80 {
        return Err(AppError::validation("name", "饰品名称须为1–80个字符"));
    }
    if text.description.trim().is_empty() || text.description.chars().count() > 1000 {
        return Err(AppError::validation(
            "description",
            "请用人设口吻写非空简介，最多1000个字符",
        ));
    }
    let display = text.display_text.trim();
    let visual = display.chars().any(|c| {
        !c.is_alphanumeric()
            && !c.is_whitespace()
            && !r#"()（）[]{}<>.,，。!！?？:：;；\"'"#.contains(c)
    });
    let word = display
        .chars()
        .any(|c| matches!(c as u32,0x3400..=0x9fff|0x20000..=0x3134f));
    if display.is_empty()
        || display.chars().count() > 40
        || display.chars().any(char::is_control)
        || !(visual
            || (display.chars().all(|c| !c.is_alphanumeric())
                && display.chars().any(|c| c.is_ascii_punctuation())))
        || word
        || display == text.name.trim()
        || display == text.description.trim()
    {
        return Err(AppError::validation(
            "display_text",
            "外观须为图标字符、Emoji、颜文字或符号组合，不能用普通名称/说明代替",
        ));
    }
    Ok(())
}
fn require_candidate(store: &Store, context: &Context, id: &str) -> AppResult<Candidate> {
    let (persona, session) = context_ids(context)?;
    super::require_current(store, persona, session, context.context_revision).map_err(|e| {
        if e.code == "CONTEXT_CHANGED" {
            expired()
        } else {
            e
        }
    })?;
    let candidate = store
        .accessory_crafts
        .candidate
        .as_ref()
        .filter(|c| c.id == id && c.context == *context)
        .ok_or_else(expired)?;
    if quote(&store.connection, session)? != Some(candidate.quote) {
        return Err(expired());
    }
    Ok(candidate.clone())
}
fn notify(candidate: &Candidate) {
    candidate
        .events
        .accessory_preview_changed(&candidate.context.session_id, &candidate.id);
}
fn preview(candidate: &Candidate, image: Option<String>) -> Preview {
    Preview {
        preview_id: candidate.id.clone(),
        uploaded:candidate.uploaded,
        context: candidate.context.clone(),
        tier: candidate.quote.tier,
        cost: candidate.quote.cost.to_string(),
        name: candidate.text.name.clone(),
        display_text: candidate.text.display_text.clone(),
        description: candidate.text.description.clone(),
        status: candidate.status,
        appearance: if candidate.image_file.is_some() {
            Appearance::Image
        } else {
            Appearance::Text
        },
        image_data_url: image,
        notice: candidate.notice.clone(),
    }
}
pub(crate) fn definition() -> Value {
    json!({"type":"function","function":{"name":"request_accessory_craft","description":"条件满足且互动适合时，请求准备饰品预览；用户确认才扣喵币/使用档位。外观必须是图标字符、Emoji、颜文字或符号，简介使用当前人设口吻。",
    "parameters":{"type":"object","properties":{"name":{"type":"string","maxLength":80},"display_text":{"type":"string","maxLength":40,"description":"实际视觉符号，例如🎀、👑、✿；不是饰品名称"},"description":{"type":"string","maxLength":1000,"description":"用当前人设口吻写饰品简介"}},"required":["name","display_text","description"],"additionalProperties":false}}})
}
pub(crate) fn hint(store: &Store, session: i64) -> String {
    let available = quote(&store.connection, session).ok().flatten();
    let cooldown = store
        .accessory_crafts
        .refused_until
        .get(&session)
        .is_some_and(|until| *until > Instant::now());
    json!({"canRequest":available.is_some() && !cooldown && store.accessory_crafts.candidate.is_none(),"nextTier":available.map(|q|q.tier),"cost":available.map(|q|q.cost.to_string()),"refusalCooldown":cooldown,"pendingPreview":store.accessory_crafts.candidate.is_some()}).to_string()
}
/// 工具入口校验活动租约并复用同一调用ID；网络图片任务不等待聊天正文。
pub(crate) fn request(
    runtime: &Runtime,
    lease: &Arc<RequestLease>,
    context: Context,
    call: &ToolCall,
    events: Arc<dyn ChatToolEventSink>,
) -> AppResult<Receipt> {
    let text: Text = serde_json::from_value(call.arguments.clone())
        .map_err(|_| AppError::validation("arguments", "饰品制作参数无效"))?;
    validate_text(&text)?;
    let (persona, session) = context_ids(&context)?;
    let (receipt, candidate, job) = runtime.read(|store, secrets| {
        lease.require_conversation_scope(&runtime.requests, session, persona)?;
        runtime.jobs_cancel.check()?;
        super::require_current(store, persona, session, context.context_revision)?;
        if store.accessory_crafts.request_id != Some(lease.id) {
            store.accessory_crafts.calls.clear();
            store.accessory_crafts.request_id = Some(lease.id);
        }
        if let Some((old, id)) = store.accessory_crafts.calls.get(&call.id) {
            if old != &text {
                return Err(AppError::new(
                    "TOOL_CALL_CONFLICT",
                    "同一制作调用ID的参数已改变",
                ));
            }
            let c = require_candidate(store, &context, id)?;
            return Ok((
                Receipt {
                    preview_id: c.id.clone(),
                    status: c.status,
                },
                None,
                None,
            ));
        }
        if let Some(old) = store.accessory_crafts.candidate.as_ref() {
            if require_candidate(store, &old.context, &old.id).is_ok() {
                return Err(AppError::new(
                    "REQUEST_BUSY",
                    "已有待确认饰品，请先确认或取消",
                ));
            }
            if let Some(old) = store.accessory_crafts.candidate.take() {
                old.cancel.cancel();
            }
        }
        if store
            .accessory_crafts
            .refused_until
            .get(&session)
            .is_some_and(|until| *until > Instant::now())
        {
            return Err(AppError::new(
                "ACCESSORY_CRAFT_COOLDOWN",
                "用户已暂不制作，至少等待一小时或下次启动后再请求",
            ));
        }
        if store.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",
            [session],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::new(
                "ACCESSORY_UNAVAILABLE",
                "出门期间不能请求制作饰品",
            ));
        }
        let quote = quote(&store.connection, session)?.ok_or_else(|| {
            AppError::new("ACCESSORY_NOT_UNLOCKED", "当前没有已解锁且未使用的制作档位")
        })?;
        let snapshot = image_generation::snapshot(&store.connection, secrets);
        store.accessory_crafts.next = store
            .accessory_crafts
            .next
            .checked_add(1)
            .ok_or_else(AppError::storage)?;
        let id = format!("accessory-{}-{}", lease.id, store.accessory_crafts.next);
        let (job, status, notice, cancel, revision) = match snapshot {
            Ok(Some(snapshot)) => {
                let image_lease = Arc::new(
                    runtime
                        .requests
                        .start(format!("accessory-image:{id}"), persona)?,
                );
                let cancellation = image_lease.cancel.clone();
                let revision = snapshot.revision;
                (
                    Some((snapshot, image_lease)),
                    Status::Preparing,
                    Some(
                        "正在准备图片；服务商可能计费，与喵币无关，即使未购买也可能产生费用".into(),
                    ),
                    cancellation,
                    Some(revision),
                )
            }
            Ok(None) => (
                None,
                Status::Ready,
                Some("生图未开启或未配置，本次使用所示图标、Emoji、颜文字或符号组合".into()),
                Cancellation::default(),
                None,
            ),
            Err(_) => (
                None,
                Status::Ready,
                Some("生图配置或Key不可用，本次使用所示图标、Emoji、颜文字或符号组合".into()),
                Cancellation::default(),
                None,
            ),
        };
        let candidate = Candidate {
            id: id.clone(),
            context: context.clone(),
            quote,
            text: text.clone(),
            status,
            image_file: None,
            notice,
            image_revision: revision,uploaded:false,
            requires_reconfirm: false,
            cancel,
            events,
        };
        store
            .accessory_crafts
            .calls
            .insert(call.id.clone(), (text.clone(), id.clone()));
        store.accessory_crafts.candidate = Some(candidate.clone());
        Ok((
            Receipt {
                preview_id: id,
                status,
            },
            Some(candidate),
            job,
        ))
    })?;
    super::images::cleanup_unused(runtime, &[]);
    if let Some(candidate) = candidate {
        notify(&candidate);
        if let Some((snapshot, lease)) = job {
            let worker = runtime.clone();
            tauri::async_runtime::spawn(async move {
                let result = match worker.ai.as_ref() {
                    Ok(client) => {
                        image_generation::openai_compatible::generate(
                            client,
                            &snapshot,
                            &candidate.text.name,
                            &candidate.text.description,
                            &lease.cancel,
                        )
                        .await
                    }
                    Err(error) => Err(error.clone()),
                };
                let cutout_paths = worker.paths.clone(); let cutout_cancel = lease.cancel.clone();
                let result = match result {
                    Ok(bytes) => tauri::async_runtime::spawn_blocking(move || {
                        let bytes = crate::background_removal::ensure(&cutout_paths?, &bytes, &cutout_cancel, true)?;
                        super::images::normalize(&bytes)
                    })
                    .await
                    .map_err(|_| AppError::storage())
                    .and_then(|r| r),
                    Err(error) => Err(error),
                };
                let update = finish_image(&worker, &candidate.id, result);
                if let Ok(Some(update)) = update {
                    notify(&update);
                }
                drop(lease);
                super::images::cleanup_unused(&worker, &[]);
            });
        }
    }
    Ok(receipt)
}
fn finish_image(
    worker: &Runtime,
    candidate_id: &str,
    result: AppResult<Vec<u8>>,
) -> AppResult<Option<Candidate>> {
    worker.read(|store, _| {
        let Some(current) = store
            .accessory_crafts
            .candidate
            .as_ref()
            .filter(|c| c.id == candidate_id && c.status == Status::Preparing)
            .cloned()
        else {
            return Ok(None);
        };
        if require_candidate(store, &current.context, &current.id).is_err()
            || worker.jobs_cancel.check().is_err()
        {
            if let Some(old) = store.accessory_crafts.candidate.take() {
                old.cancel.cancel();
            }
            return Ok(None);
        }
        let valid = current.cancel.check().is_ok()
            && image_generation::revision(&store.connection)
                .is_ok_and(|rev| Some(rev) == current.image_revision);
        let file = if valid {
            result.and_then(|png| {
                worker
                    .paths
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|paths| super::images::publish(paths, &png))
            })
        } else {
            Err(AppError::new("CANCELLED", "生图配置已改变或请求取消"))
        };
        let active = store.accessory_crafts.candidate.as_mut().unwrap();
        active.status = Status::Ready;
        match file {
            Ok(file) => {
                active.image_file = Some(file);
                active.notice = Some(
                    "透明图片已准备。服务商的API费用与喵币独立，取消预览不保证退还API费用".into(),
                );
            }
            Err(error) => {
                active.image_file = None;
                active.notice = Some(format!(
                    "图片未生成：{}。将显示所示图标、Emoji、颜文字或符号组合；本次未自动重试",
                    error.message
                ));
            }
        }
        Ok(Some(active.clone()))
    })
}
/// 配置提交后仅使未完成图片退回符号预览；已就绪图片仍可确认，不重新向新服务请求。
pub(crate) fn configuration_changed(store: &mut Store) -> Option<(String, String)> {
    let candidate = store.accessory_crafts.candidate.as_mut()?;
    if candidate.status != Status::Preparing {
        return None;
    }
    candidate.cancel.cancel();
    candidate.status = Status::Ready;
    candidate.notice = Some(
        "生图配置已改变，旧请求取消，本次使用所示图标、Emoji、颜文字或符号组合；服务商仍可能计费"
            .into(),
    );
    Some((candidate.context.session_id.clone(), candidate.id.clone()))
}
pub(crate) fn get(runtime: &Runtime, context: &Context, id: &str) -> AppResult<Preview> {
    let mut candidate = runtime.read(|store, _| require_candidate(store, context, id))?;
    let image = candidate.image_file.as_deref().map(|file| {
        runtime
            .paths
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|paths| super::images::read(paths, file))
    });
    let data = match image {
        Some(Ok(image)) => Some(image),
        Some(Err(_)) => {
            if candidate.uploaded{return Err(AppError::new("ACCESSORY_IMAGE_UNAVAILABLE","上传图片已丢失或损坏，请取消后重新上传；未扣币"));}
            runtime.read(|store, _| {
                require_candidate(store, context, id)?;
                let active = store.accessory_crafts.candidate.as_mut().unwrap();
                active.image_file = None;
                active.status = Status::Ready;
                active.requires_reconfirm = true;
                active.notice = Some(
                    "图片缺失或损坏，本次显示所示图标、Emoji、颜文字或符号组合；请重新查看并确认"
                        .into(),
                );
                candidate = active.clone();
                Ok(())
            })?;
            notify(&candidate);
            super::images::cleanup_unused(runtime, &[]);
            None
        }
        None => None,
    };
    runtime.read(|store, _| {
        let current = require_candidate(store, context, id)?;
        if current.image_file != candidate.image_file || current.status != candidate.status {
            return Err(expired());
        }
        Ok(())
    })?;
    Ok(preview(&candidate, data))
}
pub(crate) fn current(runtime: &Runtime, context: &Context) -> AppResult<Option<Preview>> {
    let (persona, session) = context_ids(context)?;
    let id = runtime.read(|store, _| {
        super::require_current(store, persona, session, context.context_revision)?;
        Ok(store
            .accessory_crafts
            .candidate
            .as_ref()
            .map(|c| c.id.clone()))
    })?;
    let Some(id) = id else { return Ok(None) };
    match get(runtime, context, &id) {
        Ok(preview) => Ok(Some(preview)),
        Err(error) if error.code == "ACCESSORY_PREVIEW_EXPIRED" => {
            runtime.read(|store, _| {
                super::require_current(store, persona, session, context.context_revision)?;
                if store
                    .accessory_crafts
                    .candidate
                    .as_ref()
                    .is_some_and(|c| c.id == id)
                {
                    if let Some(candidate) = store.accessory_crafts.candidate.take() {
                        candidate.cancel.cancel();
                    }
                }
                Ok(())
            })?;
            super::images::cleanup_unused(runtime, &[]);
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
pub(crate) fn confirm(runtime: &Runtime, context: &Context, id: &str) -> AppResult<Accessory> {
    let before = runtime.read(|store, _| require_candidate(store, context, id))?;
    if before.status == Status::Preparing {
        return Err(AppError::new(
            "REQUEST_BUSY",
            "饰品图片仍在准备，请等预览就绪后再确认",
        ));
    }
    if before.requires_reconfirm {
        runtime.read(|store, _| {
            require_candidate(store, context, id)?;
            store
                .accessory_crafts
                .candidate
                .as_mut()
                .unwrap()
                .requires_reconfirm = false;
            Ok(())
        })?;
        return Err(AppError::new(
            "ACCESSORY_IMAGE_UNAVAILABLE",
            "外观已改为符号预览，未扣币；请查看后再次确认",
        ));
    }
    let image_ok = before.image_file.as_deref().map_or(true, |file| {
        runtime
            .paths
            .as_ref()
            .is_ok_and(|paths| super::images::read(paths, file).is_ok())
    });
    if !image_ok {
        if before.uploaded{return Err(AppError::new("ACCESSORY_IMAGE_UNAVAILABLE","上传图片已丢失或损坏，未扣币；请重新上传"));}
        runtime.read(|store, _| {
            require_candidate(store, context, id)?;
            let candidate = store.accessory_crafts.candidate.as_mut().unwrap();
            candidate.image_file = None;
            candidate.notice = Some(
                "图片缺失或损坏，已改为所示图标、Emoji、颜文字或符号组合；请查看预览后重新确认"
                    .into(),
            );
            Ok(())
        })?;
        let changed = runtime.read(|s, _| Ok(s.accessory_crafts.candidate.clone()))?;
        if let Some(changed) = changed {
            notify(&changed);
        }
        super::images::cleanup_unused(runtime, &[]);
        return Err(AppError::new(
            "ACCESSORY_IMAGE_UNAVAILABLE",
            "图片已不可用，未扣币；请查看符号预览后重新确认",
        ));
    }
    runtime.read(|store,_| {
        runtime.jobs_cancel.check()?;
        let candidate=require_candidate(store,context,id)?;
        if candidate.image_file!=before.image_file || candidate.status!=Status::Ready{return Err(expired());}
        let session=parse_id(&context.session_id)?;
        let mut tx=store.connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if quote(&tx,session)?!=Some(candidate.quote){return Err(expired());}
        let at=runtime.clock.sample()?.utc;
        tx.execute("INSERT INTO accessories(session_id,name,display_text,image_file,description,cost,source,created_at) VALUES(?1,?2,?3,?4,?5,?6,'crafted',?7)",params![session,candidate.text.name.trim(),candidate.text.display_text.trim(),candidate.image_file,candidate.text.description.trim(),candidate.quote.cost,at.to_rfc3339_opts(chrono::SecondsFormat::Millis,true)])?;
        let item_id=tx.last_insert_rowid();
        currency::apply_delta_in_tx(&mut tx,&CoinChange {delta:-candidate.quote.cost,reason:CurrencyReason::AccessoryCraft,ref_id:Some(item_id),plugin:None},at)?;
        if candidate.quote.tier<=3 {tx.execute(&format!("UPDATE sessions SET craft_tier_{}_used=1 WHERE id=?1",candidate.quote.tier),[session])?;}
        let item=super::get(&tx,session,item_id)?;tx.commit()?;
        store.accessory_crafts.candidate=None;Ok(item)
    })
}
pub(crate) fn cancel(runtime: &Runtime, context: &Context, id: &str) -> AppResult<()> {
    let candidate = runtime.read(|store, _| {
        let (persona, session) = context_ids(context)?;
        super::require_current(store, persona, session, context.context_revision)?;
        let candidate = store
            .accessory_crafts
            .candidate
            .as_ref()
            .filter(|c| c.id == id && c.context == *context)
            .cloned()
            .ok_or_else(expired)?;
        candidate.cancel.cancel();
        store.accessory_crafts.candidate = None;
        store
            .accessory_crafts
            .refused_until
            .insert(session, Instant::now() + Duration::from_secs(3600));
        Ok(candidate)
    })?;
    notify(&candidate);
    super::images::cleanup_unused(runtime, &[]);
    Ok(())
}

/// 用户主动上传，只准备候选；复用确认事务扣币，不启动任何生图请求。
pub(crate) fn upload(runtime:&Runtime,context:Context,name:String,description:String,bytes:Vec<u8>,events:Arc<dyn ChatToolEventSink>)->AppResult<Preview>{
    let png=super::images::normalize(&bytes)?;
    let text=Text{name,description,display_text:"✦".into()};validate_text(&text)?;
    let candidate=runtime.read(|s,_|{
        let persona=parse_id(&context.persona_id)?;let session=parse_id(&context.session_id)?;
        super::require_current(s,persona,session,context.context_revision)?;
        if s.accessory_crafts.candidate.is_some(){return Err(AppError::new("REQUEST_BUSY","请先确认或取消当前挂件候选"));}
        if s.connection.query_row("SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",[session],|r|r.get::<_,bool>(0))?{return Err(AppError::new("ACCESSORY_UNAVAILABLE","出行期间不能制作挂件"));}
        let quote=quote(&s.connection,session)?.ok_or_else(||AppError::new("ACCESSORY_UNAVAILABLE","当前没有可用制作档位，请先提升亲密度"))?;
        let next=s.accessory_crafts.next.checked_add(1).ok_or_else(AppError::storage)?;
        let file=super::images::publish(runtime.paths.as_ref().map_err(Clone::clone)?,&png)?;
        let candidate=Candidate{id:format!("accessory-upload-{next}"),context:context.clone(),quote,text:text.clone(),status:Status::Ready,image_file:Some(file),notice:Some("手动上传透明图片，不调用生图服务；确认后按当前档位扣喵币。".into()),image_revision:None,uploaded:true,requires_reconfirm:false,cancel:Cancellation::default(),events:events.clone()};
        s.accessory_crafts.next=next;s.accessory_crafts.candidate=Some(candidate.clone());Ok(candidate)
    })?;notify(&candidate);get(runtime,&context,&candidate.id)
}
