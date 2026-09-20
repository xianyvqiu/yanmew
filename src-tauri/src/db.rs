use crate::{
    config,
    error::{AppError, AppResult},
    platform::contracts::AppPaths,
};
use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = include_str!("../../db/schema.sql");
const VERSION: i64 = 1;

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
    config::get(&tx)?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
