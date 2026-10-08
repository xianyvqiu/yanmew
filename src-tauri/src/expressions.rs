use crate::{
    ai::provider::{GenerateRequest, Message},
    error::{AppError, AppResult},
    providers::{self, ConnectionTest},
    runtime::Runtime,
    state::RequestLease,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClickExpressions {
    pub happy: Vec<String>,
    pub surprised: Vec<String>,
    pub shy: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionLibrary {
    pub click: ClickExpressions,
    pub hungry: Vec<String>,
    pub eating: Vec<String>,
    pub sleepy: Vec<String>,
    pub bored: Vec<String>,
    pub thinking: Vec<String>,
}
pub fn validate_expression(text: &str) -> AppResult<()> {
    // 保守宽度预算避免固定画布裁切；排除控制和双向文本控制字符。
    let width: usize = text.chars().map(|c| if c.is_ascii() { 1 } else { 2 }).sum();
    if text.trim().is_empty()
        || width > 14
        || text
            .chars()
            .any(|c| c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'))
    {
        return Err(AppError::new(
            "AI_PROTOCOL_ERROR",
            "模型生成的颜文字为空、过长或含不可显示字符，已保留原表情",
        ));
    }
    Ok(())
}
pub fn validate_library(value: &ExpressionLibrary) -> AppResult<()> {
    for group in [
        &value.click.happy,
        &value.click.surprised,
        &value.click.shy,
        &value.hungry,
        &value.eating,
        &value.sleepy,
        &value.bored,
        &value.thinking,
    ] {
        if group.is_empty() || group.len() > 8 {
            return Err(AppError::new(
                "AI_PROTOCOL_ERROR",
                "表情库分类必须各包含 1–8 个颜文字",
            ));
        }
        for expression in group {
            validate_expression(expression)?;
        }
    }
    Ok(())
}
type ProviderStamp = (i64, Option<String>, String, Option<String>);
fn provider_stamp(c: &Connection) -> AppResult<ProviderStamp> {
    c.query_row(
        "SELECT id,base_url,model,api_key_encrypted FROM providers WHERE is_active=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )
    .optional()?
    .ok_or_else(|| {
        AppError::new(
            "PROVIDER_UNAVAILABLE",
            "尚未配置当前聊天提供商，请先在设置中添加并选择",
        )
    })
}
struct Snapshot {
    id: i64,
    name: String,
    prompt: String,
    previous: Option<String>,
    provider: ProviderStamp,
    config: ConnectionTest,
    health: crate::pet::controller::HealthToken,
}
fn capture(runtime: &Runtime, id: i64) -> AppResult<(Snapshot, RequestLease)> {
    runtime.read(|s, p| {
        let health = s.pet.health(&s.connection, p)?;
        s.pet.require_ai()?;
        let (name, prompt, previous) = s
            .connection
            .query_row(
                "SELECT name,prompt,expressions FROM personas WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?
            .ok_or_else(AppError::not_found)?;
        let provider = provider_stamp(&s.connection)?;
        let config = providers::connection_test_config(&s.connection, provider.0, p)?;
        let lease = runtime.requests.start(format!("expressions:{id}"), id)?;
        Ok((
            Snapshot {
                id,
                name,
                prompt,
                previous,
                provider,
                config,
                health,
            },
            lease,
        ))
    })
}
fn save(
    runtime: &Runtime,
    snapshot: &Snapshot,
    lease: &RequestLease,
    value: &ExpressionLibrary,
) -> AppResult<()> {
    validate_library(value)?;
    let json = serde_json::to_string(value).map_err(|_| AppError::storage())?;
    runtime.read(|s,_| {
        lease.cancel.check()?;
        let tx=s.connection.transaction()?;
        if provider_stamp(&tx)?!=snapshot.provider {return Err(AppError::new("CONTEXT_CHANGED","提供商配置已改变，旧表情生成结果未保存"));}
        let changed=tx.execute("UPDATE personas SET expressions=?1 WHERE id=?2 AND name=?3 AND prompt=?4 AND expressions IS ?5",params![json,snapshot.id,snapshot.name,snapshot.prompt,snapshot.previous])?;
        if changed!=1 {return Err(AppError::new("CONTEXT_CHANGED","人设已改变或删除，旧表情生成结果未保存"));}
        tx.commit()?;Ok(())
    })
}
pub async fn generate_persona_expressions(
    runtime: Runtime,
    id: i64,
) -> AppResult<ExpressionLibrary> {
    let capture_runtime = runtime.clone();
    let (snapshot, lease) =
        tauri::async_runtime::spawn_blocking(move || capture(&capture_runtime, id))
            .await
            .map_err(|_| AppError::storage())??;
    let input=GenerateRequest{messages:vec![
        Message::new("system","只返回 JSON 对象，不附加 Markdown。结构必须为 {\"click\":{\"happy\":[\"颜文字\"],\"surprised\":[\"颜文字\"],\"shy\":[\"颜文字\"]},\"hungry\":[\"颜文字\"],\"eating\":[\"颜文字\"],\"sleepy\":[\"颜文字\"],\"bored\":[\"颜文字\"],\"thinking\":[\"颜文字\"]}。每类生成 3 个符合人设的单行颜文字，不写解释。每个颜文字最多 14 个半角宽度，非 ASCII 字符按 2 个宽度计算。"),
        Message::new("user",format!("人设名称：{}\n人设设定：{}",snapshot.name,snapshot.prompt))],max_tokens:2048,tools:vec![],json:true};
    let result = runtime
        .ai
        .clone()?
        .structured(&snapshot.config, &input, &lease.cancel, validate_library)
        .await;
    if let Err(error) = &result {
        let _ = runtime.read(|s, p| {
            s.pet.health(&s.connection, p)?;
            s.pet.report_ai_failure(&snapshot.health, error);
            Ok(())
        });
    }
    let result = result?;
    tauri::async_runtime::spawn_blocking(move || {
        save(&runtime, &snapshot, &lease, &result)?;
        Ok(result)
    })
    .await
    .map_err(|_| AppError::storage())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn library() -> ExpressionLibrary {
        serde_json::from_value(json!({"click":{"happy":["(^_^)"],"surprised":["(O_O)"],"shy":["(>_<)"]},"hungry":["(・ω・)"],"eating":["(・ω・)"],"sleepy":["(-_-)"],"bored":["(._.)"],"thinking":["(?)"]})).unwrap()
    }
    #[test]
    fn categories_and_display_width_are_validated() {
        validate_library(&library()).unwrap();
        let mut value = library();
        value.thinking.clear();
        assert!(validate_library(&value).is_err());
        assert!(serde_json::from_value::<ExpressionLibrary>(json!({"hungry":["x"]})).is_err());
        for bad in ["", "\n(^_^)", "123456789012345", "\u{202e}abc"] {
            assert!(validate_expression(bad).is_err());
        }
    }
    #[test]
    fn stale_or_cancelled_result_never_overwrites_library() {
        let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
        let (snapshot, lease) = capture(&runtime, 1).unwrap();
        assert!(capture(&runtime, 1).is_err());
        save(&runtime, &snapshot, &lease, &library()).unwrap();
        drop(lease);
        let previous = runtime
            .read(|s, _| {
                Ok(s.connection.query_row(
                    "SELECT expressions FROM personas WHERE id=1",
                    [],
                    |r| r.get::<_, String>(0),
                )?)
            })
            .unwrap();
        let (snapshot, lease) = capture(&runtime, 1).unwrap();
        runtime
            .read(|s, _| {
                s.connection
                    .execute("UPDATE personas SET prompt='修改后' WHERE id=1", [])?;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            save(&runtime, &snapshot, &lease, &library())
                .unwrap_err()
                .code,
            "CONTEXT_CHANGED"
        );
        drop(lease);
        let (snapshot, lease) = capture(&runtime, 1).unwrap();
        lease.cancel.cancel();
        assert_eq!(
            save(&runtime, &snapshot, &lease, &library())
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        drop(lease);
        let (snapshot, lease) = capture(&runtime, 1).unwrap();
        runtime
            .read(|s, _| {
                s.connection
                    .execute("UPDATE providers SET model='new-model'", [])?;
                Ok(())
            })
            .unwrap();
        assert!(save(&runtime, &snapshot, &lease, &library()).is_err());
        assert_eq!(
            runtime
                .read(|s, _| Ok(s.connection.query_row(
                    "SELECT expressions FROM personas WHERE id=1",
                    [],
                    |r| r.get::<_, String>(0)
                )?))
                .unwrap(),
            previous
        );
    }
    #[test]
    fn generation_persists_only_a_valid_complete_library() {
        tauri::async_runtime::block_on(async {
            use crate::ai::tests::{Response, Server};
            let server = Server::new(vec![
                Response::json(serde_json::to_value(library()).unwrap()),
                Response::json(json!({"hungry":[]})),
            ]);
            let runtime = crate::runtime::test_runtime(&server.url);
            generate_persona_expressions(runtime.clone(), 1)
                .await
                .unwrap();
            assert!(generate_persona_expressions(runtime.clone(), 1)
                .await
                .is_err());
            let saved = runtime
                .read(|s, _| {
                    Ok(s.connection.query_row(
                        "SELECT expressions FROM personas WHERE id=1",
                        [],
                        |r| r.get::<_, String>(0),
                    )?)
                })
                .unwrap();
            assert_eq!(
                serde_json::from_str::<ExpressionLibrary>(&saved)
                    .unwrap()
                    .click
                    .happy,
                vec!["(^_^)"]
            );
            server.finish();
        });
    }
}
