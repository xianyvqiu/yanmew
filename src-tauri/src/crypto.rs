use crate::{
    error::{AppError, AppResult},
    platform::contracts::SecretProtector,
};
use serde::Deserialize;

/// 未提供字段表示保留；清除与写入必须显式区分。
#[derive(Default, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum SecretUpdate {
    #[default]
    Keep,
    Clear,
    Set {
        value: String,
    },
}

pub fn prepare(
    update: SecretUpdate,
    protector: &dyn SecretProtector,
) -> AppResult<Option<Option<String>>> {
    match update {
        SecretUpdate::Keep => Ok(None),
        SecretUpdate::Clear => Ok(Some(None)),
        SecretUpdate::Set { value } => {
            if value.trim().is_empty() || value.len() > 16384 {
                return Err(AppError::validation(
                    "apiKey",
                    "密钥不能为空或过长；删除密钥请使用清除操作",
                ));
            }
            Ok(Some(Some(protector.protect(&value)?)))
        }
    }
}
