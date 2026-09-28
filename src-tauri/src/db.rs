use crate::{
    config,
    error::{AppError, AppResult},
    platform::contracts::AppPaths,
};
use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = include_str!("../../db/schema.sql");
const VERSION: i64 = 3;
const PET_STATE_MIGRATION: &str = include_str!("../../db/migrations/002_session_pet_states.sql");
const INTIMACY_MIGRATION: &str = include_str!("../../db/migrations/003_intimacy_limit.sql");

/// 通过 SQLite 生成一致性快照；不能复制可能尚有 WAL 数据的数据库主文件。
fn backup_before_migration(connection: &Connection) -> AppResult<()> {
    let version =
        connection.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?;
    if !(1..VERSION).contains(&version) {
        return Ok(());
    }
    let Some(path) = connection.path().filter(|path| !path.is_empty()) else {
        // 内存测试库没有磁盘原文件；正式运行入口始终使用磁盘数据库。
        return Ok(());
    };
    let database_path = std::path::Path::new(path);
    let parent = database_path.parent().ok_or_else(AppError::storage)?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AppError::storage())?
        .as_nanos();
    let backup_path = parent.join(format!(
        "yanwenzi-before-v{VERSION}-{}-{unique}.db",
        std::process::id()
    ));
    let backup_path = backup_path.to_str().ok_or_else(AppError::storage)?;
    connection
        .execute("VACUUM main INTO ?1", [backup_path])
        .map_err(|_| {
            AppError::new(
                "STORAGE_ERROR",
                "数据库升级前备份失败，尚未升级；请检查磁盘空间与目录权限",
            )
        })?;
    Ok(())
}

pub fn open(paths: &AppPaths) -> AppResult<Connection> {
    std::fs::create_dir_all(&paths.data_root).map_err(|_| AppError::storage())?;
    let mut conn = Connection::open(paths.data_root.join("yanwenzi.db"))?;
    initialize(&mut conn)?;
    Ok(conn)
}

