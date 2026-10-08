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
            "message":"仅已暂存候选。完整回复成功保存且人设仍未改变时才提交；现在不要声称已经修改。"}))
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
        "description":"默认保持当前人设；只有持续交流揭示重要且明确的设定问题、确实有必要修正时，才提出调整候选。拿不准必要性时不要调用。不必每轮调用；日常聊天、暂时情绪、玩笑、零碎偏好及不重要的小问题都不修改。用户一句语气反馈不等于持久修改人设的依据，可以先在当次回复中适度缓和表达。确有必要调整时，只针对明确问题作最小幅度、渐进的具体修正，幅度与必要程度相称，保留原身份、核心性格、价值观、背景和其他性格侧面；不得突然反转性格，也不得连续放大某种特质至极端。例如原人设冷淡、慢热，用户说‘不要这么冷漠’，不能立刻改成热情开朗；即使后续确需调整，也只逐步减少生硬措辞、增加少量回应，仍保持克制、慢热的底色。不迎合临时要求、不把用户资料或聊天记忆写进人设、不虚构经历。new_prompt 是保留其余内容的新完整设定，不是补丁。每轮最多一个候选，初始设定不改变。工具只暂存，完整回复成功后才保存；中断/失败就丢弃，勿在回复中声称已保存。",
        "parameters":{"type":"object","properties":{"new_prompt":{"type":"string","description":"确有必要时按程度渐进小幅修正、保留核心性格及其余内容的完整人设设定；不得性格反转或极端化"}},"required":["new_prompt"],"additionalProperties":false}
    }})
}
