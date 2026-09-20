use crate::{
    error::{AppError, AppResult},
    platform::contracts::SecretProtector,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use windows::Win32::{
    Foundation::{LocalFree, HLOCAL},
    Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    },
};

pub struct Dpapi;

fn crypt(input: &[u8], decrypt: bool) -> AppResult<Vec<u8>> {
    let failed = || {
        AppError::new(
            "SECRET_UNAVAILABLE",
            "密钥保护失败或凭据不属于当前 Windows 用户，请重新填写密钥",
        )
    };
    let source = CRYPT_INTEGER_BLOB {
        cbData: input.len().try_into().map_err(|_| failed())?,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        if decrypt {
            CryptUnprotectData(
                &source,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &source,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
        .map_err(|_| failed())?;
        // DPAPI 分配的缓冲区必须由 LocalFree 释放，不能交给 Rust allocator。
        let result = if output.cbData == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        if !output.pbData.is_null() {
            let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        }
        Ok(result)
    }
}
impl SecretProtector for Dpapi {
    fn protect(&self, plain: &str) -> AppResult<String> {
        Ok(STANDARD.encode(crypt(plain.as_bytes(), false)?))
    }
    fn unprotect(&self, encrypted: &str) -> AppResult<String> {
        let bytes = STANDARD
            .decode(encrypted)
            .map_err(|_| AppError::new("SECRET_UNAVAILABLE", "密钥数据损坏，请重新填写"))?;
        String::from_utf8(crypt(&bytes, true)?)
            .map_err(|_| AppError::new("SECRET_UNAVAILABLE", "密钥内容无效，请重新填写"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpapi_round_trip_and_corruption() {
        let encrypted = Dpapi.protect("测试-key-🔑").unwrap();
        assert!(!encrypted.contains("测试"));
        assert_eq!(Dpapi.unprotect(&encrypted).unwrap(), "测试-key-🔑");
        assert!(Dpapi.unprotect("broken").is_err());
        assert!(Dpapi.unprotect(&STANDARD.encode(b"invalid blob")).is_err());
    }
}
