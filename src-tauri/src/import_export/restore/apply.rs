//! 正式数据库恢复：来源ID映射保存在临时库，业务写入使用一个Immediate事务。
use super::*;
use crate::{error::parse_id, runtime::Store};
use rusqlite::{params, TransactionBehavior};
use std::{collections::BTreeSet, fs};

fn bindings(
    target: &Connection,
    nodes: &[Node],
    input: &[Binding],
    table: &str,
) -> AppResult<BTreeMap<i64, Option<i64>>> {
    if nodes.len() != input.len() {
        return Err(stale());
    }
    let mut out = BTreeMap::new();
    for binding in input {
        let node = nodes
            .iter()
            .find(|n| n.source_id == binding.source_id)
            .ok_or_else(stale)?;
        let id = parse_id(&binding.source_id)?;
        let to = binding.target_id.as_deref().map(parse_id).transpose()?;
        if node.is_builtin && binding.target_id != node.suggested_target_id {
            return Err(stale());
        }
        if table == "user_profiles" && to.is_some() && out.values().any(|existing| *existing == to)
        {
            return Err(AppError::validation(
                "profiles",
                "一份本机用户设定只能接收一份来源设定，请为其他项新建",
            ));
        }
        if let Some(to) = to {
            let candidate = node
                .candidates
                .iter()
                .find(|c| c.id == to.to_string())
                .ok_or_else(stale)?;
            if stamp(row(target, table, to)?)? != candidate.stamp {
                return Err(stale());
            }
        }
        if out.insert(id, to).is_some() {
            return Err(stale());
        }
    }
    Ok(out)
}
pub(super) fn check(target: &Connection, p: &Pending, input: &Confirm) -> AppResult<()> {
    if p.preview.kind == PackageKind::Full && !input.confirm_global_restore {
        return Err(AppError::validation(
            "confirmGlobalRestore",
            "请确认全局数据恢复范围",
        ));
    }
    bindings(target, &p.preview.personas, &input.personas, "personas")?;
    bindings(
        target,
        &p.preview.profiles,
        &input.profiles,
        "user_profiles",
    )?;
    for (id, expected) in &p.plugins {
        let current = crate::game_plugins::get(target, id)?;
        if serde_json::to_value(&current).map_err(|_| AppError::storage())?
            != serde_json::to_value(expected).map_err(|_| AppError::storage())?
        {
            return Err(stale());
        }
    }
    Ok(())
}
fn rows(
    c: &Connection,
    table: &str,
    mut consume: impl FnMut(Record) -> AppResult<()>,
) -> AppResult<()> {
    let cols = super::super::protocol::COLUMNS
        .iter()
        .find(|(n, _)| *n == table)
        .ok_or_else(invalid_import)?
        .1;
    let names = cols.split(',').collect::<Vec<_>>();
    let mut stmt = c.prepare(&format!("SELECT {cols} FROM {table} ORDER BY 1"))?;
    let mut cursor = stmt.query([])?;
    while let Some(r) = cursor.next()? {
        consume(super::super::protocol::record(r, &names)?)?;
    }
    Ok(())
}
fn put(c: &Connection, table: &str, r: &Record, upsert: bool) -> AppResult<()> {
    let keys = r.keys().map(String::as_str).collect::<Vec<_>>();
    let values = stage::sql_values(c, table, r)?;
    let extra = if upsert {
        let key = if table == "session_pet_states" {
            "session_id"
        } else {
            "id"
        };
        format!(
            " ON CONFLICT({key}) DO UPDATE SET {}",
            keys.iter()
                .filter(|k| **k != key)
                .map(|k| format!("{k}=excluded.{k}"))
                .collect::<Vec<_>>()
                .join(",")
        )
    } else {
        String::new()
    };
    c.prepare_cached(&format!(
        "INSERT INTO {table}({}) VALUES({}){extra}",
        keys.join(","),
        vec!["?"; keys.len()].join(",")
    ))?
    .execute(rusqlite::params_from_iter(values))?;
    Ok(())
}
fn id(r: &Record, k: &str) -> AppResult<i64> {
    parse_id(value(r, k)?)
}
fn mapped(source: &Connection, table: &str, id: i64) -> AppResult<i64> {
    source
        .query_row(
            "SELECT target_id FROM import_id_map WHERE table_name=?1 AND source_id=?2",
            params![table, id],
            |r| r.get(0),
        )
        .map_err(|_| invalid_import())
}
fn map_field(source: &Connection, r: &mut Record, key: &str, table: &str) -> AppResult<()> {
    if let Some(value) = r.get_mut(key) {
        if !value.is_null() {
            *value = Value::String(
                mapped(
                    source,
                    table,
                    parse_id(value.as_str().ok_or_else(invalid_import)?)?,
                )?
                .to_string(),
            );
        }
    }
    Ok(())
}
fn allocate(
    source: &Connection,
    target: &Connection,
    persona: &BTreeMap<i64, Option<i64>>,
    profiles: &BTreeMap<i64, Option<i64>>,
) -> AppResult<()> {
    source.execute_batch("CREATE TABLE IF NOT EXISTS import_id_map(table_name TEXT,source_id INTEGER,target_id INTEGER,PRIMARY KEY(table_name,source_id)); DELETE FROM import_id_map; CREATE TABLE IF NOT EXISTS import_published(path TEXT,role TEXT,filename TEXT,PRIMARY KEY(path,role)); DELETE FROM import_published;")?;
    for (table, columns) in super::super::protocol::COLUMNS {
        if table.starts_with("real_") || !columns.split(',').any(|k| k == "id") || *table == "user_state" {
            continue;
        }
        let last:i64=target.query_row(&format!("SELECT MAX(COALESCE((SELECT seq FROM sqlite_sequence WHERE name=?1),0),COALESCE((SELECT MAX(id) FROM {table}),0))"),[table],|r|r.get(0))?;
        let mut next = last;
        let mut stmt = source.prepare(&format!("SELECT id FROM {table} ORDER BY id"))?;
        for r in stmt.query_map([], |r| r.get::<_, i64>(0))? {
            let old = r?;
            let reuse = if *table == "personas" {
                persona.get(&old).copied().flatten()
            } else if *table == "user_profiles" {
                profiles.get(&old).copied().flatten()
            } else {
                None
            };
            let new = if let Some(n) = reuse {
                n
            } else {
                next = next.checked_add(1).ok_or_else(AppError::storage)?;
                next
            };
            source.execute(
                "INSERT INTO import_id_map VALUES(?1,?2,?3)",
                params![table, old, new],
            )?;
        }
    }
    Ok(())
}
fn unique_name(c: &Connection, table: &str, name: &str) -> AppResult<String> {
    let mut candidate = name.to_owned();
    let mut number = 1u64;
    loop {
        let exists: bool = c.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE name=?1)"),
            [&candidate],
            |r| r.get(0),
        )?;
        if !exists {
            return Ok(candidate);
        }
        candidate = format!("{name}（导入{number}）");
        number = number.checked_add(1).ok_or_else(AppError::storage)?;
    }
}
fn assets(
    runtime: &Runtime,
    source: &stage::Staged,
    personas: &BTreeMap<i64, Option<i64>>,
) -> AppResult<()> {
    for (table, key, role) in [
        ("personas", "chat_bg_file", "background"),
        ("accessories", "image_file", "accessory"),
        ("souvenirs", "image_file", "accessory"),
        ("pet_artworks", "image_file", "accessory"),
        ("pet_artworks", "preview_file", "accessory"),
        ("message_images", "image_file", "chat"),
    ] {
        rows(&source.connection, table, |r| {
            if table == "personas" && personas.get(&id(&r, "id")?).is_some_and(Option::is_some) {
                return Ok(());
            }
            if table=="pet_artworks" && personas.get(&id(&r,"persona_id")?).is_some_and(Option::is_some){return Ok(());}
            let Some(path) = r.get(key).and_then(Value::as_str) else {
                return Ok(());
            };
            let exists: bool = source.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM import_published WHERE path=?1 AND role=?2)",
                params![path, role],
                |r| r.get(0),
            )?;
            if exists {
                return Ok(());
            }
            let disk: String = source.connection.query_row(
                "SELECT disk_name FROM import_files WHERE path=?1",
                [path],
                |r| r.get(0),
            )?;
            let bytes =
                fs::read(source.directory.path.join(disk)).map_err(|_| AppError::storage())?;
            let published = if role == "background" {
                let ext = crate::persona::background::validate(
                    path.strip_prefix("files/").ok_or_else(invalid_import)?,
                    &bytes,
                )?;
                crate::persona::background::publish(runtime, ext, &bytes)?
            } else if role=="chat" {crate::chat_images::publish(runtime.paths.as_ref().map_err(Clone::clone)?,&bytes,value(&r,"filename")?)?
            } else {
                crate::accessories::images::publish(
                    runtime.paths.as_ref().map_err(Clone::clone)?,
                    &bytes,
                )?
            };
            source.connection.execute(
                "INSERT INTO import_published VALUES(?1,?2,?3)",
                params![path, role, published],
            )?;
            Ok(())
        })?;
    }
    Ok(())
}
fn resource(source: &Connection, r: &mut Record, key: &str, role: &str) -> AppResult<()> {
    if let Some(path) = r.get(key).and_then(Value::as_str) {
        let name: String = source.query_row(
            "SELECT filename FROM import_published WHERE path=?1 AND role=?2",
            params![path, role],
            |r| r.get(0),
        )?;
        r.insert(key.into(), Value::String(name));
    }
    Ok(())
}
fn foreign(source: &Connection, r: &mut Record) -> AppResult<()> {
    for (key, table) in [
        ("skill_id", "skills"),
        ("ledger_id", "currency_ledger"),
        ("provider_id", "providers"),
        ("message_id", "messages"),
        ("persona_id", "personas"),
        ("user_profile_id", "user_profiles"),
        ("session_id", "sessions"),
        ("round_id", "chat_rounds"),
        ("activated_by_round_id", "chat_rounds"),
        ("short_memory_id", "short_memories"),
        ("long_memory_id", "long_memories"),
        ("travel_id", "travels"),
        ("reply_to_id", "letters"),
        ("reminder_id", "reminders"),
        ("presentation_persona_id", "personas"),
        ("presentation_session_id", "sessions"),
        ("presentation_message_id", "messages"),
        ("game_run_id", "game_runs"),
    ] {
        map_field(source, r, key, table)?;
    }
    Ok(())
}
fn plugin_record(c: &Connection, id: &str) -> AppResult<Record> {
    let cols = super::super::protocol::COLUMNS
        .iter()
        .find(|(n, _)| *n == "game_plugins")
        .ok_or_else(invalid_import)?
        .1;
    let mut s = c.prepare(&format!(
        "SELECT {cols} FROM game_plugins WHERE plugin_id=?1"
    ))?;
    let mut cursor = s.query([id])?;
    let r = cursor.next()?.ok_or_else(stale)?;
    super::super::protocol::record(r, &cols.split(',').collect::<Vec<_>>())
}

