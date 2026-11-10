//! 便携更新协议与文件事务；不依赖窗口、数据库业务或用户密钥。
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub mod install;
pub mod network;
#[cfg(windows)]
pub mod windows;

pub type Result<T> = std::result::Result<T, String>;
pub const REPOSITORY: &str = "https://github.com/xianyvqiu/yanmew";
pub const RELEASES: &str = "https://github.com/xianyvqiu/yanmew/releases";
pub const PACKAGE_MANIFEST: &str = "yanmew-package.json";
pub const HELPER: &str = "yanmew-update.exe";
pub const BUILD_ID: &str = match option_env!("YANMEW_BUILD_ID") { Some(value) => value, None => "00000000-000000" };
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct PackageFile { pub path: String, pub size: u64, pub sha256: String }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Package {
    pub schema_version: u32, pub product: String, pub version: String, pub build_id: String,
    pub files: Vec<PackageFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema_version: u32, pub version: String, pub build_id: String,
    pub asset_name: String, pub size: u64, pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Candidate { pub manifest: ReleaseManifest, pub url: String, pub release_url: String, pub notes: String }

pub fn valid_build(value: &str) -> bool {
    value.len()==15 && value.as_bytes()[8]==b'-' && value.bytes().enumerate().all(|(i,b)|i==8||b.is_ascii_digit())
}
pub fn valid_hash(value: &str) -> bool { value.len()==64 && value.bytes().all(|b|b.is_ascii_hexdigit()) }
pub fn newer(version: &str, build: &str, current_version: &str, current_build: &str) -> Result<bool> {
    if !valid_build(build)||!valid_build(current_build) { return Err("更新构建编号无效".into()); }
    let next=semver::Version::parse(version).map_err(|_|"更新版本号无效")?;
    let current=semver::Version::parse(current_version).map_err(|_|"当前版本号无效")?;
    Ok(next>current||(next==current&&build>current_build))
}
pub fn hash_file(path: &Path) -> Result<String> {
    let mut file=File::open(path).map_err(|_|"无法读取待校验文件")?;
    let mut hash=Sha256::new(); let mut buffer=[0u8;65536];
    loop { let n=file.read(&mut buffer).map_err(|_|"文件校验读取失败")?; if n==0 { break; } hash.update(&buffer[..n]); }
    Ok(format!("{:x}",hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn same_version_uses_build_without_changing_display_version() {
        assert!(newer("0.1.0","20261110-170000","0.1.0","20261110-162831").unwrap());
        assert!(!newer("0.1.0","20261110-162831","0.1.0","20261110-162831").unwrap());
        assert!(!newer("0.0.9","20261111-170000","0.1.0","20261110-162831").unwrap());
        assert!(newer("0.2.0","20261101-170000","0.1.0","20261110-162831").unwrap());
        assert!(newer("bad","20261110-170000","0.1.0",BUILD_ID).is_err());
        assert!(newer("0.1.0","../oops","0.1.0",BUILD_ID).is_err());
    }
}