pub fn initialize(conn: &mut Connection) -> AppResult<()> {
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    if conn.pragma_query_value(None, "foreign_keys", |r| r.get::<_, i64>(0))? != 1 {
        return Err(AppError::storage());
    }
    backup_before_migration(conn)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let version: i64 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
    match version {
        0 => {
            let tables: i64 = tx.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )?;
            if tables != 0 {
                return Err(AppError::new(
                    "STORAGE_ERROR",
                    "此数据库没有可识别的版本，请保留原文件并使用匹配的应用版本",
                ));
            }
            tx.execute_batch(SCHEMA)?;
            tx.pragma_update(None, "user_version", VERSION)?;
        }
        1 => {
            tx.execute_batch(PET_STATE_MIGRATION)?;
            tx.execute_batch(INTIMACY_MIGRATION)?;
            tx.pragma_update(None, "user_version", VERSION)?;
        }
        2 => {
            tx.execute_batch(INTIMACY_MIGRATION)?;
            tx.pragma_update(None, "user_version", VERSION)?;
        }
        VERSION => {}
        _ => {
            return Err(AppError::new(
                "STORAGE_ERROR",
                "数据库版本不受当前应用支持，请使用匹配的应用版本；原数据未修改",
            ))
        }
    }
    for (key, value) in config::defaults() {
        tx.execute(
            "INSERT OR IGNORE INTO settings(key,value) VALUES (?1,?2)",
            params![key, value],
        )?;
    }
    tx.execute(
        "INSERT OR IGNORE INTO user_state(id,coins,updated_at) VALUES (1,0,?1)",
        [chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)],
    )?;
    tx.execute("INSERT INTO personas(name,prompt,initial_prompt,is_builtin) SELECT '喵喵桌宠',?1,?1,1 WHERE NOT EXISTS(SELECT 1 FROM personas WHERE is_builtin=1)",["你是喵喵桌宠，用自然、亲切的中文与用户交流。"])?;
    tx.execute("UPDATE personas SET is_active=1 WHERE is_builtin=1 AND NOT EXISTS(SELECT 1 FROM personas WHERE is_active=1)",[])?;
    tx.execute("INSERT INTO embedding_providers(name,kind,model,is_builtin) SELECT '内置中文 Embedding','local','BAAI/bge-small-zh-v1.5',1 WHERE NOT EXISTS(SELECT 1 FROM embedding_providers WHERE is_builtin=1)",[])?;
    tx.execute("UPDATE embedding_providers SET is_active=1 WHERE is_builtin=1 AND NOT EXISTS(SELECT 1 FROM embedding_providers WHERE is_active=1)",[])?;
    // current_session_id 无外键；启动时只保留归属当前人设的有效会话。
    let current: Option<String> = tx
        .query_row(
            "SELECT value FROM settings WHERE key='current_session_id'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(current) = current {
        let valid = match crate::error::parse_id(&current) {
            Ok(id) => tx.query_row("SELECT EXISTS(SELECT 1 FROM sessions s JOIN personas p ON p.id=s.persona_id WHERE s.id=?1 AND p.is_active=1)",[id],|r|r.get::<_,bool>(0))?,
            Err(_) => false,
        };
        if !valid {
            tx.execute("DELETE FROM settings WHERE key='current_session_id'", [])?;
        }
    }
    crate::sessions::recover_pending_in_transaction(
        &tx,
        &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    )?;
    config::get(&tx)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_one_fixture() -> Connection {
        let connection = memory();
        connection.execute_batch(
            "DROP TRIGGER sessions_intimacy_insert;
             DROP TRIGGER sessions_intimacy_update;
             DROP TRIGGER sessions_create_pet_state;
             DROP TABLE session_pet_states;
             PRAGMA user_version=1;
             INSERT INTO sessions(id,persona_id,title,created_at,updated_at)
             VALUES(1,1,'保留的会话','2026-09-20T00:00:00.000Z','2026-09-20T00:00:00.000Z');
             UPDATE user_state SET coins=37;
             INSERT INTO providers(name,model,api_key_encrypted) VALUES('保留的提供商','model','opaque-secret');",
        ).unwrap();
        connection
    }

    fn objects(connection: &Connection) -> Vec<(String, String)> {
        connection.prepare("SELECT name,sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap().query_map([], |row| Ok((row.get(0)?,row.get(1)?)))
            .unwrap().collect::<Result<Vec<_>,_>>().unwrap()
    }

    #[test]
    fn version_two_caps_intimacy_preserves_state_and_rolls_back_on_failure() {
        for fail in [true, false] {
            let mut connection = memory();
            connection
                .execute_batch(
                    "DROP TRIGGER sessions_intimacy_insert;
                DROP TRIGGER sessions_intimacy_update;
                PRAGMA user_version=2;
                INSERT INTO sessions(id,persona_id,title,created_at,updated_at,intimacy)
                VALUES(1,1,'保留','a','a',1005);
                UPDATE session_pet_states SET hunger_elapsed_ms=10000;
                UPDATE settings SET value='1000' WHERE key='intimacy_threshold_1';
                UPDATE settings SET value='1001' WHERE key='intimacy_threshold_2';
                UPDATE settings SET value='1002' WHERE key='intimacy_threshold_3';
                UPDATE settings SET value='1003' WHERE key='intimacy_threshold_4';",
                )
                .unwrap();
            if fail {
                connection
                    .execute(
                        "UPDATE settings SET value='bad' WHERE key='hungry_minutes'",
                        [],
                    )
                    .unwrap();
                assert!(initialize(&mut connection).is_err());
                assert_eq!(
                    connection
                        .query_row("SELECT intimacy FROM sessions", [], |r| r.get::<_, i64>(0))
                        .unwrap(),
                    1005
                );
                assert_eq!(
                    connection
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                        .unwrap(),
                    2
                );
            } else {
                initialize(&mut connection).unwrap();
                assert_eq!(objects(&connection), objects(&memory()));
                assert_eq!(
                    connection
                        .query_row("SELECT intimacy FROM sessions", [], |r| r.get::<_, i64>(0))
                        .unwrap(),
                    999
                );
                assert_eq!(
                    crate::pet_store::read(&connection, 1)
                        .unwrap()
                        .hunger_elapsed_ms,
                    10000
                );
                let values = config::get(&connection).unwrap();
                assert_eq!(values["intimacy_threshold_1"], "996");
                assert_eq!(values["intimacy_threshold_4"], "999");
                assert!(config::save(
                    &mut connection,
                    [("intimacy_threshold_4".into(), "1000".into())].into()
                )
                .is_err());
                assert!(connection.execute("INSERT INTO sessions(persona_id,title,created_at,updated_at,intimacy) VALUES(1,'非法','a','a',1000)", []).is_err());
            }
        }
    }

    #[test]
    fn version_one_migration_preserves_data_and_matches_new_schema() {
        let mut connection = version_one_fixture();
        initialize(&mut connection).unwrap();
        assert_eq!(objects(&connection), objects(&memory()));
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            VERSION
        );
        assert_eq!(
            connection
                .query_row("SELECT coins FROM user_state", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            37
        );
        assert_eq!(
            connection
                .query_row("SELECT api_key_encrypted FROM providers", [], |row| row
                    .get::<_, String>(
                    0
                ))
                .unwrap(),
            "opaque-secret"
        );
        let state = crate::pet_store::read(&connection, 1).unwrap();
        assert_eq!(state.hunger_elapsed_ms, 0);
        assert_eq!(state.idle_branch, crate::pet_store::IdleBranch::None);
        assert!(state.last_fed_at.is_none());
        initialize(&mut connection).unwrap();
        assert_eq!(crate::pet_store::read(&connection, 1).unwrap(), state);
    }

    #[test]
    fn failed_migration_rolls_back_version_and_schema() {
        let mut connection = version_one_fixture();
        connection
            .execute(
                "UPDATE settings SET value='bad' WHERE key='hungry_minutes'",
                [],
            )
            .unwrap();
        let before = objects(&connection);
        assert!(initialize(&mut connection).is_err());
        assert_eq!(objects(&connection), before);
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            connection
                .query_row("SELECT coins FROM user_state", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            37
        );
    }

    #[test]
    fn disk_migration_creates_readable_original_backup_and_backup_failure_blocks_upgrade() {
        let root = std::env::temp_dir().join(format!(
            "yanmew-migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let database_path = root.join("yanwenzi.db");
        version_one_fixture()
            .execute("VACUUM INTO ?1", [database_path.to_str().unwrap()])
            .unwrap();
        {
            // 在已有事务中 VACUUM 必须失败，模拟升级前快照无法建立。
            let mut readonly = Connection::open_with_flags(
                &database_path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            readonly.pragma_update(None, "foreign_keys", true).unwrap();
            readonly.execute_batch("BEGIN").unwrap();
            assert!(initialize(&mut readonly).is_err());
            readonly.execute_batch("ROLLBACK").unwrap();
            assert_eq!(
                readonly
                    .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                    .unwrap(),
                1
            );
        }
        let paths = AppPaths {
            data_root: root.clone(),
        };
        drop(open(&paths).unwrap());
        let backups: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(&format!("yanwenzi-before-v{VERSION}-"))
            })
            .collect();
        assert_eq!(backups.len(), 1);
        {
            let backup = Connection::open(&backups[0]).unwrap();
            assert_eq!(
                backup
                    .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                    .unwrap(),
                1
            );
            assert_eq!(
                backup
                    .query_row("SELECT coins FROM user_state", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                37
            );
            assert_eq!(
                backup
                    .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
        }
        drop(open(&paths).unwrap());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        for backup in backups {
            std::fs::remove_file(backup).unwrap();
        }
        std::fs::remove_file(database_path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn disk_reopen_preserves_data_and_invalid_directory_fails() {
        let unique = format!(
            "yanmew-db-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let paths = AppPaths {
            data_root: root.clone(),
        };
        {
            let c = open(&paths).unwrap();
            c.execute("UPDATE user_state SET coins=17", []).unwrap();
        }
        {
            let c = open(&paths).unwrap();
            assert_eq!(
                c.query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                17
            );
        }
        let blocker = root.join("not-a-directory");
        std::fs::write(&blocker, b"keep").unwrap();
        assert!(open(&AppPaths {
            data_root: blocker.clone()
        })
        .is_err());
        assert_eq!(std::fs::read(&blocker).unwrap(), b"keep");
        std::fs::remove_file(blocker).unwrap();
        std::fs::remove_file(root.join("yanwenzi.db")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn transaction_failure_does_not_partially_save_or_seed() {
        let mut c = memory();
        c.execute_batch("CREATE TRIGGER reject_theme BEFORE UPDATE ON settings WHEN NEW.key='theme_mode' BEGIN SELECT RAISE(ABORT,'test failure'); END;").unwrap();
        assert!(config::save(
            &mut c,
            [
                ("memory_merge_count".into(), "5".into()),
                ("theme_mode".into(), "dark".into())
            ]
            .into()
        )
        .is_err());
        assert_eq!(config::get(&c).unwrap()["memory_merge_count"], "20");
        c.execute("DELETE FROM settings WHERE key='hungry_minutes'", [])
            .unwrap();
        c.execute(
            "UPDATE settings SET value='bad' WHERE key='memory_merge_count'",
            [],
        )
        .unwrap();
        assert!(initialize(&mut c).is_err());
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM settings WHERE key='hungry_minutes'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }
    pub fn memory() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        initialize(&mut c).unwrap();
        c
    }
    #[test]
    fn seeds_are_idempotent_and_foreign_keys_enforced() {
        let mut c = memory();
        c.execute("UPDATE user_state SET coins=23", []).unwrap();
        c.execute(
            "UPDATE settings SET value='7' WHERE key='memory_merge_count'",
            [],
        )
        .unwrap();
        initialize(&mut c).unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM personas", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            23
        );
        assert_eq!(config::get(&c).unwrap()["memory_merge_count"], "7");
        assert!(c.execute("INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(999,'x','x','x')",[]).is_err());
        // 实际执行 MATCH，确认运行时 SQLite 包含 FTS5/trigram。
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM long_memories_fts WHERE long_memories_fts MATCH '测试检索'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }
    #[test]
    fn unknown_database_is_not_rebuilt() {
        let mut c = Connection::open_in_memory().unwrap();
        c.execute_batch("CREATE TABLE keep_me(value); INSERT INTO keep_me VALUES(42);")
            .unwrap();
        assert!(initialize(&mut c).is_err());
        assert_eq!(
            c.query_row("SELECT value FROM keep_me", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            42
        );
        c.pragma_update(None, "user_version", 99).unwrap();
        assert!(initialize(&mut c).is_err());
    }
    #[test]
    fn settings_validate_and_rollback_as_a_batch() {
        let mut c = memory();
        let changes = [
            ("theme_mode".into(), "dark".into()),
            ("travel_min_online_hours".into(), "8".into()),
        ]
        .into();
        assert!(config::save(&mut c, changes).is_err());
        assert_eq!(config::get(&c).unwrap()["theme_mode"], "system");
        assert!(config::save(&mut c, [("current_session_id".into(), "1".into())].into()).is_err());
        assert!(config::save(&mut c, [("theme_mode".into(), "blue".into())].into()).is_err());
        config::save(&mut c, [("memory_merge_count".into(), "10".into())].into()).unwrap();
        assert_eq!(config::get(&c).unwrap()["memory_merge_count"], "10");
    }
}