/// 第一方SDK的已确认state.main协议：只改写待核对请求的宿主运行ID，不改变玩法进度或金额。
fn remap_first_party_request(source: &Connection, r: &mut Record) -> AppResult<()> {
    if r.get("data_key").and_then(Value::as_str) != Some("state.main")
        || !matches!(
            value(r, "plugin_id")?,
            "com.yanmew.game.rock-paper-scissors"
                | "com.yanmew.game.dice"
                | "com.yanmew.game.star-catcher"
                | "com.yanmew.game.my-home"
        )
    {
        return Ok(());
    }
    let mut envelope: Value =
        serde_json::from_str(value(r, "value_json")?).map_err(|_| invalid_import())?;
    if envelope.get("version").and_then(Value::as_i64) != Some(1) {
        return Ok(());
    }
    let Some(old) = envelope
        .pointer("/pending/request/game_run_id")
        .and_then(Value::as_str)
    else {
        return Ok(());
    };
    let next: Option<i64> = source
        .query_row(
            "SELECT target_id FROM import_id_map WHERE table_name='game_runs' AND source_id=?1",
            [parse_id(old)?],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(next) = next {
        envelope["pending"]["request"]["game_run_id"] = Value::String(next.to_string());
        r.insert("value_json".into(), Value::String(envelope.to_string()));
    }
    Ok(())
}
fn install_plugins(
    c: &Connection,
    p: &Pending,
    full: bool,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    let source = &p.stage.connection;
    rows(source, "game_plugins", |mut r| {
        let plugin_id = value(&r, "plugin_id")?.to_owned();
        for table in [
            "game_permissions",
            "game_plugin_settings",
            "game_plugin_data",
        ] {
            c.execute(
                &format!("DELETE FROM {table} WHERE plugin_id=?1"),
                [&plugin_id],
            )?;
        }
        if let Some(target) = p.plugins.get(&plugin_id) {
            let actual = plugin_record(c, &plugin_id)?;
            // 只使用本机已校验程序的身份、路径和权限声明，不接受包内自报的内置身份。
            for key in [
                "name",
                "description",
                "author",
                "installed_version",
                "api_version",
                "storage_version",
                "source_type",
                "package_path",
                "package_sha256",
                "network_origins_json",
                "is_bundled",
                "package_status",
                "installed_at",
            ] {
                r.insert(key.into(), actual[key].clone());
            }
            let enabled: i64 = source.query_row(
                "SELECT is_enabled FROM import_plugin_flags WHERE plugin_id=?1",
                [&plugin_id],
                |r| r.get(0),
            )?;
            r.insert("is_enabled".into(), Value::from(enabled));
            let _ = target;
        } else {
            let bundled: Option<i64> = c
                .query_row(
                    "SELECT is_bundled FROM game_plugins WHERE plugin_id=?1",
                    [&plugin_id],
                    |r| r.get(0),
                )
                .optional()?;
            r.insert("is_bundled".into(), Value::from(bundled.unwrap_or(0)));
            r.insert(
                "source_type".into(),
                Value::String(
                    if bundled == Some(1) {
                        "bundled"
                    } else {
                        "local"
                    }
                    .into(),
                ),
            );
            r.insert("package_path".into(), Value::Null);
            r.insert(
                "package_status".into(),
                Value::String("missing_package".into()),
            );
            r.insert("is_enabled".into(), Value::from(0));
            warnings.push(format!(
                "{plugin_id} 的兼容程序包未找到，已保留设置、存档及来源链接，请重新安装。"
            ));
        }
        c.execute("DELETE FROM game_plugins WHERE plugin_id=?1", [&plugin_id])?;
        put(c, "game_plugins", &r, false)?;
        Ok(())
    })?;
    for table in ["game_plugin_settings", "game_plugin_data"] {
        rows(source, table, |mut r| {
            if full && table == "game_plugin_data" {
                remap_first_party_request(source, &mut r)?;
            }
            put(c, table, &r, false)
        })?;
    }
    rows(source, "game_permissions", |mut r| {
        let plugin_id = value(&r, "plugin_id")?.to_owned();
        let original = value(&r, "plugin_version")?.to_owned();
        if let Some(scopes) = p.scopes.get(&plugin_id) {
            if !scopes.contains(&(
                value(&r, "permission")?.to_owned(),
                value(&r, "permission_scope")?.to_owned(),
            )) {
                return Ok(());
            }
        }
        if let Some(target) = p.plugins.get(&plugin_id) {
            if value(&r, "permission")? == "network.fetch"
                && !target
                    .network_origins
                    .iter()
                    .any(|o| r.get("permission_scope") == Some(&Value::String(o.clone())))
            {
                return Ok(());
            }
            let source_sha: String = source.query_row(
                "SELECT package_sha256 FROM game_plugins WHERE plugin_id=?1",
                [&plugin_id],
                |r| r.get(0),
            )?;
            if !full || original != target.installed_version || source_sha != target.package_sha256
            {
                r.insert("decision".into(), Value::String("pending".into()));
                r.insert("decided_at".into(), Value::Null);
            }
            r.insert(
                "plugin_version".into(),
                Value::String(target.installed_version.clone()),
            );
        } else if !full {
            r.insert("decision".into(), Value::String("pending".into()));
            r.insert("decided_at".into(), Value::Null);
        }
        put(c, "game_permissions", &r, false)
    })?;
    // 已安装版本的新权限不继承；缺失申请只以pending补齐。
    for target in p.plugins.values() {
        for (permission, scope) in p.scopes.get(&target.plugin_id).into_iter().flatten() {
            c.execute(
                "INSERT OR IGNORE INTO game_permissions VALUES(?1,?2,?3,'pending',?4,NULL)",
                params![
                    target.plugin_id,
                    permission,
                    scope,
                    target.installed_version
                ],
            )?;
        }
    }
    Ok(())
}
fn chats_to_stop(
    store: &Store,
    all: bool,
    profiles: &BTreeSet<i64>,
) -> AppResult<Vec<(i64, crate::chat::RequestSnapshot)>> {
    let mut affected = Vec::new();
    for snapshot in store.chat_requests.values() {
        let session = parse_id(&snapshot.session_id)?;
        let profile: Option<i64> = store.connection.query_row(
            "SELECT user_profile_id FROM sessions WHERE id=?1", [session], |r| r.get(0),
        )?;
        if all || profile.is_some_and(|p| profiles.contains(&p)) {
            affected.push((session, snapshot.clone()));
        }
    }
    Ok(affected)
}
pub(super) fn commit(
    runtime: &Runtime,
    store: &mut Store,
    p: &mut Pending,
    input: &Confirm,
) -> AppResult<Imported> {
    check(&store.connection, p, input)?;
    let persona = bindings(
        &store.connection,
        &p.preview.personas,
        &input.personas,
        "personas",
    )?;
    let profiles = bindings(
        &store.connection,
        &p.preview.profiles,
        &input.profiles,
        "user_profiles",
    )?;
    let source = &p.stage.connection;
    let full = p.preview.kind == PackageKind::Full;
    allocate(source, &store.connection, &persona, &profiles)?;
    // 节日code必须在目标母表存在；不能用来源数字ID或伪造日期代替。
    let mut codes = source.prepare("SELECT DISTINCT code FROM festival_catalog")?;
    for code in codes.query_map([], |r| r.get::<_, String>(0))? {
        let code = code?;
        if !store.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM festival_catalog WHERE code=?1)",
            [code],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(AppError::new(
                "BACKUP_FESTIVAL_UNKNOWN",
                "备份引用的内置节日尚不可用，请保留备份并更新应用后重试",
            ));
        }
    }
    assets(runtime, &p.stage, &persona)?;
    let changed_profiles = profiles
        .values()
        .copied()
        .flatten()
        .collect::<BTreeSet<_>>();
    let current_profile:Option<i64>=store.connection.query_row("SELECT user_profile_id FROM sessions WHERE id=(SELECT value FROM settings WHERE key='current_session_id')",[],|r|r.get(0)).optional()?.flatten();
    let current_profile_changed = current_profile.is_some_and(|id| changed_profiles.contains(&id));
    let chats = chats_to_stop(store, full, &changed_profiles)?;
    let now = runtime.clock.sample()?.utc;
    let date = now.to_rfc3339();
    let mut warnings = p.preview.warnings.clone();
    let tx = store
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch("PRAGMA defer_foreign_keys=ON;")?;
    if full {
        for table in ["real_operations","real_occurrences","real_entries","real_schedules","real_budgets","real_categories","real_assets"]{tx.execute(&format!("DELETE FROM {table}"),[])?;}
        tx.execute("DELETE FROM custom_festivals", [])?;
        crate::travel::cancel_current(&tx, &date)?;
        // 保留本机程序登记供兼容存档关联；不沿用其旧使用数据。
        for table in [
            "food_actions",
            "food_inventory",
            "persona_food_settings",
            "persona_skills",
            "skills",
            "reminder_email_deliveries",
            "currency_ledger",
            "game_runs",
            "game_permissions",
            "game_plugin_settings",
            "game_plugin_data",
            "reminder_occurrences",
            "reminders",
            "provider_roles",
            "provider_models",
            "providers",
            "settings",
            "user_state",
        ] {
            if table == "settings" {
                // 本机的一次性安装标记不随备份替换，避免删除过的角色重新出现。
                tx.execute("DELETE FROM settings WHERE key NOT IN (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", [crate::preset_personas::SEED_KEY,crate::preset_personas::DOCK_SEED_KEY,crate::preset_personas::WALK_SEED_KEY,crate::preset_personas::EMOTION_SEED_KEY,crate::preset_personas::LIFT_SEED_KEY,crate::preset_personas::ERROR_SEED_KEY,crate::preset_personas::TOSS_SEED_KEY,crate::preset_personas::WAIT_PET_SEED_KEY,crate::preset_personas::TOSS_PROPORTIONS_SEED_KEY,crate::preset_personas::SURPRISE_SEED_KEY,"pet_desktop_preferences"])?;
            } else {
                tx.execute(&format!("DELETE FROM {table}"), [])?;
            }
        }
        tx.execute("DELETE FROM embedding_providers WHERE is_builtin=0", [])?;
        tx.execute("UPDATE embedding_providers SET is_active=0", [])?;
    }
    rows(source, "personas", |mut r| {
        let old = id(&r, "id")?;
        if persona.get(&old).is_some_and(Option::is_some) {
            return Ok(());
        }
        map_field(source, &mut r, "id", "personas")?;
        r.insert("is_active".into(), Value::from(0));
        r.insert("is_builtin".into(), Value::from(0));
        r.insert(
            "name".into(),
            Value::String(unique_name(&tx, "personas", value(&r, "name")?)?),
        );
        resource(source, &mut r, "chat_bg_file", "background")?;
        put(&tx, "personas", &r, false)
    })?;

    rows(source,"skills",|mut r|{
        let old=id(&r,"id")?;
        let same:Option<i64>=tx.query_row("SELECT id FROM skills WHERE title=?1 AND summary=?2 AND body=?3 ORDER BY id LIMIT 1",params![value(&r,"title")?,value(&r,"summary")?,value(&r,"body")?],|r|r.get(0)).optional()?;
        if let Some(target)=same{source.execute("UPDATE import_id_map SET target_id=?1 WHERE table_name='skills' AND source_id=?2",params![target,old])?;}
        else{map_field(source,&mut r,"id","skills")?;put(&tx,"skills",&r,false)?;}Ok(())
    })?;
    for table in ["persona_food_settings","persona_skills"] {
        rows(source,table,|mut r|{let old=id(&r,"persona_id")?;if persona.get(&old).is_some_and(Option::is_some){return Ok(())}foreign(source,&mut r)?;if table=="persona_skills"&&tx.query_row("SELECT EXISTS(SELECT 1 FROM persona_skills WHERE persona_id=?1 AND skill_id=?2)",params![value(&r,"persona_id")?,value(&r,"skill_id")?],|r|r.get::<_,bool>(0))?{return Ok(())}put(&tx,table,&r,false)})?;
    }
    rows(source,"pet_artworks",|mut r|{let old_persona=id(&r,"persona_id")?;if persona.get(&old_persona).is_some_and(Option::is_some){return Ok(());}map_field(source,&mut r,"id","pet_artworks")?;foreign(source,&mut r)?;resource(source,&mut r,"image_file","accessory")?;resource(source,&mut r,"preview_file","accessory")?;put(&tx,"pet_artworks",&r,false)})?;
    rows(source, "user_profiles", |mut r| {
        let old = id(&r, "id")?;
        if let Some(Some(target)) = profiles.get(&old) {
            tx.execute(
                "UPDATE user_profiles SET name=?1,content=?2,updated_at=?3 WHERE id=?4",
                params![value(&r, "name")?, value(&r, "content")?, date, target],
            )?;
            tx.execute(
                "UPDATE sessions SET memory_revision=memory_revision+1 WHERE user_profile_id=?1",
                [target],
            )?;
        } else {
            map_field(source, &mut r, "id", "user_profiles")?;
            r.insert(
                "name".into(),
                Value::String(unique_name(&tx, "user_profiles", value(&r, "name")?)?),
            );
            put(&tx, "user_profiles", &r, false)?;
        }
        Ok(())
    })?;
    for table in [
        "sessions",
        "session_pet_states",
        "chat_rounds",
        "messages",
        "message_images",
        "short_memories",
        "long_memories",
        "long_memory_sources",
        "notebooks",
        "travels",
        "travel_journals",
        "accessories",
        "letters",
        "souvenirs",
    ] {
        rows(source, table, |mut r| {
            if r.contains_key("id") {
                map_field(source, &mut r, "id", table)?;
            }
            foreign(source, &mut r)?;
            if table=="messages" {if let Some(old)=r.get("expression").and_then(Value::as_str).and_then(|s|s.strip_prefix("@pet-image:")){if let Ok(old)=old.parse::<i64>(){let new:Option<i64>=source.query_row("SELECT target_id FROM import_id_map WHERE table_name='pet_artworks' AND source_id=?1",[old],|r|r.get(0)).optional()?;let new=new.filter(|n|tx.query_row("SELECT EXISTS(SELECT 1 FROM pet_artworks WHERE id=?1)",[n],|r|r.get::<_,bool>(0)).unwrap_or(false));r.insert("expression".into(),new.map(|n|Value::String(crate::pet_creation::marker(&n.to_string()))).unwrap_or(Value::Null));}}}

            if table == "chat_rounds" && r.get("status") == Some(&Value::String("pending".into())) {
                r.insert("status".into(), Value::String("interrupted".into()));
                r.insert("completed_at".into(), Value::String(date.clone()));
                r.insert(
                    "stop_reason".into(),
                    Value::String("recovered_after_restart".into()),
                );
            }
            if table == "message_images" {resource(source,&mut r,"image_file","chat")?;}
            if table == "accessories" || table == "souvenirs" {
                resource(source, &mut r, "image_file", "accessory")?;
            }
            put(&tx, table, &r, table == "session_pet_states")?;
            Ok(())
        })?;
    }
    if full {
        rows(source, "custom_festivals", |mut r| {
            map_field(source, &mut r, "id", "custom_festivals")?;
            {
                if let Some(old) = r.get("festival_id").and_then(Value::as_str) {
                    let code: String = source.query_row(
                        "SELECT code FROM festival_catalog WHERE id=?1",
                        [old],
                        |r| r.get(0),
                    )?;
                    let target: i64 = tx.query_row(
                        "SELECT id FROM festival_catalog WHERE code=?1",
                        [code],
                        |r| r.get(0),
                    )?;
                    r.insert("festival_id".into(), Value::String(target.to_string()));
                }
            }
            put(&tx, "custom_festivals", &r, false)?;
            {
                let id = parse_id(r.get("id").and_then(Value::as_str).ok_or_else(invalid_import)?)?;
                crate::festivals::custom::refresh_imported_in_transaction(
                    &tx, now.with_timezone(&chrono::Local).date_naive(), id,
                )?;
            }
            Ok(())
        })?;
    }
    if full {
        for table in [
            "providers",
            "provider_models",
            "provider_roles",
            "embedding_providers",
            "reminders",
            "reminder_occurrences",
            "user_state",
            "game_runs",
            "currency_ledger",
        ] {
            rows(source, table, |mut r| {
                if table != "user_state" && table!="provider_roles" && table!="provider_models" {
                    map_field(source, &mut r, "id", table)?;
                }
                foreign(source, &mut r)?;
                if table=="reminder_occurrences" && matches!(r.get("presentation_state").and_then(Value::as_str),Some("pending"|"generating")) {
                    r.insert("presentation_state".into(),Value::from("legacy"));
                }
                if table=="provider_roles" && r.get("purpose")==Some(&Value::String("image".into())){r.insert("enabled".into(),Value::from(0));}
                if table == "currency_ledger" && r.get("ref_id").is_some_and(|v| !v.is_null()) {
                    map_field(source, &mut r, "ref_id", "accessories")?;
                }
                if table == "game_runs" && r.get("status") == Some(&Value::String("running".into()))
                {
                    r.insert("status".into(), Value::String("error".into()));
                    r.insert("ended_at".into(), Value::String(date.clone()));
                }
                put(&tx, table, &r, false)
            })?;
        }
        for (table,_) in crate::real_money::TABLES{rows(source,table,|r|put(&tx,table,&r,false))?;}
        tx.execute("UPDATE real_schedules SET review_required=1",[])?;
        rows(source,"food_inventory",|r|put(&tx,"food_inventory",&r,false))?;
        rows(source,"food_actions",|mut r|{foreign(source,&mut r)?;r.insert("action_id".into(),Value::String(format!("import-{}-{}",date,value(&r,"action_id")?)));put(&tx,"food_actions",&r,false)})?;
        crate::db::ensure_builtin_embedding(&tx)?;
        rows(source, "settings", |mut r| {
            let key = value(&r, "key")?.to_owned();
            if key == "current_session_id" {
                let current = parse_id(value(&r, "value")?)?;
                r.insert(
                    "value".into(),
                    Value::String(mapped(source, "sessions", current)?.to_string()),
                );
            }
            if key == "reminder_email_config" {
                let safe = crate::reminder_email::public_config(value(&r, "value")?)?;
                r.insert("value".into(), Value::String(safe));
            }
            if key == "image_generation_config" || key == "web_search_config" {
                let mut config: Value =
                    serde_json::from_str(value(&r, "value")?).map_err(|_| invalid_import())?;
                config["enabled"] = Value::Bool(false);
                r.insert("value".into(), Value::String(config.to_string()));
            }
            put(&tx, "settings", &r, false)
        })?;
        for (key, default) in crate::config::DEFAULTS {
            tx.execute(
                "INSERT OR IGNORE INTO settings VALUES(?1,?2)",
                params![key, default.to_string()],
            )?;
        }
        let active: i64 =
            source.query_row("SELECT id FROM personas WHERE is_active=1", [], |r| {
                r.get(0)
            })?;
        tx.execute("UPDATE personas SET is_active=0", [])?;
        tx.execute(
            "UPDATE personas SET is_active=1 WHERE id=?1",
            [mapped(source, "personas", active)?],
        )?;
        crate::config::get(&tx)?;
        crate::provider_roles::migrate_legacy(&tx)?;
        crate::provider_roles::image_changed(&tx)?;
        crate::provider_roles::changed(&tx)?;
        crate::image_generation::get(&tx)?;
        install_plugins(&tx, p, true, &mut warnings)?;
        // 未列入备份的第三方登记不保留；当前便携版内置程序仍可作为无使用数据的插件。
        let kept = p.preview.plugins.iter().cloned().collect::<BTreeSet<_>>();
        let mut q = tx.prepare("SELECT plugin_id,is_bundled FROM game_plugins")?;
        let existing = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(q);
        for (id, bundled) in existing {
            if !kept.contains(&id) {
                if bundled {
                    tx.execute(
                        "UPDATE game_plugins SET is_enabled=0 WHERE plugin_id=?1",
                        [&id],
                    )?;
                } else {
                    tx.execute("DELETE FROM game_plugins WHERE plugin_id=?1", [&id])?;
                }
            }
        }
    } else if p.preview.kind == PackageKind::PluginData {
        install_plugins(&tx, p, false, &mut warnings)?;
    }
    // 先插入已分配ID的来源消息，再保存当前部分回复，避免自动消息ID碰撞。
    for (session, snapshot) in &chats {
        if !snapshot.regenerating {
            crate::sessions::interrupt_for_import(
                &tx, parse_id(&snapshot.persona_id)?, *session,
                parse_id(&snapshot.round_id)?, &snapshot.partial_content,
                snapshot.expression.as_deref(),
            )?;
        }
    }
    if tx
        .prepare("PRAGMA foreign_key_check")?
        .query([])?
        .next()?
        .is_some()
    {
        return Err(invalid_import());
    }
    // FTS使用同一事务重建，避免业务成功后出现可见的旧索引。
    let fts: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='long_memories_fts')",
        [],
        |r| r.get(0),
    )?;
    if fts
        && source.query_row("SELECT count(*) FROM long_memories", [], |r| {
            r.get::<_, i64>(0)
        })? > 0
    {
        tx.execute(
            "INSERT INTO long_memories_fts(long_memories_fts) VALUES('rebuild')",
            [],
        )?;
    }
    let sessions = source.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_, u64>(0))?;
    tx.commit()?;
    // 仍持有store锁；成功后才取消请求，失败保留原流式任务和内存片段。
    for (session, _) in chats {
        runtime.requests.cancel_key(&format!("chat:{session}"));
        store.chat_requests.remove(&session);
    }
    if full { runtime.requests.cancel_all(); }
    if full || current_profile_changed {
        store.context_revision = store.context_revision.saturating_add(1);
        store.pet.reset_context();
        store.accessory_crafts.discard();
    }
    if full {
        store.weather = Default::default();
        if crate::reminders::dispatch(&mut store.connection, now, &chrono::Local, true).is_err() {
            warnings.push("导入已保存；提醒补偿检查失败，请查看提醒页面后重试。".into());
        }
    }
    let classify = (|| -> AppResult<()> {
        let mut q=source.prepare("SELECT id,status,planned_return_at FROM travels WHERE gift_state='pending' AND status IN ('away','returned')")?;
        let mut cursor = q.query([])?;
        while let Some(r) = cursor.next()? {
            let due = chrono::DateTime::parse_from_rfc3339(&r.get::<_, String>(2)?)
                .map_err(|_| AppError::storage())?
                .with_timezone(&chrono::Utc)
                <= now;
            if due || r.get::<_, String>(1)? == "returned" {
                runtime.travel_gifts.restore_without_image(mapped(
                    source,
                    "travels",
                    r.get(0)?,
                )?)?;
            }
        }
        Ok(())
    })();
    if classify.is_err() {
        warnings.push("导入已保存；部分旅行恢复状态需重启后重新检查。".into());
    }

    Ok(Imported {
        sessions,
        personas_created: persona.values().filter(|v| v.is_none()).count() as u64,
        personas_reused: persona.values().filter(|v| v.is_some()).count() as u64,
        warnings,
    })
}
