//! 主动生成复用聊天推理，但没有用户输入、回合或唤醒互动。
use super::*;

pub(crate) fn prepare_proactive(runtime: &Runtime, store: &mut crate::runtime::Store,
    secrets: &dyn crate::platform::contracts::SecretProtector, persona_id: i64, session_id: i64,
    lease: Arc<RequestLease>) -> AppResult<PreparedReply> {
    const INSTRUCTION: &str = "当前会话处于无聊状态，请按人设主动发起一条自然、简短的话题。此次没有新的用户消息，不要声称用户刚刚问了问题；结合已有上下文，不编造经历，不要求用户必须回复。必要时可使用现有工具查阅资料。";
    runtime.ai.as_ref().map_err(Clone::clone)?;
    let health = store.pet.health(&store.connection, secrets)?;
    store.pet.require_ai()?;
    let prompt: String = store.connection.query_row("SELECT prompt FROM personas WHERE id=?1", [persona_id], |r|r.get(0))?;
    let provider = store.connection.query_row("SELECT id FROM provider_effective WHERE is_active=1", [], |r|r.get(0))?;
    let config = providers::connection_test_config(&store.connection, provider, secrets)?;
    let context = context::Context::proactive(store, runtime, session_id, provider, &prompt, INSTRUCTION)?;
    Ok(PreparedReply { image_message:None, image_description:Arc::new(Mutex::new(None)), persona_id, session_id, context_revision: store.context_revision, prompt, config,
        messages: vec![Message::new("system", INSTRUCTION)], context, continuation:false, regenerating:false,
        proactive:true, evolution:None, lease, runtime:runtime.clone(), health, tool_events:None })
}
