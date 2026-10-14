use super::*;
use base64::Engine;
use plugins::{Manifest, PermissionChoice};
use std::{collections::HashSet, path::Path};
pub struct Pending {
    pub token: String,
    pub package: Package,
    pub created: Instant,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub token: String,
    pub manifest: Manifest,
    pub installed: Option<plugins::Plugin>,
    pub permissions: Vec<plugins::Permission>,
    pub migration_required: bool,
}
pub fn cancel(host: &Host, token: &str) -> AppResult<()> {
    let mut pending = host.pending.lock().map_err(|_| AppError::storage())?;
    if pending.as_ref().is_some_and(|p| p.token == token) {
        pending.take();
    }
    Ok(())
}
pub fn preview(
    host: &Host,
    runtime: &Runtime,
    filename: &str,
    bytes_base64: &str,
) -> AppResult<Preview> {
    host.ensure_ready()?;
    if !filename.ends_with(".yanmew-game")
        || bytes_base64.len() > plugins::package::MAX_PACKAGE.div_ceil(3) * 4
    {
        return Err(plugins::package::invalid());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(bytes_base64)
        .map_err(|_| plugins::package::invalid())?;
    let package = plugins::package::validate(bytes)?;
    let (installed, permissions, migration_required) = runtime.read(|s, _| {
        let installed = match plugins::get(&s.connection, &package.manifest.id) {
            Ok(p) => Some(p),
            Err(e) if e.code == "NOT_FOUND" => None,
            Err(e) => return Err(e),
        };
        let permissions = if installed.is_some() {
            plugins::permissions(&s.connection, &package.manifest.id)?
        } else {
            vec![]
        };
        let migration = if let Some(p) = &installed {
            check_version(p, &package)?;
            let snap = plugins::snapshot(&s.connection, &p.plugin_id)?;
            p.storage_version != package.manifest.storage_version
                && (!snap.data.is_empty() || !snap.settings.is_empty())
        } else {
            false
        };
        Ok((installed, permissions, migration))
    })?;
    let token = Host::token(runtime, "install-")?;
    let manifest = package.manifest.clone();
    *host.pending.lock().map_err(|_| AppError::storage())? = Some(Pending {
        token: token.clone(),
        package,
        created: Instant::now(),
    });
    Ok(Preview {
        token,
        manifest,
        installed,
        permissions,
        migration_required,
    })
}
fn check_version(p: &plugins::Plugin, pkg: &Package) -> AppResult<()> {
    let old =
        semver::Version::parse(&p.installed_version).map_err(|_| plugins::package::invalid())?;
    let new =
        semver::Version::parse(&pkg.manifest.version).map_err(|_| plugins::package::invalid())?;
    if new < old
        || new == old && p.package_sha256 != pkg.sha
        || pkg.manifest.storage_version < p.storage_version
    {
        return Err(AppError::new(
            "PLUGIN_VERSION_CONFLICT",
            "不能降级插件或存档版本，同一版本也不能替换为不同内容",
        ));
    }
    Ok(())
}
pub async fn confirm(
    app: &AppHandle,
    runtime: &Runtime,
    token: &str,
    choices: Vec<PermissionChoice>,
) -> AppResult<plugins::Plugin> {
    let host = app.state::<Host>();
    let _operation = host.operations.lock().await;
    host.ensure_ready()?;
    let pending = {
        let mut p = host.pending.lock().map_err(|_| AppError::storage())?;
        if !p
            .as_ref()
            .is_some_and(|p| p.token == token && !super::expired(p.created))
        {
            return Err(AppError::new(
                "PLUGIN_INSTALL_EXPIRED",
                "安装预览已过期，请重新选择插件包",
            ));
        }
        p.take().ok_or_else(plugins::package::invalid)?
    };
    install_locked(app, runtime, pending.package, false, choices).await
}
/// 调用者已取得 operations，不能在这里递归锁定。源身份来自随包清单而不是 manifest。
async fn install_locked(
    app: &AppHandle,
    runtime: &Runtime,
    pkg: Package,
    bundled: bool,
    choices: Vec<PermissionChoice>,
) -> AppResult<plugins::Plugin> {
    let id = pkg.manifest.id.clone();
    super::close_plugin(app, &id)?;
    let (old, original) = runtime.read(|s, _| {
        let old = match plugins::get(&s.connection, &id) {
            Ok(p) => Some(p),
            Err(e) if e.code == "NOT_FOUND" => None,
            Err(e) => return Err(e),
        };
        if let Some(p) = &old {
            if p.is_bundled != bundled {
                return Err(AppError::new(
                    "PLUGIN_SOURCE_CONFLICT",
                    "外部插件不能替换随应用插件",
                ));
            }
            check_version(p, &pkg)?;
            Ok((old, Some(plugins::snapshot(&s.connection, &id)?)))
        } else {
            Ok((None, None))
        }
    })?;
    let scopes = pkg.manifest.scopes();
    let existing = runtime.read(|s, _| {
        if old.is_some() {
            plugins::permissions(&s.connection, &id)
        } else {
            Ok(vec![])
        }
    })?;
    // 安装确认必须明确决定新增的每项可选权限；随包登记只能保持 pending。
    if !bundled {
        for (permission, scope) in &scopes {
            if !existing.iter().any(|p| {
                &p.permission == permission && &p.scope == scope && p.decision != "pending"
            }) && !choices.iter().any(|p| {
                &p.permission == permission
                    && &p.scope == scope
                    && p.decision != plugins::Decision::Pending
            }) {
                return Err(AppError::validation(
                    "permissions",
                    "请逐项允许或拒绝新增或待确认权限",
                ));
            }
        }
    }
    let target = if let Some(s) = &original {
        if s.storage_version != pkg.manifest.storage_version
            && (!s.settings.is_empty() || !s.data.is_empty())
        {
            super::migrate(app, runtime, pkg.clone(), s.clone()).await?
        } else {
            let mut s = s.clone();
            s.storage_version = pkg.manifest.storage_version;
            plugins::validate_snapshot(&pkg.manifest, &s)?;
            s
        }
    } else {
        PrivateSnapshot {
            storage_version: pkg.manifest.storage_version,
            settings: Default::default(),
            data: Default::default(),
        }
    };
    let root = plugins::package::root(runtime.paths.as_ref().map_err(Clone::clone)?);
    let write_pkg = pkg.clone();
    let relative =
        tauri::async_runtime::spawn_blocking(move || plugins::package::publish(&root, &write_pkg))
            .await
            .map_err(|_| AppError::storage())??;
    runtime.read(|s, _| {
        let tx = s
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(expected) = &old {
            let current = plugins::get(&tx, &id)?;
            if current.installed_version != expected.installed_version
                || current.package_sha256 != expected.package_sha256
                || original.as_ref() != Some(&plugins::snapshot(&tx, &id)?)
            {
                return Err(AppError::new(
                    "PLUGIN_INSTALL_CONFLICT",
                    "升级前数据发生变化，请重新安装；较新数据未被覆盖",
                ));
            }
        } else if plugins::get(&tx, &id).is_ok() {
            return Err(AppError::new(
                "PLUGIN_INSTALL_CONFLICT",
                "插件已被其他操作登记，请重新安装",
            ));
        }
        let now = runtime.clock.sample()?.utc;
        plugins::register(
            &tx,
            &pkg.manifest,
            &relative,
            &pkg.sha,
            bundled,
            now,
            &choices,
        )?;
        plugins::replace_snapshot(&tx, &pkg.manifest, &target, now)?;
        tx.commit()?;
        plugins::get(&s.connection, &id)
    })
}
pub async fn initialize(app: AppHandle, runtime: Runtime) -> AppResult<()> {
    let host = app.state::<Host>();
    let _operation = host.operations.lock().await;
    runtime.read(|s, _| plugins::runs::recover(&s.connection, runtime.clock.sample()?.utc))?;
    let root = plugins::package::root(runtime.paths.as_ref().map_err(Clone::clone)?);
    let paths = runtime.paths.as_ref().map_err(Clone::clone)?;
    let resources = paths.plugin_resources()?;
    let catalogue = resources.join("bundled-games.json");
    // 随包清单与源码构建产物共用包校验；不包含资源的开发环境不伪造登记。
    if catalogue.exists() {
        let bytes = std::fs::read(&catalogue).map_err(|_| plugins::package::invalid())?;
        if bytes.len() > 64 * 1024 {
            return Err(plugins::package::invalid());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Bundled {
            api_version: String,
            packages: Vec<String>,
        }
        let list: Bundled =
            serde_json::from_slice(&bytes).map_err(|_| plugins::package::invalid())?;
        if list.api_version != "1.0.0" || list.packages.len() > 64 {
            return Err(plugins::package::invalid());
        }
        let mut ids = HashSet::new();
        for relative in list.packages {
            plugins::manifest::path(&relative)?;
            if !relative.ends_with(".yanmew-game") {
                return Err(plugins::package::invalid());
            }
            let file = resources.join(relative);
            let pkg = read_package(&file)?;
            if !ids.insert(pkg.manifest.id.clone()) || pkg.manifest.repository.is_none() {
                return Err(plugins::package::invalid());
            }
            let unchanged = runtime.read(|s, _| {
                Ok(
                    plugins::get(&s.connection, &pkg.manifest.id).is_ok_and(|p| {
                        p.is_bundled
                            && p.package_sha256 == pkg.sha
                            && plugins::package::load(&root, &p).is_ok()
                    }),
                )
            })?;
            if !unchanged {
                if let Err(e) = install_locked(&app, &runtime, pkg, true, vec![]).await {
                    let _ = app.emit("game_plugin_error", &e);
                }
            }
        }
    }
    // 不可用的登记保留使用数据；缺包不允许宿主继续加载。
    let referenced=runtime.read(|s,_|{
        for p in plugins::list(&s.connection)? {if p.package_status=="available"&&plugins::package::load(&root,&p).is_err(){s.connection.execute("UPDATE game_plugins SET package_status='missing_package',package_path=NULL,is_enabled=0 WHERE plugin_id=?1",[p.plugin_id])?;}}
        Ok(plugins::list(&s.connection)?.into_iter().filter_map(|p|p.package_path).collect::<HashSet<_>>())
    })?;
    tauri::async_runtime::spawn_blocking(move || plugins::package::cleanup(&root, &referenced))
        .await
        .map_err(|_| AppError::storage())??;
    let _ = app.emit("game_plugins_changed", ());
    Ok(())
}
fn read_package(path: &Path) -> AppResult<Package> {
    let meta = std::fs::metadata(path).map_err(|_| plugins::package::invalid())?;
    if !meta.is_file() || meta.len() > plugins::package::MAX_PACKAGE as u64 {
        return Err(plugins::package::invalid());
    }
    plugins::package::validate(std::fs::read(path).map_err(|_| plugins::package::invalid())?)
}
