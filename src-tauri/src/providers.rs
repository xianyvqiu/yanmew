use crate::{
    crypto::{self, SecretUpdate},
    error::{AppError, AppResult},
    platform::contracts::SecretProtector,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// 仅后端使用；不派生 Serialize/Debug，避免凭据进入 DTO 或日志。
pub struct ConnectionTest {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub max_output_tokens: u32,
    pub context_window_override: Option<u32>,
}
pub fn connection_test_config(
    c: &Connection,
    id: i64,
    protector: &dyn SecretProtector,
) -> AppResult<ConnectionTest> {
    let row: Option<(Option<String>, String, Option<String>, u32, Option<u32>)> = c
        .query_row(
            "SELECT base_url,model,api_key_encrypted,max_output_tokens,context_window_override FROM provider_effective WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    let (base_url, model, key, max_output_tokens, context_window_override) = row.ok_or_else(AppError::not_found)?;
    Ok(ConnectionTest {
        base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        model,
        api_key: key.map(|v| protector.unprotect(&v)).transpose()?,
        max_output_tokens,
        context_window_override,
    })
}
pub async fn test_connection(
    client: &crate::ai::client::AiClient,
    config: ConnectionTest,
) -> AppResult<()> {
    let cancel = crate::state::Cancellation::default();
    cancel
        .wait(
            std::time::Duration::from_secs(30),
            client.complete(
                &config,
                &crate::ai::provider::GenerateRequest {
                    messages: vec![crate::ai::provider::Message::new("user", "请回复 OK")],
                    max_tokens: config.max_output_tokens,
                    tools: vec![],
                    json: false,
                },
                &cancel,
            ),
        )
        .await?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub base_url: Option<String>,
    pub model: String,
    pub has_api_key: bool,
    pub is_active: bool,
    pub memory_is_active: bool,
    pub max_output_tokens: u32,
    pub context_window_override: Option<u32>,
    pub capacity: crate::chat_capacity::Capacity,
    pub capabilities:crate::native_tools::Capabilities,
    pub purposes: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInput {
    pub name: String,
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub api_key: SecretUpdate,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: u32,
    #[serde(default)]
    pub context_window_override: Option<u32>,
}

pub fn default_max_output_tokens() -> u32 {
    32768
}

pub fn validate_url(value: &str) -> AppResult<()> {
    let url = url::Url::parse(value)
        .map_err(|_| AppError::validation("baseUrl", "请输入完整的 HTTP 或 HTTPS 接口地址"))?;
    if !["https", "http"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::validation(
            "baseUrl",
            "接口地址不能包含账号、密码、查询参数或片段",
        ));
    }
    Ok(())
}
pub fn list(c: &Connection) -> AppResult<Vec<Provider>> {
    let mut stmt=c.prepare("SELECT id,name,base_url,model,api_key_encrypted IS NOT NULL,is_active,memory_is_active,max_output_tokens,context_window_override,created_at,updated_at FROM provider_effective ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        let base_url: Option<String> = r.get(2)?;
        let model: String = r.get(3)?;
        let context_window_override = r.get(8)?;
        let capacity = crate::chat_capacity::resolve(
            base_url.as_deref().unwrap_or("https://api.openai.com/v1"),
            &model,
            context_window_override,
        );
        let capabilities=crate::native_tools::resolve(base_url.as_deref().unwrap_or("https://api.openai.com/v1"),&model);
        Ok(Provider {
            id: r.get::<_, i64>(0)?.to_string(),
            name: r.get(1)?,
            base_url,
            model,
            has_api_key: r.get(4)?,
            is_active: r.get(5)?,
            memory_is_active: r.get(6)?,
            max_output_tokens: r.get(7)?,
            context_window_override,
            capacity,capabilities,
            purposes: vec![],
            created_at: r.get(9)?,
            updated_at: r.get(10)?,
        })
    })?;
    let mut providers = rows.collect::<Result<Vec<_>, _>>()?;
    let roles = crate::provider_roles::get(c)?;
    for p in &mut providers {
        p.purposes = roles
            .roles
            .iter()
            .filter(|r| r.provider_id == p.id)
            .map(|r| r.purpose.clone())
            .collect();
    }
    Ok(providers)
}
pub fn save(
    c: &mut Connection,
    id: Option<i64>,
    input: ProviderInput,
    protector: &dyn SecretProtector,
) -> AppResult<Provider> {
    if input.name.trim().is_empty() {
        return Err(AppError::validation("name", "请填写提供商名称"));
    }
    if input.max_output_tokens == 0 {
        return Err(AppError::validation(
            "maxOutputTokens",
            "最大输出 token 数必须为正整数",
        ));
    }
    if input.context_window_override == Some(0) {
        return Err(AppError::validation(
            "contextWindowOverride",
            "上下文容量必须为正整数",
        ));
    }
    let base_url = input.base_url.filter(|v| !v.trim().is_empty());
    if let Some(url) = &base_url {
        validate_url(url)?;
    }
    let secret = crypto::prepare(input.api_key, protector)?;
    let tx = c.transaction()?;
    let now = chrono::Utc::now().to_rfc3339();
    let creating = id.is_none();
    let id = if let Some(id) = id {
        let old: Option<Option<String>> = tx
            .query_row(
                "SELECT api_key_encrypted FROM providers WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        let key = secret.unwrap_or(old.ok_or_else(AppError::not_found)?);
        tx.execute(
            "UPDATE providers SET name=?1,base_url=?2,api_key_encrypted=?3 WHERE id=?4",
            params![input.name.trim(), base_url, key, id],
        )?;
        id
    } else {
        tx.execute(
            "INSERT INTO providers(name,base_url,api_key_encrypted) VALUES(?1,?2,?3)",
            params![input.name.trim(), base_url, secret.flatten()],
        )?;
        tx.last_insert_rowid()
    };
    if creating && !input.model.trim().is_empty() && input.model.trim() != "未配置" {
        tx.execute("INSERT INTO provider_models(provider_id,purpose,model,max_output_tokens,context_window_override) VALUES(?1,'chat',?2,?3,?4)",params![id,input.model.trim(),input.max_output_tokens,input.context_window_override])?;
    }
    tx.execute("UPDATE providers SET created_at=CASE WHEN ?2 THEN ?3 ELSE created_at END,updated_at=?3 WHERE id=?1",params![id,creating,now])?;
    if tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM provider_roles WHERE purpose='image' AND provider_id=?1)",
        [id],
        |r| r.get::<_, bool>(0),
    )? {
        tx.execute("UPDATE provider_roles SET enabled=0 WHERE purpose='image' AND provider_id=?1 AND (SELECT api_key_encrypted FROM providers WHERE id=?1) IS NULL",[id])?;
        crate::provider_roles::image_changed(&tx)?;
    }
    crate::provider_roles::changed(&tx)?;
    let result = list(&tx)?
        .into_iter()
        .find(|p| p.id == id.to_string())
        .ok_or_else(AppError::not_found)?;
    tx.commit()?;
    Ok(result)
}
pub fn select(c: &mut Connection, id: Option<i64>, memory: bool) -> AppResult<()> {
    if !memory && id.is_none() {
        return Err(AppError::validation("id", "请选择聊天提供商"));
    }
    let tx = c.transaction()?;
    if let Some(id) = id {
        if !tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM providers WHERE id=?1)",
            [id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::not_found());
        }
    }
    let purpose = if memory { "memory" } else { "chat" };
    if let Some(id) = id {
        if memory {
            tx.execute("INSERT OR IGNORE INTO provider_models(provider_id,purpose,model) SELECT provider_id,'memory',model FROM provider_models WHERE provider_id=?1 AND purpose='chat'",[id])?;
        }
        tx.execute("INSERT INTO provider_roles(purpose,provider_id) SELECT purpose,provider_id FROM provider_models WHERE purpose=?1 AND provider_id=?2 ON CONFLICT(purpose) DO UPDATE SET provider_id=excluded.provider_id",params![purpose,id])?;
        if !tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_roles WHERE purpose=?1 AND provider_id=?2)",
            params![purpose, id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::validation("model", "请先配置该用途的模型"));
        }
    } else {
        tx.execute("DELETE FROM provider_roles WHERE purpose=?1", [purpose])?;
    }
    crate::provider_roles::changed(&tx)?;
    tx.commit()?;
    Ok(())
}
pub fn delete(c: &mut Connection, id: i64) -> AppResult<()> {
    let tx = c.transaction()?;
    if !tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM providers WHERE id=?1)",
        [id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(AppError::not_found());
    }
    if tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM provider_roles WHERE provider_id=?1)",
        [id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(AppError::new(
            "VALIDATION_ERROR",
            "不能删除正在使用的提供商，请先更改聊天、记忆、生图或识图用途",
        ));
    }
    tx.execute("DELETE FROM providers WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_test_checks_http_and_response_protocol() {
        use std::io::{Read, Write};
        for (status, body, success) in [
            (
                "200 OK",
                r#"{"choices":[{"message":{"content":"OK"},"finish_reason":"stop"}]}"#,
                true,
            ),
            ("200 OK", r#"{"choices":[]}"#, false),
            ("401 Unauthorized", "private upstream details", false),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .unwrap();
                let mut request = [0; 8192];
                let count = stream.read(&mut request).unwrap();
                assert!(String::from_utf8_lossy(&request[..count])
                    .starts_with("POST /v1/chat/completions "));
                write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            });
            let client = crate::ai::client::AiClient::new(Default::default()).unwrap();
            let result = tauri::async_runtime::block_on(test_connection(
                &client,
                ConnectionTest {
                    max_output_tokens: default_max_output_tokens(),
                    context_window_override: None,
                    base_url: format!("http://{address}/v1"),
                    model: "fake-model".into(),
                    api_key: None,
                },
            ));
            assert_eq!(result.is_ok(), success);
            if let Err(e) = result {
                assert!(!e.message.contains("private upstream"));
            }
            server.join().unwrap();
        }
    }
    struct Secret;
    impl SecretProtector for Secret {
        fn protect(&self, p: &str) -> AppResult<String> {
            Ok(format!("encrypted:{p}"))
        }
        fn unprotect(&self, _: &str) -> AppResult<String> {
            unreachable!()
        }
    }
    fn input(key: SecretUpdate) -> ProviderInput {
        ProviderInput {
            name: "测试".into(),
            base_url: None,
            model: "test-model".into(),
            api_key: key,
            max_output_tokens: default_max_output_tokens(),
            context_window_override: None,
        }
    }
    #[test]
    fn output_budget_defaults_roundtrip_and_validation() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        let legacy: ProviderInput =
            serde_json::from_str(r#"{"name":"兼容","baseUrl":null,"model":"test-model"}"#).unwrap();
        assert_eq!(legacy.max_output_tokens, 32768);
        assert_eq!(legacy.context_window_override, None);
        let p = save(&mut c, None, legacy, &Secret).unwrap();
        assert_eq!(p.max_output_tokens, 32768);
        let id: i64 = p.id.parse().unwrap();
        let mut updated = input(SecretUpdate::Keep);
        updated.max_output_tokens = 4096;
        updated.context_window_override = Some(128000);
        let revision = crate::provider_roles::revision(&c).unwrap();
        crate::provider_roles::save(
            &mut c,
            "chat",
            Some(&id.to_string()),
            "test-model",
            true,
            revision,
            Some(updated.max_output_tokens),
            updated.context_window_override,
        )
        .unwrap();
        let p = save(&mut c, Some(id), input(SecretUpdate::Keep), &Secret).unwrap();
        assert_eq!(p.max_output_tokens, 4096);
        assert_eq!(p.context_window_override, Some(128000));
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["maxOutputTokens"], 4096);
        assert_eq!(json["contextWindowOverride"], 128000);
        assert!(json.get("capacity").is_some());
        for (max_output_tokens, context_window_override) in [(0, None), (1, Some(0))] {
            let mut invalid = input(SecretUpdate::Keep);
            invalid.max_output_tokens = max_output_tokens;
            invalid.context_window_override = context_window_override;
            assert!(save(&mut c, Some(id), invalid, &Secret).is_err());
        }
        assert_eq!(list(&c).unwrap()[0].max_output_tokens, 4096);
        assert!(serde_json::from_str::<ProviderInput>(
            r#"{"name":"x","model":"x","maxOutputTokens":1.5}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ProviderInput>(
            r#"{"name":"x","model":"x","maxOutputTokens":-1}"#
        )
        .is_err());
    }
    #[test]
    fn secrets_selection_and_deletion() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::initialize(&mut c).unwrap();
        let p = save(
            &mut c,
            None,
            input(SecretUpdate::Set {
                value: "private-key".into(),
            }),
            &Secret,
        )
        .unwrap();
        let json = serde_json::to_string(&p).unwrap();
        assert!(!json.contains("private-key"));
        assert!(!json.contains("encrypted"));
        let id = p.id.parse().unwrap();
        assert!(
            save(&mut c, Some(id), input(SecretUpdate::Keep), &Secret)
                .unwrap()
                .has_api_key
        );
        select(&mut c, Some(id), false).unwrap();
        assert!(select(&mut c, Some(999), false).is_err());
        assert!(list(&c).unwrap()[0].is_active);
        assert!(delete(&mut c, id).is_err());
        assert!(
            !save(&mut c, Some(id), input(SecretUpdate::Clear), &Secret)
                .unwrap()
                .has_api_key
        );
        select(&mut c, Some(id), true).unwrap();
        select(&mut c, None, true).unwrap();
        assert!(!list(&c).unwrap()[0].memory_is_active);
        assert!(validate_url("https://name:secret@example.com").is_err());
        assert!(validate_url("file:///tmp/foo").is_err());
    }
}
