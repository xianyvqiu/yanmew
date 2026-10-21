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
    #[serde(default)]
    pub lifted:Vec<String>,
    pub click: ClickExpressions,
    pub hungry: Vec<String>,
    pub eating: Vec<String>,
    pub sleepy: Vec<String>,
    pub bored: Vec<String>,
    pub thinking: Vec<String>,
    /// 兼容旧人设和旧备份；新生成的表情库必须包含独立待机表情。
    #[serde(default)]
    pub standby: Vec<String>,
    #[serde(default)] pub craving:Vec<String>,
    #[serde(default)] pub craving_favorite:Vec<String>,
    #[serde(default)] pub eating_favorite:Vec<String>,
    #[serde(default)] pub normal: Vec<String>,
    #[serde(default)] pub peek_left: Vec<String>,
    #[serde(default)] pub peek_right: Vec<String>,
    #[serde(default)] pub rest_sit: Vec<String>,
    #[serde(default)] pub rest_lie: Vec<String>,
    #[serde(default)] pub walk_left: Vec<String>,
    #[serde(default)] pub walk_right: Vec<String>,
}

/// 程序自带基础姿态；未保存专属表情时，展示与运行时使用同一来源。
pub(crate) fn builtin(category: &str) -> &'static str {
    match category {
        "lifted"=>"(｡>﹏<｡)",
        "click" | "click_happy" => "(^_^)", "click_surprised" => "(O_O)", "click_shy" => "(〃ω〃)", "standby" => "(｡-ω-｡)", "hungry" => "(>_<)饿",
        "eating" => "(๑´ڡ`๑)", "sleepy" => "(-_-) zZ", "bored" => "(._.)", "thinking" => "(・・?)",
        "craving" => "(๑•﹏•๑)", "craving_favorite" => "(✧ω✧)", "eating_favorite" => "(๑>؂<๑)",
        "peek_left" => "|・ω・)", "peek_right" => "(・ω・|", "rest_sit" => "(｡･ω･｡)＿", "rest_lie" => "＿(｡-ω-｡)＿",
        "walk_left" => "ｃ(・ω・ｃ)з", "walk_right" => "ε(っ・ω・)っ", _ => "(・ω・)",
    }
}
pub(crate) fn effective_library(saved: Option<&serde_json::Value>) -> serde_json::Value {
    let mut result=serde_json::Map::new();
    for category in ["normal","standby","hungry","eating","sleepy","bored","thinking","craving","craving_favorite","eating_favorite","peek_left","peek_right","rest_sit","rest_lie","walk_left","walk_right","lifted"] {
        let value=saved.and_then(|v|v.get(category).or_else(||category.strip_suffix("_favorite").and_then(|base|v.get(base)))).filter(|v|v.as_array().is_some_and(|a|!a.is_empty())).cloned()
            .unwrap_or_else(||serde_json::json!([builtin(category)]));
        result.insert(category.into(),value);
    }
    let mut click=serde_json::Map::new();
    for kind in ["happy","surprised","shy"] {
        click.insert(kind.into(),saved.and_then(|v|v.get("click")).and_then(|v|v.get(kind)).filter(|v|v.as_array().is_some_and(|a|!a.is_empty())).cloned().unwrap_or_else(||serde_json::json!([builtin(&format!("click_{kind}"))])));
    }
    result.insert("click".into(),serde_json::Value::Object(click));
    serde_json::Value::Object(result)
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
    if value.standby.len() > 8 {
        return Err(AppError::new("AI_PROTOCOL_ERROR", "待机分类最多包含 8 个颜文字"));
    }
    for group in [&value.craving,&value.craving_favorite,&value.eating_favorite,&value.normal,&value.lifted,&value.peek_left,&value.peek_right,&value.rest_sit,&value.rest_lie,&value.walk_left,&value.walk_right] {if group.len()>8{return Err(AppError::validation("expressions","表情分类最多8项"));}for e in group{validate_expression(e)?;}}
    for expression in &value.standby { validate_expression(expression)?; }
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

pub fn save_manual(c: &mut Connection, id: i64, expected: Option<serde_json::Value>, value: &ExpressionLibrary) -> AppResult<()> {
    validate_library(value).map_err(|_| AppError::validation("expressions", "请为每个分类填写简短、可显示的颜文字，每行一个，最多8个；过长或控制字符不能保存"))?;
    let tx = c.transaction()?;
    let raw: Option<String> = tx.query_row("SELECT expressions FROM personas WHERE id=?1", [id], |r| r.get(0)).optional()?.ok_or_else(AppError::not_found)?;
    let previous = raw.map(|s| serde_json::from_str::<serde_json::Value>(&s).map_err(|_|AppError::storage())).transpose()?;
    if previous != expected { return Err(AppError::new("CONFLICT", "表情库已更新，修改尚未保存；请保留草稿，取消修改后查看最新内容")); }
    tx.execute("UPDATE personas SET expressions=?1,updated_at=?2 WHERE id=?3",params![serde_json::to_string(value).map_err(|_|AppError::storage())?,chrono::Utc::now().to_rfc3339(),id])?;
    tx.commit()?; Ok(())
}

/// 手动编辑的本地动作与运行时共用；旧 JSON 缺字段继续使用基础姿态。
pub(crate) fn choose_local(c: &Connection, id: i64, category: &str, random: &mut dyn crate::pet::RandomSource) -> AppResult<String> {
    let raw: Option<String> = c.query_row("SELECT expressions FROM personas WHERE id=?1",[id],|r|r.get(0))?;
    let value = raw.map(|s|serde_json::from_str::<serde_json::Value>(&s).map_err(|_|AppError::storage())).transpose()?;
    let effective = effective_library(value.as_ref());
    let choices = effective.get(category).and_then(|v|v.as_array()).ok_or_else(AppError::storage)?;
    let index = random.below(choices.len() as u64)? as usize;
    choices.get(index).and_then(|v|v.as_str()).map(str::to_owned).ok_or_else(AppError::storage)
}

fn validate_generated_library(value: &ExpressionLibrary) -> AppResult<()> {
    validate_library(value)?;
    if value.craving.is_empty()||value.craving_favorite.is_empty()||value.eating_favorite.is_empty(){return Err(AppError::new("AI_PROTOCOL_ERROR","新生成表情库须包含嘴馋和喜好进食分类"));}
    if value.standby.is_empty() {
        return Err(AppError::new("AI_PROTOCOL_ERROR", "新生成的表情库必须包含待机颜文字"));
    }
    Ok(())
}
type ProviderStamp = (i64, Option<String>, String, Option<String>);
fn provider_stamp(c: &Connection) -> AppResult<ProviderStamp> {
    c.query_row(
        "SELECT id,base_url,model,api_key_encrypted FROM provider_effective WHERE is_active=1",
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
        Message::new("system","只返回 JSON 对象，不附加 Markdown。结构必须为 {\"click\":{\"happy\":[\"颜文字\"],\"surprised\":[\"颜文字\"],\"shy\":[\"颜文字\"]},\"hungry\":[\"颜文字\"],\"eating\":[\"颜文字\"],\"sleepy\":[\"颜文字\"],\"bored\":[\"颜文字\"],\"thinking\":[\"颜文字\"],\"standby\":[\"颜文字\"]}。每类生成 3 个符合人设的单行颜文字，不写解释。另外输出lifted数组（被轻轻拎住后颈的悬空反应，3个符合人设的颜文字），必须输出craving、craving_favorite、eating_favorite三个数组，分别是等待投喂、期待最爱食物、喜欢地咀嚼或品尝，每类3个颜文字。hungry每项须包含“饿”字，点击happy、surprised、shy须分别体现开心、惊讶、害羞，不共用同一表情。standby 表示安静待机，须区别于默认、无聊和睡觉。每个颜文字最多 14 个半角宽度，非 ASCII 字符按 2 个宽度计算。"),
        Message::new("user",format!("人设名称：{}\n人设设定：{}",snapshot.name,snapshot.prompt))],max_tokens:snapshot.config.max_output_tokens,tools:vec![],json:true};
    let result = runtime
        .ai
        .clone()?
        .structured(&snapshot.config, &input, &lease.cancel, validate_generated_library)
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
    #[test]
    fn builtin_click_emotions_are_distinct_and_hunger_has_label() {
        let happy=builtin("click_happy");let shy=builtin("click_shy");let surprised=builtin("click_surprised");
        assert_ne!(happy,shy);assert_ne!(happy,surprised);assert_ne!(shy,surprised);
        assert!(builtin("hungry").contains('饿'));
    }

    use super::*;
    use serde_json::json;
    #[test]
    fn manual_library_saves_without_provider_and_preserves_conflicting_edits() {
        struct First;
        impl crate::pet::RandomSource for First { fn below(&mut self,_:u64)->AppResult<u64>{Ok(0)} }
        let mut c=crate::db::test_database();
        let mut value:ExpressionLibrary=serde_json::from_value(effective_library(None)).unwrap();
        value.normal=vec!["(=^_^=)".into()]; value.peek_left=vec!["|(^_^)".into()];
        save_manual(&mut c,1,None,&value).unwrap();
        assert_eq!(choose_local(&c,1,"normal",&mut First).unwrap(),"(=^_^=)");
        assert_eq!(choose_local(&c,1,"peek_left",&mut First).unwrap(),"|(^_^)");
        let raw:String=c.query_row("SELECT expressions FROM personas WHERE id=1",[],|r|r.get(0)).unwrap();
        assert_eq!(save_manual(&mut c,1,None,&value).unwrap_err().code,"CONFLICT");
        let expected=serde_json::from_str(&raw).unwrap();
        value.hungry=vec!["bad\ncontrol".into()];
        assert_eq!(save_manual(&mut c,1,Some(expected),&value).unwrap_err().code,"VALIDATION_ERROR");
        assert_eq!(c.query_row("SELECT expressions FROM personas WHERE id=1",[],|r|r.get::<_,String>(0)).unwrap(),raw);
        assert_eq!(c.query_row("SELECT COUNT(*) FROM providers",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    fn library() -> ExpressionLibrary {
        serde_json::from_value(json!({"click":{"happy":["(^_^)"],"surprised":["(O_O)"],"shy":["(>_<)"]},"hungry":["(・ω・)"],"eating":["(・ω・)"],"sleepy":["(-_-)"],"bored":["(._.)"],"thinking":["(?)"],"craving":["(o_o)"],"craving_favorite":["(^o^)"],"eating_favorite":["(>w<)"],"standby":["(｡-ω-｡)"]})).unwrap()
    }
    #[test]
    fn effective_library_preserves_saved_choices_and_shares_runtime_fallbacks() {
        let saved=serde_json::json!({"standby":["(-ω-)"],"eating":["(๑´ڡ`๑)"],"click":{"happy":["(^o^)"]}});
        let effective=effective_library(Some(&saved));
        assert_eq!(effective["standby"],saved["standby"]);
        assert_eq!(effective["click"]["happy"],saved["click"]["happy"]);
        assert_eq!(effective["eating_favorite"],saved["eating"]);
        assert_eq!(effective["walk_left"][0],builtin("walk_left"));
        assert!(saved.get("walk_left").is_none());
        assert_eq!(effective_library(None)["hungry"][0],builtin("hungry"));
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
    fn legacy_library_reads_without_standby_but_new_generation_requires_it() {
        let mut json=serde_json::to_value(library()).unwrap();
        json.as_object_mut().unwrap().remove("standby");
        let legacy:ExpressionLibrary=serde_json::from_value(json).unwrap();
        validate_library(&legacy).unwrap();
        assert!(legacy.standby.is_empty());
        assert!(validate_generated_library(&legacy).is_err());
        validate_generated_library(&library()).unwrap();
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
                    .execute("UPDATE provider_models SET model='new-model' WHERE purpose='chat'", [])?;
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
