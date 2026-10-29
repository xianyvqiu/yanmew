//! 本轮人设调整候选：工具只暂存，唯一完成入口在回复提交后尝试保存。
use super::*;
use crate::{ai::provider::ToolCall, events::PersonaUpdated};
use serde_json::{json, Value};

pub(super) struct Evolution {
    expected: crate::persona::EvolutionSnapshot,
    candidate: Option<String>,
}

impl Evolution {
    pub(super) fn new(expected: crate::persona::EvolutionSnapshot) -> Self {
        Self {
            expected,
            candidate: None,
        }
    }

    pub(super) fn stage(&mut self, call: &ToolCall) -> AppResult<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            new_prompt: String,
        }
        let input: Input = serde_json::from_value(call.arguments.clone())
            .map_err(|_| AppError::validation("new_prompt", "只接受 new_prompt 字符串参数"))?;
        if input.new_prompt.trim().is_empty() {
            return Err(AppError::validation("new_prompt", "人设设定不能为空"));
        }
        if input.new_prompt.trim() == self.expected.prompt.trim() {
            return Err(AppError::new(
                "PERSONA_NO_CHANGE",
                "设定没有实质变化，不需要调整",
            ));
        }
        if let Some(candidate) = &self.candidate {
            if candidate != &input.new_prompt {
                return Err(AppError::new(
                    "PERSONA_EVOLUTION_ALREADY_STAGED",
                    "本轮已有调整候选，不重复改写",
                ));
            }
            return Ok(json!({"staged":true,"already_staged":true,"saved":false}));
        }
        self.candidate = Some(input.new_prompt);
        Ok(json!({"staged":true,"already_staged":false,"saved":false,
            "message":"调整已提出，尚未生效。"}))
    }

    pub(super) fn commit(
        &self,
        prepared: &PreparedReply,
        store: &mut crate::runtime::Store,
    ) -> AppResult<Option<PersonaUpdated>> {
        let Some(candidate) = &self.candidate else {
            return Ok(None);
        };
        prepared.runtime.jobs_cancel.check()?;
        prepared.lease.require_scope(
            &prepared.runtime.requests,
            &format!("chat:{}", prepared.session_id),
            prepared.persona_id,
        )?;
        coordinator::require_context(
            store,
            prepared.persona_id,
            prepared.session_id,
            prepared.context_revision,
        )?;
        store.pet.require_ai()?;
        let evolved_at = prepared
            .runtime
            .clock
            .sample()?
            .utc
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        if !crate::persona::apply_evolution(
            &mut store.connection,
            &self.expected,
            candidate,
            &evolved_at,
        )? {
            return Ok(None);
        }
        Ok(Some(PersonaUpdated {
            persona_id: prepared.persona_id.to_string(),
            session_id: prepared.session_id.to_string(),
            context_revision: prepared.context_revision,
            evolved_at,
        }))
    }
}

pub(super) fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"update_persona_prompt",
        "description":"默认保持人设。仅当持续交流揭示重要、明确且确需修正的设定问题时，才作最小幅度的渐进调整；不确定就不调用。保留身份、核心性格、价值观、背景和其他性格侧面，不突然反转，也不持续放大某种特质至极端。一次语气反馈可先影响当次表达：冷淡慢热的角色被说“别太冷漠”时，可逐步缓和生硬措辞，仍保持克制慢热。日常情绪、玩笑、零碎偏好、用户资料和聊天记忆不写入人设。每轮最多提出一份调整，初始设定不变。结果为待提交，完整回复成功后才生效。",
        "parameters":{"type":"object","properties":{"new_prompt":{"type":"string","description":"调整后的完整人设，须包含未改动的其余内容，而非局部补丁"}},"required":["new_prompt"],"additionalProperties":false}
    }})
}
