//! 运行期协调：计时锚点和临时表情不落库，业务事实由 pet_store 唯一保存。
use super::{IdleBranch, Interaction, PetWriteScope, RandomSource, Tick};
use crate::{
    error::{parse_id, AppError, AppResult},
    platform::contracts::{ClockSample, SecretProtector},
};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::Duration;

type ProviderStamp = (i64, Option<String>, String, Option<String>);
#[derive(Clone)]
pub(crate) struct HealthToken {
    stamp: Option<ProviderStamp>,
    revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Normal,
    Hungry,
    Bored,
    Sleeping,
    Eating,
    Interacting,
    Thinking,
    Speaking,
    Away,
    Dormant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub persona_id: String,
    pub session_id: Option<String>,
    pub context_revision: u64,
    pub revision: u64,
    pub activity: Activity,
    pub expression: String,
    pub intimacy: Option<i64>,
    pub sleep_until: Option<String>,
    pub error: Option<String>,
    pub can_interact: bool,
    /// 仅资格提示，阶段 11 才接入真实游走和主动消息任务。
    pub can_wander: bool,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Click,
    Stroke,
    Feed,
}
impl Action {
    pub fn interaction(self) -> Interaction {
        match self {
            Self::Click => Interaction::Click,
            Self::Stroke => Interaction::Stroke,
            Self::Feed => Interaction::Feed,
        }
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InteractionInput {
    pub persona_id: String,
    pub session_id: String,
    pub context_revision: u64,
    pub action: Action,
}

struct Anchor {
    persona_id: String,
    session_id: String,
    context_revision: u64,
    running: Duration,
    idle_allowed: bool,
    effective_allowed: bool,
}
#[derive(Default)]
pub struct Controller {
    anchor: Option<Anchor>,
    pub hidden: bool,
    pub dragging: bool,
    pub stopping: bool,
    transient: Option<(Duration, Activity, String)>,
    pub chat_expression: Option<String>,
    provider: Option<ProviderStamp>,
    provider_initialized: bool,
    health_revision: u64,
    pub provider_error: Option<AppError>,
    last_view: Option<View>,
    expression_key: Option<(String, Activity, Option<String>)>,
    expression: String,
    sequence: u64,
}

impl Controller {
    /// 保存失败时仍执行界面运行许可变化；不把跨越暂停边界的区间误算为空闲。
    pub(crate) fn block_idle_interval(&mut self) {
        if let Some(anchor) = self.anchor.as_mut() {
            anchor.idle_allowed = false;
        }
    }

    pub(crate) fn health(
        &mut self,
        connection: &Connection,
        secrets: &dyn SecretProtector,
    ) -> AppResult<HealthToken> {
        let stamp: Option<ProviderStamp> = connection
            .query_row(
                "SELECT id,base_url,model,api_key_encrypted FROM providers WHERE is_active=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        if !self.provider_initialized || stamp != self.provider {
            self.provider_error = match &stamp {
                None => Some(AppError::new(
                    "PROVIDER_UNAVAILABLE",
                    "暂无聊天提供商，请先在设置中添加并选择",
                )),
                Some(provider) => {
                    crate::providers::connection_test_config(connection, provider.0, secrets).err()
                }
            };
            self.provider = stamp;
            self.provider_initialized = true;
            self.health_revision += 1;
        }
        Ok(HealthToken {
            stamp: self.provider.clone(),
            revision: self.health_revision,
        })
    }

    pub(crate) fn complete_health(&mut self, token: &HealthToken, result: &AppResult<()>) {
        if self.provider != token.stamp || self.health_revision != token.revision {
            return;
        }
        if result
            .as_ref()
            .err()
            .is_some_and(|e| e.code == "CANCELLED" || e.code == "CONTEXT_CHANGED")
        {
            return;
        }
        self.provider_error = result.as_ref().err().cloned();
        self.health_revision += 1;
    }
    pub fn require_ai(&self) -> AppResult<()> {
        if let Some(error) = &self.provider_error {
            return Err(error.clone());
        }
        Ok(())
    }

    pub fn reset_context(&mut self) {
        self.anchor = None;
        self.transient = None;
        self.chat_expression = None;
        self.expression_key = None;
    }

    pub fn settle(
        &mut self,
        connection: &mut Connection,
        revision: u64,
        sample: ClockSample,
        busy: bool,
        random: &mut dyn RandomSource,
    ) -> AppResult<View> {
        if self.stopping {
            return self.last_view.clone().ok_or_else(AppError::storage);
        }
        let context = crate::persona::context(connection, revision)?;
        let (name, library): (String, Option<String>) = connection.query_row(
            "SELECT name,expressions FROM personas WHERE id=?1",
            [&context.persona_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let same = self.anchor.as_ref().is_some_and(|a| {
            a.persona_id == context.persona_id
                && Some(&a.session_id) == context.session_id.as_ref()
                && a.context_revision == revision
        });
        if !same {
            self.reset_context();
        }
        if self
            .transient
            .as_ref()
            .is_some_and(|t| sample.running >= t.0)
        {
            self.transient = None;
        }
        if !busy {
            self.chat_expression = None;
        }
        let idle_allowed = !self.hidden
            && !self.dragging
            && !busy
            && self.transient.is_none()
            && self.provider_error.is_none();
        let mut activity = Activity::Normal;
        let mut intimacy = None;
        let mut sleep_until = None;
        let mut away = false;
        if let Some(session) = &context.session_id {
            let session_id = parse_id(session)?;
            away = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM travels WHERE session_id=?1 AND status='away')",
                [session_id],
                |r| r.get(0),
            )?;
            intimacy = Some(connection.query_row(
                "SELECT intimacy FROM sessions WHERE id=?1 AND persona_id=?2",
                rusqlite::params![session_id, context.persona_id],
                |r| r.get(0),
            )?);
            if !away {
                let snapshot = crate::pet_store::read(connection, session_id)?;
                let elapsed = self
                    .anchor
                    .as_ref()
                    .filter(|a| a.effective_allowed)
                    .map(|a| {
                        sample
                            .running
                            .checked_sub(a.running)
                            .ok_or_else(AppError::storage)
                    })
                    .transpose()?
                    .unwrap_or_default();
                let idle = if idle_allowed && self.anchor.as_ref().is_some_and(|a| a.idle_allowed) {
                    elapsed
                } else {
                    Duration::ZERO
                };
                let result = super::advance(
                    connection,
                    revision,
                    PetWriteScope {
                        persona_id: parse_id(&context.persona_id)?,
                        session_id,
                        context_revision: revision,
                        state_revision: snapshot.state_revision,
                    },
                    Tick {
                        now: sample.utc,
                        effective_elapsed: elapsed,
                        idle_elapsed: idle,
                        allow_idle_transition: idle_allowed,
                    },
                    random,
                )?;
                sleep_until = result.snapshot.sleep_until;
                activity = match result.snapshot.idle_branch {
                    IdleBranch::Sleeping => Activity::Sleeping,
                    _ if result.hungry => Activity::Hungry,
                    IdleBranch::Bored => Activity::Bored,
                    _ => Activity::Normal,
                };
            }
            // 只有保存成功后推进锚点；出门期间仅移动锚点，不写有效累计。
            self.anchor = Some(Anchor {
                persona_id: context.persona_id.clone(),
                session_id: session.clone(),
                context_revision: revision,
                running: sample.running,
                idle_allowed: idle_allowed && !away,
                effective_allowed: !away,
            });
        }
        if busy {
            activity = if self.chat_expression.is_some() {
                Activity::Speaking
            } else {
                Activity::Thinking
            };
        } else if let Some(t) = &self.transient {
            activity = t.1;
        }
        if self.provider_error.is_some() {
            activity = Activity::Dormant;
        }
        if away {
            activity = Activity::Away;
        }
        let key = (context.persona_id.clone(), activity, library.clone());
        if self.expression_key.as_ref() != Some(&key)
            && !matches!(
                activity,
                Activity::Eating | Activity::Interacting | Activity::Speaking | Activity::Away
            )
        {
            self.expression = pick_expression(library.as_deref(), activity, random)?;
            self.expression_key = Some(key);
        }
        let expression = if away {
            format!("{name}出门啦")
        } else if activity == Activity::Speaking {
            self.chat_expression
                .clone()
                .unwrap_or_else(|| self.expression.clone())
        } else if matches!(activity, Activity::Eating | Activity::Interacting) {
            self.transient
                .as_ref()
                .map(|t| t.2.clone())
                .unwrap_or_else(|| self.expression.clone())
        } else {
            self.expression.clone()
        };
        let mut view = View {
            persona_id: context.persona_id,
            session_id: context.session_id,
            context_revision: revision,
            revision: self.sequence,
            activity,
            expression,
            intimacy,
            sleep_until,
            error: self.provider_error.as_ref().map(|e| e.message.clone()),
            can_interact: !away,
            can_wander: idle_allowed
                && !away
                && matches!(
                    activity,
                    Activity::Normal | Activity::Hungry | Activity::Bored
                ),
        };
        view.can_interact &= view.session_id.is_some();
        view.can_wander &= view.session_id.is_some();
        if self.last_view.as_ref() != Some(&view) {
            self.sequence += 1;
            view.revision = self.sequence;
            self.last_view = Some(view.clone());
        }
        Ok(view)
    }

    pub fn interact(
        &mut self,
        connection: &mut Connection,
        revision: u64,
        input: InteractionInput,
        sample: ClockSample,
        random: &mut dyn RandomSource,
    ) -> AppResult<()> {
        let context = crate::persona::context(connection, revision)?;
        if self.stopping
            || context.persona_id != input.persona_id
            || context.session_id.as_ref() != Some(&input.session_id)
            || revision != input.context_revision
        {
            return Err(AppError::new(
                "CONTEXT_CHANGED",
                "当前会话已变化，请重新操作",
            ));
        }
        let session_id = parse_id(&input.session_id)?;
        let snapshot = crate::pet_store::read(connection, session_id)?;
        let activity = if matches!(input.action, Action::Feed) {
            Activity::Eating
        } else {
            Activity::Interacting
        };
        let library: Option<String> = connection.query_row(
            "SELECT expressions FROM personas WHERE id=?1",
            [&input.persona_id],
            |r| r.get(0),
        )?;
        let expression = pick_expression(library.as_deref(), activity, random)?;
        super::interact(
            connection,
            revision,
            PetWriteScope {
                persona_id: parse_id(&input.persona_id)?,
                session_id,
                context_revision: revision,
                state_revision: snapshot.state_revision,
            },
            input.action.interaction(),
            sample.utc,
        )?;
        self.transient = Some((
            sample.running + Duration::from_secs(3),
            activity,
            expression,
        ));
        if let Some(anchor) = &mut self.anchor {
            anchor.running = sample.running;
            anchor.idle_allowed = false;
        }
        Ok(())
    }
}

fn pick_expression(
    raw: Option<&str>,
    activity: Activity,
    random: &mut dyn RandomSource,
) -> AppResult<String> {
    let fallback = match activity {
        Activity::Sleeping => "(-_-) zZ",
        Activity::Bored => "(._.)",
        Activity::Hungry => "(>_<)",
        Activity::Eating => "(๑´ڡ`๑)",
        Activity::Thinking => "(・・?)",
        Activity::Interacting => "(^_^)",
        _ => "(・ω・)",
    };
    let Some(raw) = raw else {
        return Ok(fallback.into());
    };
    let library: crate::expressions::ExpressionLibrary =
        serde_json::from_str(raw).map_err(|_| AppError::storage())?;
    crate::expressions::validate_library(&library)?;
    let choices = match activity {
        Activity::Sleeping => library.sleepy,
        Activity::Bored => library.bored,
        Activity::Hungry => library.hungry,
        Activity::Eating => library.eating,
        Activity::Thinking => library.thinking,
        Activity::Interacting => match random.below(3)? {
            0 => library.click.happy,
            1 => library.click.surprised,
            2 => library.click.shy,
            _ => return Err(AppError::storage()),
        },
        _ => return Ok(fallback.into()),
    };
    let index = random.below(choices.len() as u64)? as usize;
    choices.get(index).cloned().ok_or_else(AppError::storage)
}
