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
}
pub fn connection_test_config(
    c: &Connection,
    id: i64,
    protector: &dyn SecretProtector,
) -> AppResult<ConnectionTest> {
    let row: Option<(Option<String>, String, Option<String>)> = c
        .query_row(
            "SELECT base_url,model,api_key_encrypted FROM providers WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (base_url, model, key) = row.ok_or_else(AppError::not_found)?;
    Ok(ConnectionTest {
        base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".into()),
        model,
        api_key: key.map(|v| protector.unprotect(&v)).transpose()?,
    })
}
pub async fn test_connection(config: ConnectionTest) -> AppResult<()> {
    validate_url(&config.base_url)?;
    let failed = || {
        AppError::new(
            "PROVIDER_UNAVAILABLE",
            "连接测试失败，请检查接口地址、网络和模型配置",
        )
    };
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| failed())?;
    let mut request=client.post(format!("{}/chat/completions",config.base_url.trim_end_matches('/'))).json(&serde_json::json!({"model":config.model,"messages":[{"role":"user","content":"请回复 OK"}],"stream":false}));
    if let Some(key) = config.api_key {
        request = request.bearer_auth(key);
    }
    let mut response = request.send().await.map_err(|_| failed())?;
    if !response.status().is_success() {
        return Err(AppError::new(
            "PROVIDER_UNAVAILABLE",
            format!(
                "提供商返回 HTTP {}，请检查密钥、模型权限及服务状态",
                response.status().as_u16()
            ),
        ));
    }
    // 不读取无限响应，也不把上游错误正文（可能含凭据）传给前端。
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| failed())? {
        if body.len() + chunk.len() > 262144 {
            return Err(AppError::new("PROVIDER_UNAVAILABLE", "连接测试响应过大"));
        }
        body.extend_from_slice(&chunk);
    }
    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| AppError::new("PROVIDER_UNAVAILABLE", "提供商未返回有效的 JSON 回复"))?;
    if value
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .is_none_or(|v| v.trim().is_empty())
    {
        return Err(AppError::new(
            "PROVIDER_UNAVAILABLE",
            "提供商未返回有效的聊天内容，请检查模型与接口兼容性",
        ));
    }
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
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderInput {
    pub name: String,
    pub base_url: Option<String>,
    pub model: String,
    #[serde(default)]
    pub api_key: SecretUpdate,
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
    let mut stmt=c.prepare("SELECT id,name,base_url,model,api_key_encrypted IS NOT NULL,is_active,memory_is_active FROM providers ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok(Provider {
            id: r.get::<_, i64>(0)?.to_string(),
            name: r.get(1)?,
            base_url: r.get(2)?,
            model: r.get(3)?,
            has_api_key: r.get(4)?,
            is_active: r.get(5)?,
            memory_is_active: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
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
    if input.model.trim().is_empty() {
        return Err(AppError::validation("model", "请填写模型名称"));
    }
    let base_url = input.base_url.filter(|v| !v.trim().is_empty());
    if let Some(url) = &base_url {
        validate_url(url)?;
    }
    let secret = crypto::prepare(input.api_key, protector)?;
    let tx = c.transaction()?;
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
            "UPDATE providers SET name=?1,base_url=?2,model=?3,api_key_encrypted=?4 WHERE id=?5",
            params![input.name.trim(), base_url, input.model.trim(), key, id],
        )?;
        id
    } else {
        tx.execute(
            "INSERT INTO providers(name,base_url,model,api_key_encrypted) VALUES(?1,?2,?3,?4)",
            params![
                input.name.trim(),
                base_url,
                input.model.trim(),
                secret.flatten()
            ],
        )?;
        tx.last_insert_rowid()
    };
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
    if memory {
        tx.execute(
            "UPDATE providers SET memory_is_active=0 WHERE memory_is_active=1",
            [],
        )?;
        if let Some(id) = id {
            tx.execute("UPDATE providers SET memory_is_active=1 WHERE id=?1", [id])?;
        }
    } else {
        tx.execute("UPDATE providers SET is_active=0 WHERE is_active=1", [])?;
        tx.execute(
            "UPDATE providers SET is_active=1 WHERE id=?1",
            [id.unwrap()],
        )?;
    }
    tx.commit()?;
    Ok(())
}
pub fn delete(c: &mut Connection, id: i64) -> AppResult<()> {
    let tx = c.transaction()?;
    let active = tx
        .query_row(
            "SELECT is_active OR memory_is_active FROM providers WHERE id=?1",
            [id],
            |r| r.get::<_, bool>(0),
        )
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if active {
        return Err(AppError::new(
            "VALIDATION_ERROR",
            "不能删除正在使用的提供商，请先切换聊天和记忆提供商",
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
                r#"{"choices":[{"message":{"content":"OK"}}]}"#,
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
            let result = tauri::async_runtime::block_on(test_connection(ConnectionTest {
                base_url: format!("http://{address}/v1"),
                model: "fake-model".into(),
                api_key: None,
            }));
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
        }
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
