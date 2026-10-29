use crate::{
    config,
    error::{AppError, AppResult},
    platform::contracts::AppPaths,
};
use rusqlite::{params, Connection, OptionalExtension};

const SCHEMA: &str = include_str!("../../db/schema.sql");
const VERSION: i64 = 26;
const CHAT_STICKERS_MIGRATION: &str = include_str!("../../db/migrations/026_chat_stickers.sql");
#[cfg(test)]
mod reminder_presentation_migration_tests {
    use super::*;
    #[test]
    fn v24_reminder_upgrade_keeps_legacy_records_and_sequences(){
        let mut c=test_database();
        c.execute_batch("DROP INDEX idx_reminder_presentation_pending;ALTER TABLE reminder_occurrences DROP COLUMN presentation_message_id;ALTER TABLE reminder_occurrences DROP COLUMN presentation_text;ALTER TABLE reminder_occurrences DROP COLUMN presentation_input;ALTER TABLE reminder_occurrences DROP COLUMN presentation_session_id;ALTER TABLE reminder_occurrences DROP COLUMN presentation_persona_id;ALTER TABLE reminder_occurrences DROP COLUMN presentation_state;PRAGMA user_version=24;INSERT INTO reminders(id,name,kind,content,status,source,created_at) VALUES(7,'旧提醒','once','保留内容','pending','manual','2026-10-23T00:00:00Z');INSERT INTO reminder_occurrences(id,reminder_id,scheduled_at,status,delivery_state,created_at) VALUES(17,7,'2026-10-23T01:00:00Z','pending','queued','2026-10-23T01:00:00Z'),(99,7,'2026-10-24T01:00:00Z','expired','shown','2026-10-24T01:00:00Z');DELETE FROM reminder_occurrences WHERE id=99;").unwrap();
        initialize(&mut c).unwrap();let row=crate::reminders::get_occurrence(&c,17).unwrap();assert_eq!(row.content,"保留内容");assert_eq!(row.status,crate::models::ReminderStatus::Pending);assert!(!row.auto_completed);
        assert_eq!(c.query_row("SELECT presentation_state FROM reminder_occurrences WHERE id=17",[],|r|r.get::<_,String>(0)).unwrap(),"legacy");
        c.execute("INSERT INTO reminder_occurrences(reminder_id,scheduled_at,status,delivery_state,created_at) VALUES(7,'2026-10-25T01:00:00Z','expired','shown','2026-10-25T01:00:00Z')",[]).unwrap();assert!(c.last_insert_rowid()>99);
        assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());initialize(&mut c).unwrap();
    }
}
const REMINDER_PRESENTATIONS_MIGRATION: &str = include_str!("../../db/migrations/025_reminder_presentations.sql");
const SOUVENIR_TITLES_MIGRATION:&str=include_str!("../../db/migrations/024_souvenir_titles.sql");
const PET_WAIT_MIGRATION:&str=include_str!("../../db/migrations/023_pet_wait_pet.sql");
const PET_TOSS_MIGRATION:&str=include_str!("../../db/migrations/022_pet_toss.sql");
#[cfg(test)]
mod toss_migration_tests {
    use super::*;
    #[test]
    fn v21_toss_upgrade_preserves_artwork_files_flags_and_sequence() {
        let mut c=test_database();
        c.execute_batch(PET_ERROR_MIGRATION).unwrap();
        c.execute_batch("INSERT INTO pet_artworks(id,persona_id,kind,name,category,enabled,image_file,preview_file,revision,created_at,updated_at) VALUES(17,1,'expression','自定义超载','error',0,'assets/accessories/old.png','assets/accessories/preview.png',7,'a','b'),(99,1,'expression','已删除','normal',1,NULL,NULL,1,'a','b');DELETE FROM pet_artworks WHERE id=99;PRAGMA user_version=21;").unwrap();
        initialize(&mut c).unwrap();
        let row:(String,String,i64,String,String,i64)=c.query_row("SELECT name,category,enabled,image_file,preview_file,revision FROM pet_artworks WHERE id=17",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).unwrap();
        assert_eq!(row,("自定义超载".into(),"error".into(),0,"assets/accessories/old.png".into(),"assets/accessories/preview.png".into(),7));
        for category in ["toss_up","toss_down","landing"]{c.execute("INSERT INTO pet_artworks(persona_id,kind,name,category,created_at,updated_at) VALUES(1,'expression',?1,?1,'a','b')",[category]).unwrap();assert!(c.last_insert_rowid()>99);}
        assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
        initialize(&mut c).unwrap();assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
    }
}
const SOUVENIR_IMAGES_MIGRATION: &str = include_str!("../../db/migrations/021_souvenir_images.sql");
#[cfg(test)]
mod souvenir_image_migration_tests {
    use super::*;
    #[test]
    fn v20_souvenir_upgrade_preserves_text_and_sequence(){
        let mut c=test_database();
        c.execute_batch("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(81,1,'原会话','2026-10-01T00:00:00Z','2026-10-01T00:00:00Z'); INSERT INTO souvenirs(id,session_id,content,note,is_favorite,created_at) VALUES(17,81,'原礼物','原备注',1,'2026-10-01T00:00:00Z'),(99,81,'删除项','备注',0,'2026-10-01T00:00:00Z'); DELETE FROM souvenirs WHERE id=99; ALTER TABLE souvenirs DROP COLUMN image_file; PRAGMA user_version=20;").unwrap();
        initialize(&mut c).unwrap();let gift=crate::souvenirs::get(&c,81,17).unwrap();assert_eq!(gift.content,"原礼物");assert_eq!(gift.note,"原备注");assert!(gift.is_favorite);assert!(gift.image_file.is_none());
        c.execute("INSERT INTO souvenirs(session_id,content,note,created_at) VALUES(81,'新礼物','备注','2026-10-02T00:00:00Z')",[]).unwrap();assert!(c.last_insert_rowid()>99);
        assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);initialize(&mut c).unwrap();assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
    }
}
const PET_ERROR_MIGRATION: &str = include_str!("../../db/migrations/020_pet_error.sql");
const PET_LAYERS_MIGRATION: &str = include_str!("../../db/migrations/019_pet_layers.sql");
const REAL_MONEY_MIGRATION: &str = include_str!("../../db/migrations/018_real_money.sql");
const WALKING_MIGRATION: &str = include_str!("../../db/migrations/017_pet_walking.sql");
const DOCKING_MIGRATION: &str = include_str!("../../db/migrations/016_pet_docking.sql");
const FOOD_SKILL_MIGRATION: &str = include_str!("../../db/migrations/015_foods_and_skills.sql");
const STANDBY_MIGRATION: &str = include_str!("../../db/migrations/014_pet_standby.sql");
const CUSTOM_FESTIVALS_MIGRATION: &str = include_str!("../../db/migrations/013_global_custom_festivals.sql");
const PET_CREATION_MIGRATION: &str = include_str!("../../db/migrations/012_pet_creation.sql");
const SESSION_PINNING_MIGRATION: &str = include_str!("../../db/migrations/011_session_pinning.sql");
const CHAT_APPEARANCE_MIGRATION: &str = include_str!("../../db/migrations/010_chat_appearance.sql");
const REMINDER_EMAIL_MIGRATION: &str = include_str!("../../db/migrations/009_reminder_email.sql");
const PROVIDER_MODELS_MIGRATION: &str = include_str!("../../db/migrations/008_provider_models.sql");
const MODEL_ROLES_MIGRATION: &str = include_str!("../../db/migrations/007_model_roles_and_chat_images.sql");
const ACCESSORY_IMAGES_MIGRATION: &str = include_str!("../../db/migrations/006_accessory_images.sql");
const PROACTIVE_MIGRATION: &str = include_str!("../../db/migrations/005_proactive_messages.sql");
const PET_STATE_MIGRATION: &str = include_str!("../../db/migrations/002_session_pet_states.sql");
const INTIMACY_MIGRATION: &str = include_str!("../../db/migrations/003_intimacy_limit.sql");
const CHAT_BUDGET_MIGRATION: &str = include_str!("../../db/migrations/004_chat_budget.sql");

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

pub fn open(paths: &AppPaths) -> AppResult<(Connection, bool)> {
    std::fs::create_dir_all(&paths.data_root).map_err(|_| AppError::storage())?;
    let mut conn = Connection::open(paths.data_root.join("yanwenzi.db"))?;
    let created_initial_session = initialize(&mut conn)?;
    Ok((conn, created_initial_session))
}

/// 返回本次是否原子创建了初始教程；仅用于本次启动导航，不持久化已读标记。
pub fn initialize(conn: &mut Connection) -> AppResult<bool> {
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
            tx.execute_batch(CHAT_BUDGET_MIGRATION)?;
            tx.pragma_update(None, "user_version", VERSION)?;
        }
        2 => {
            tx.execute_batch(INTIMACY_MIGRATION)?;
            tx.execute_batch(CHAT_BUDGET_MIGRATION)?;
            tx.pragma_update(None, "user_version", VERSION)?;
        }
        3 => {
            tx.execute_batch(CHAT_BUDGET_MIGRATION)?;
            tx.pragma_update(None, "user_version", VERSION)?;
        }
        4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 | 25 => {}
        VERSION => {}
        _ => {
            return Err(AppError::new(
                "STORAGE_ERROR",
                "数据库版本不受当前应用支持，请使用匹配的应用版本；原数据未修改",
            ))
        }
    }
    if (1..5).contains(&version) {
        tx.execute_batch(PROACTIVE_MIGRATION)?;
        tx.pragma_update(None, "user_version", VERSION)?;
    }
    if (1..6).contains(&version) {
        tx.execute_batch(ACCESSORY_IMAGES_MIGRATION)?;
        tx.pragma_update(None, "user_version", VERSION)?;
    }
    if (1..7).contains(&version) {
        tx.execute_batch(MODEL_ROLES_MIGRATION)?;
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..8).contains(&version) {
        tx.execute_batch(PROVIDER_MODELS_MIGRATION)?;
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..9).contains(&version) {
        tx.execute_batch(REMINDER_EMAIL_MIGRATION)?;
        tx.pragma_update(None, "user_version", VERSION)?;
    }
    if (1..10).contains(&version) {
        tx.execute_batch(CHAT_APPEARANCE_MIGRATION)?;
        tx.pragma_update(None, "user_version", VERSION)?;
    }
    if (1..11).contains(&version) {
        tx.execute_batch(SESSION_PINNING_MIGRATION)?;
        tx.pragma_update(None, "user_version", VERSION)?;
    }
    if (1..12).contains(&version) { tx.execute_batch(PET_CREATION_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    if (1..13).contains(&version) {
        // 允许已转换结构的开发库保持幂等；真实旧库在此逐条搬迁。
        if tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='session_festivals')", [], |r| r.get::<_,bool>(0))? {
            tx.execute_batch(CUSTOM_FESTIVALS_MIGRATION)?;
        }
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..14).contains(&version) {
        let sql: String = tx.query_row("SELECT sql FROM sqlite_master WHERE name='pet_artworks'", [], |r| r.get(0))?;
        if !sql.contains("'standby'") { tx.execute_batch(STANDBY_MIGRATION)?; }
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..15).contains(&version) { tx.execute_batch(FOOD_SKILL_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    if (1..16).contains(&version) { tx.execute_batch(DOCKING_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    if (1..17).contains(&version) { tx.execute_batch(WALKING_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    if (1..18).contains(&version) { tx.execute_batch(REAL_MONEY_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    if (1..19).contains(&version) { tx.execute_batch(PET_LAYERS_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    if (1..20).contains(&version) { tx.execute_batch(PET_ERROR_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    if (1..21).contains(&version) {
        let exists=tx.prepare("PRAGMA table_info(souvenirs)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?.iter().any(|name|name=="image_file");
        if !exists {tx.execute_batch(SOUVENIR_IMAGES_MIGRATION)?;}
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..22).contains(&version) {
        let sql:String=tx.query_row("SELECT sql FROM sqlite_master WHERE name='pet_artworks'",[],|r|r.get(0))?;
        if !sql.contains("'toss_up'") {tx.execute_batch(PET_TOSS_MIGRATION)?;}
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..23).contains(&version) {
        let sql:String=tx.query_row("SELECT sql FROM sqlite_master WHERE name='pet_artworks'",[],|r|r.get(0))?;
        if !sql.contains("'wait_pet'") {tx.execute_batch(PET_WAIT_MIGRATION)?;}
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..24).contains(&version) {
        let exists=tx.prepare("PRAGMA table_info(souvenirs)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?.iter().any(|name|name=="title");
        if !exists {tx.execute_batch(SOUVENIR_TITLES_MIGRATION)?;}
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..25).contains(&version) {
        let exists=tx.prepare("PRAGMA table_info(reminder_occurrences)")?.query_map([],|r|r.get::<_,String>(1))?.collect::<Result<Vec<_>,_>>()?.iter().any(|name|name=="presentation_state");
        if !exists {tx.execute_batch(REMINDER_PRESENTATIONS_MIGRATION)?;}
        tx.pragma_update(None,"user_version",VERSION)?;
    }
    if (1..26).contains(&version) && !tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='sticker_libraries')", [], |r| r.get::<_,bool>(0))? { tx.execute_batch(CHAT_STICKERS_MIGRATION)?; tx.pragma_update(None,"user_version",VERSION)?; }
    crate::real_money::seed(&tx)?;
    crate::foods::seed(&tx)?;
    config::normalize_legacy_idle_settings(&tx)?;
    crate::provider_roles::migrate_legacy(&tx)?;
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
    seed_default_persona(&tx)?;
    tx.execute("INSERT OR IGNORE INTO persona_food_settings(persona_id,default_food_id,favorite_food_id,updated_at) SELECT id,'token','dried_fish',?1 FROM personas WHERE is_builtin=1",[chrono::Utc::now().to_rfc3339()])?;
    let created_initial_session = version == 0;
    if created_initial_session {
        tx.execute("INSERT OR IGNORE INTO settings(key,value) VALUES ('onboarding_pending','true')", [])?;
        crate::sessions::create_initial_in_transaction(&tx)?;
    }
    ensure_builtin_embedding(&tx)?;
    crate::festivals::ensure_in_transaction(&tx, chrono::Local::now().date_naive())?;
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
    Ok(created_initial_session)
}

/// 既有业务测试从已初始化的空会话库开始；新库教程单独通过真实 initialize 验证。
#[cfg(test)]
pub(crate) fn test_database() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    connection.execute_batch(SCHEMA).unwrap();
    connection.pragma_update(None, "user_version", VERSION).unwrap();
    assert!(!initialize(&mut connection).unwrap());
    connection
}

/// 种子在初始化事务内维护；保留用户修改过的名称、正文和初始设定。
fn seed_default_persona(tx: &rusqlite::Transaction<'_>) -> AppResult<()> {
    const NAME: &str = "电子小猫";
    const PROMPT: &str = "你是电子小猫，用自然、亲切的中文与用户交流。";
    const OLD_PROMPT: &str = "你是喵喵桌宠，用自然、亲切的中文与用户交流。";
    let count: i64 = tx.query_row(
        "SELECT count(*) FROM personas WHERE is_builtin=1", [], |r| r.get(0),
    )?;
    if count > 1 {
        return Err(AppError::new("STORAGE_ERROR", "存在多个系统默认人设，数据不一致；请保留原数据并检查备份"));
    }
    if count == 0 {
        let prompt=format!("{PROMPT}你最喜欢的食物是小鱼干。用户之后可在食物设置修改喜好，以当前保存的配置为准。");
        tx.execute("INSERT INTO personas(name,prompt,initial_prompt,is_builtin,created_at,updated_at,chat_user_text_color,chat_assistant_text_color) VALUES(?1,?2,?2,1,strftime('%Y-%m-%dT%H:%M:%fZ','now'),strftime('%Y-%m-%dT%H:%M:%fZ','now'),NULL,NULL)", params![NAME, prompt])?;
    } else {
        // 只替换上一版种子的原值；三项独立判断，不覆盖手动编辑或人设自调。
        tx.execute("UPDATE personas SET name=CASE WHEN name='喵喵桌宠' THEN ?1 ELSE name END,
            prompt=CASE WHEN prompt=?3 THEN ?2 ELSE prompt END,
            initial_prompt=CASE WHEN initial_prompt=?3 THEN ?2 ELSE initial_prompt END
            WHERE is_builtin=1", params![NAME, PROMPT, OLD_PROMPT])?;
    }
    tx.execute("UPDATE personas SET is_active=1 WHERE is_builtin=1 AND NOT EXISTS(SELECT 1 FROM personas WHERE is_active=1)",[])?;
    Ok(())
}

#[cfg(test)]
#[path = "db/onboarding_tests.rs"]
mod onboarding_tests;

#[cfg(test)]
#[path = "db/custom_festival_tests.rs"]
mod custom_festival_tests;

#[cfg(test)]
#[path = "db/standby_tests.rs"]
mod standby_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_persona_is_idempotent_and_missing_builtin_is_seeded() {
        let mut c = memory();
        initialize(&mut c).unwrap();
        let initial: (String, String, bool) = c.query_row("SELECT name,prompt,is_active FROM personas WHERE is_builtin=1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(initial, ("电子小猫".into(), "你是电子小猫，用自然、亲切的中文与用户交流。你最喜欢的食物是小鱼干。用户之后可在食物设置修改喜好，以当前保存的配置为准。".into(), true));
        assert_eq!(c.query_row("SELECT count(*) FROM personas", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
        c.execute_batch("UPDATE personas SET is_active=0; INSERT INTO personas(name,prompt,initial_prompt,is_active) VALUES('我的人设','内容','初始',1); DELETE FROM personas WHERE is_builtin=1;").unwrap();
        initialize(&mut c).unwrap();
        assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE is_builtin=1", [], |r| r.get::<_,i64>(0)).unwrap(), 1);
        assert_eq!(c.query_row("SELECT name FROM personas WHERE is_active=1", [], |r| r.get::<_,String>(0)).unwrap(), "我的人设");
        assert_eq!(c.query_row("SELECT count(*) FROM sessions", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    }

    #[test]
    fn default_persona_updates_seed_values_but_preserves_user_edits() {
        let mut c = memory();
        c.execute("UPDATE personas SET name='喵喵桌宠',prompt=?1,initial_prompt=?1 WHERE is_builtin=1", ["你是喵喵桌宠，用自然、亲切的中文与用户交流。"]).unwrap();
        initialize(&mut c).unwrap();
        assert_eq!(c.query_row("SELECT name FROM personas WHERE is_builtin=1", [], |r| r.get::<_,String>(0)).unwrap(), "电子小猫");
        c.execute("UPDATE personas SET name='自己起的名字',prompt='我编辑的正文',initial_prompt='我编辑的初始设定' WHERE is_builtin=1", []).unwrap();
        initialize(&mut c).unwrap();
        let values: (String,String,String) = c.query_row("SELECT name,prompt,initial_prompt FROM personas WHERE is_builtin=1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(values, ("自己起的名字".into(), "我编辑的正文".into(), "我编辑的初始设定".into()));
    }

    #[test]
    fn default_persona_inconsistent_duplicates_fail_without_deleting_data() {
        let mut c = memory();
        c.execute_batch("DROP INDEX idx_personas_one_builtin; INSERT INTO personas(name,prompt,initial_prompt,is_builtin) VALUES('第二个内置','保留','保留',1); DELETE FROM settings WHERE key='hungry_minutes';").unwrap();
        assert!(initialize(&mut c).unwrap_err().message.contains("多个系统默认人设"));
        assert_eq!(c.query_row("SELECT count(*) FROM personas WHERE is_builtin=1", [], |r| r.get::<_,i64>(0)).unwrap(), 2);
        assert_eq!(c.query_row("SELECT count(*) FROM settings WHERE key='hungry_minutes'", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
    }

    fn remove_v12_fixture_columns(connection:&Connection){connection.execute_batch("ALTER TABLE accessories DROP COLUMN render_layer;ALTER TABLE accessories DROP COLUMN layer_order; DROP TABLE pet_artworks; ALTER TABLE personas DROP COLUMN pet_image_enabled; ALTER TABLE personas DROP COLUMN pet_image_revision; ALTER TABLE accessories DROP COLUMN position_x; ALTER TABLE accessories DROP COLUMN position_y; ALTER TABLE accessories DROP COLUMN scale;").unwrap();}
    fn remove_v10_fixture_columns(connection: &Connection) {
        remove_v12_fixture_columns(connection);
        connection.execute_batch("ALTER TABLE sessions DROP COLUMN is_pinned;").unwrap();
        connection.execute_batch("ALTER TABLE personas DROP COLUMN chat_bubble_style; ALTER TABLE personas DROP COLUMN chat_user_text_color; ALTER TABLE personas DROP COLUMN chat_assistant_text_color;").unwrap();
    }
    fn remove_v7_fixture_columns(connection:&Connection) {
        remove_v10_fixture_columns(connection);
        connection.execute_batch("DROP TABLE reminder_email_items; DROP TABLE reminder_email_deliveries;").unwrap();
        connection.execute_batch("DROP VIEW provider_effective; DROP TABLE provider_roles; DROP TABLE provider_models; DROP TABLE message_images;
            ALTER TABLE providers DROP COLUMN created_at; ALTER TABLE providers DROP COLUMN updated_at;
            ALTER TABLE providers ADD COLUMN model TEXT NOT NULL DEFAULT 'legacy-model';
            ALTER TABLE providers ADD COLUMN is_active INTEGER NOT NULL DEFAULT 0 CHECK(is_active IN (0,1));
            ALTER TABLE providers ADD COLUMN memory_is_active INTEGER NOT NULL DEFAULT 0 CHECK(memory_is_active IN (0,1));
            ALTER TABLE providers ADD COLUMN max_output_tokens INTEGER NOT NULL DEFAULT 32768 CHECK(max_output_tokens>0);
            ALTER TABLE providers ADD COLUMN context_window_override INTEGER CHECK(context_window_override>0);
            CREATE UNIQUE INDEX idx_providers_one_active ON providers(is_active) WHERE is_active=1;
            CREATE UNIQUE INDEX idx_providers_one_memory_active ON providers(memory_is_active) WHERE memory_is_active=1;
            ALTER TABLE personas DROP COLUMN created_at; ALTER TABLE personas DROP COLUMN updated_at;").unwrap();
    }
    #[test]
    fn session_pinning_v10_upgrade_preserves_sessions_and_matches_fresh_schema() {
        let mut c=memory();
        remove_v12_fixture_columns(&c);
        c.execute_batch("ALTER TABLE sessions DROP COLUMN is_pinned; PRAGMA user_version=10; INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(1,'原会话','2026-10-01','2026-10-02');").unwrap();
        initialize(&mut c).unwrap();
        assert_current_schema(&c);
        let saved=crate::sessions::get(&c,1,1).unwrap();
        assert_eq!((saved.title.as_str(),saved.updated_at.as_str(),saved.is_pinned),("原会话","2026-10-02",false));
        assert!(c.execute("UPDATE sessions SET is_pinned=2",[]).is_err());
        crate::sessions::set_pinned(&mut c,1,1,true).unwrap();
        initialize(&mut c).unwrap();
        assert!(crate::sessions::get(&c,1,1).unwrap().is_pinned);
    }

    #[test]
    fn v9_chat_appearance_upgrade_preserves_persona_and_matches_fresh_schema() {
        let mut c=memory();
        remove_v10_fixture_columns(&c);
        c.execute_batch("PRAGMA user_version=9; UPDATE personas SET prompt='保留人设',chat_bg_opacity=63;").unwrap();
        initialize(&mut c).unwrap();
        assert_current_schema(&c);
        let persona=crate::persona::list(&c).unwrap().remove(0);
        assert_eq!(persona.prompt,"保留人设"); assert_eq!(persona.chat_bg_opacity,63);
        assert_eq!(persona.chat_appearance,crate::persona::background::Appearance::default());
        initialize(&mut c).unwrap();
        assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
    }
    #[test]
    fn v7_provider_models_migrate_without_losing_inactive_connection_budget() {
        let mut c=memory();
        remove_v10_fixture_columns(&c);
        c.execute_batch("DROP TABLE reminder_email_items; DROP TABLE reminder_email_deliveries; DROP VIEW provider_effective; DROP TABLE provider_roles; DROP TABLE provider_models;
            CREATE TABLE provider_roles(purpose TEXT PRIMARY KEY CHECK(purpose IN ('chat','memory','image','vision')),provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE RESTRICT,model TEXT NOT NULL CHECK(length(trim(model))>0),enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)));
            ALTER TABLE providers ADD COLUMN model TEXT NOT NULL DEFAULT 'old';
            ALTER TABLE providers ADD COLUMN is_active INTEGER NOT NULL DEFAULT 0 CHECK(is_active IN (0,1));
            ALTER TABLE providers ADD COLUMN memory_is_active INTEGER NOT NULL DEFAULT 0 CHECK(memory_is_active IN (0,1));
            ALTER TABLE providers ADD COLUMN max_output_tokens INTEGER NOT NULL DEFAULT 32768 CHECK(max_output_tokens>0);
            ALTER TABLE providers ADD COLUMN context_window_override INTEGER CHECK(context_window_override>0);
            CREATE UNIQUE INDEX idx_providers_one_active ON providers(is_active) WHERE is_active=1;
            CREATE UNIQUE INDEX idx_providers_one_memory_active ON providers(memory_is_active) WHERE memory_is_active=1;
            INSERT INTO providers(name,model,is_active,max_output_tokens,context_window_override) VALUES('甲','a',1,2048,16000);
            INSERT INTO providers(name,model,max_output_tokens,context_window_override) VALUES('乙','b',4096,32000);
            INSERT INTO provider_roles(purpose,provider_id,model) VALUES('chat',1,'a-current');
            PRAGMA user_version=7;").unwrap();
        initialize(&mut c).unwrap();
        let all=crate::providers::list(&c).unwrap();
        assert_eq!((all[0].model.as_str(),all[0].max_output_tokens,all[0].context_window_override),("a-current",2048,Some(16000)));
        assert_eq!((all[1].model.as_str(),all[1].max_output_tokens,all[1].context_window_override),("b",4096,Some(32000)));
        assert_eq!(c.query_row("SELECT count(*) FROM pragma_table_info('providers') WHERE name IN ('model','is_active','memory_is_active','max_output_tokens','context_window_override')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        let revision=crate::provider_roles::revision(&c).unwrap();
        crate::provider_roles::save(&mut c,"chat",Some("2"),"b",true,revision,Some(4096),Some(32000)).unwrap();
        assert_eq!(crate::providers::list(&c).unwrap()[0].max_output_tokens,2048);
        assert_eq!(crate::providers::list(&c).unwrap()[1].max_output_tokens,4096);
    }
    #[test]
    fn settings_redesign_v6_upgrade_preserves_provider_key_and_initial_persona() {
        let mut connection=memory();remove_v7_fixture_columns(&connection);
        connection.execute_batch("PRAGMA user_version=6; INSERT INTO providers(name,model,api_key_encrypted,is_active) VALUES('旧连接','旧模型','原密文',1); UPDATE personas SET prompt='当前正文',initial_prompt='初始正文';").unwrap();
        initialize(&mut connection).unwrap();assert_current_schema(&connection);
        let values:(String,String)=connection.query_row("SELECT prompt,initial_prompt FROM personas WHERE is_builtin=1",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();assert_eq!(values,("当前正文".into(),"初始正文".into()));
        let key:String=connection.query_row("SELECT api_key_encrypted FROM providers",[],|r|r.get(0)).unwrap();assert_eq!(key,"原密文");
        assert_eq!(crate::provider_roles::role(&connection,"chat").unwrap().unwrap().model,"旧模型");
        let time:Option<String>=connection.query_row("SELECT created_at FROM providers",[],|r|r.get(0)).unwrap();assert!(time.is_none());
    }
    fn version_three_fixture() -> Connection {
        let connection = memory();
        remove_v7_fixture_columns(&connection);
        connection
            .execute_batch(
                "ALTER TABLE accessories DROP COLUMN image_file;
             ALTER TABLE session_pet_states DROP COLUMN proactive_next_check_idle_ms;
             ALTER TABLE session_pet_states DROP COLUMN last_proactive_sent_at;
             DROP TRIGGER chat_rounds_stop_reason_insert;
             DROP TRIGGER chat_rounds_stop_reason_update;
             ALTER TABLE chat_rounds DROP COLUMN stop_reason;
             ALTER TABLE providers DROP COLUMN context_window_override;
             ALTER TABLE providers DROP COLUMN max_output_tokens;
             PRAGMA user_version=3;",
            )
            .unwrap();
        connection
    }

    fn version_one_fixture() -> Connection {
        let connection = version_three_fixture();
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

    fn assert_current_schema(connection: &Connection) {
        let expected = memory();
        let normalized = |connection: &Connection| {
            objects(connection)
                .into_iter()
                .map(|(name, sql)| {
                    let sql = sql
                        .lines()
                        .map(|line| line.split("--").next().unwrap_or(""))
                        .collect::<String>();
                    let compact = sql
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>();
                    (name, compact.replace("IFNOTEXISTS", "").replace('"', ""))
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(normalized(connection), normalized(&expected));
    }

    #[test]
    fn version_four_migration_preserves_idle_and_success_across_restart() {
        let mut connection = memory();
        remove_v7_fixture_columns(&connection);
        connection.execute_batch("ALTER TABLE accessories DROP COLUMN image_file;
            ALTER TABLE session_pet_states DROP COLUMN proactive_next_check_idle_ms;
            ALTER TABLE session_pet_states DROP COLUMN last_proactive_sent_at;
            PRAGMA user_version=4;
            INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'保留会话','a','a');
            UPDATE session_pet_states SET idle_branch='bored',idle_elapsed_ms=3000000 WHERE session_id=1;").unwrap();
        initialize(&mut connection).unwrap();
        assert_current_schema(&connection);
        let scheduled: (i64, Option<i64>, Option<String>) = connection.query_row(
            "SELECT idle_elapsed_ms,proactive_next_check_idle_ms,last_proactive_sent_at FROM session_pet_states WHERE session_id=1",
            [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(scheduled, (3000000, Some(3300000), None));
        connection.execute("UPDATE session_pet_states SET proactive_next_check_idle_ms=NULL,last_proactive_sent_at='2026-10-06T10:00:00Z' WHERE session_id=1", []).unwrap();
        initialize(&mut connection).unwrap();
        let saved: (Option<i64>, String) = connection.query_row(
            "SELECT proactive_next_check_idle_ms,last_proactive_sent_at FROM session_pet_states WHERE session_id=1",
            [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(saved, (None, "2026-10-06T10:00:00Z".into()));
        assert_eq!(connection.pragma_query_value(None, "user_version", |r| r.get::<_,i64>(0)).unwrap(), VERSION);
    }

    #[test]
    fn version_three_migration_preserves_chat_data_and_rolls_back_on_failure() {
        for fail in [true, false] {
            let mut connection = version_three_fixture();
            connection.execute_batch(
                "INSERT INTO providers(name,model,api_key_encrypted,is_active,memory_is_active)
                 VALUES('保留提供商','model','opaque-secret',1,1);
                 INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'保留会话','a','a');
                 INSERT INTO chat_rounds(id,session_id,status,created_at,completed_at) VALUES(1,1,'interrupted','a','b');
                 INSERT INTO messages(id,session_id,round_id,role,content,created_at) VALUES(1,1,1,'user','问题','a');
                 INSERT INTO messages(id,session_id,round_id,role,content,created_at) VALUES(2,1,1,'assistant','已保存正文 ','b');
                 INSERT INTO notebooks(session_id,content,updated_at) VALUES(1,'保留记事本','b');",
            ).unwrap();
            if fail {
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
                        .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                        .unwrap(),
                    3
                );
                assert_eq!(
                    connection
                        .query_row("SELECT status FROM chat_rounds WHERE id=1", [], |r| r
                            .get::<_, String>(0))
                        .unwrap(),
                    "interrupted"
                );
            } else {
                initialize(&mut connection).unwrap();
                assert_current_schema(&connection);
                assert_eq!(
                    connection
                        .query_row("SELECT max_output_tokens FROM provider_effective", [], |r| r
                            .get::<_, i64>(0))
                        .unwrap(),
                    32768
                );
                assert_eq!(
                    connection
                        .query_row("SELECT context_window_override FROM provider_effective", [], |r| {
                            r.get::<_, Option<i64>>(0)
                        })
                        .unwrap(),
                    None
                );
                assert_eq!(
                    connection
                        .query_row("SELECT stop_reason FROM chat_rounds", [], |r| r
                            .get::<_, String>(0))
                        .unwrap(),
                    "unknown"
                );
                assert_eq!(
                    connection
                        .query_row("SELECT api_key_encrypted FROM providers", [], |r| r
                            .get::<_, String>(0))
                        .unwrap(),
                    "opaque-secret"
                );
                assert_eq!(
                    connection
                        .query_row(
                            "SELECT is_active+memory_is_active FROM provider_effective",
                            [],
                            |r| r.get::<_, i64>(0)
                        )
                        .unwrap(),
                    2
                );
                assert_eq!(
                    connection
                        .query_row("SELECT content FROM notebooks", [], |r| r
                            .get::<_, String>(0))
                        .unwrap(),
                    "保留记事本"
                );
                initialize(&mut connection).unwrap();
            }
            assert_eq!(
                connection
                    .query_row("SELECT content FROM messages WHERE id=2", [], |r| r
                        .get::<_, String>(0))
                    .unwrap(),
                "已保存正文 "
            );
            assert_eq!(
                connection
                    .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                    .unwrap(),
                "ok"
            );
            assert_eq!(
                connection
                    .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                        .get::<_, i64>(
                        0
                    ))
                    .unwrap(),
                0
            );
        }
    }

    #[test]
    fn version_two_caps_intimacy_preserves_state_and_rolls_back_on_failure() {
        for fail in [true, false] {
            let mut connection = version_three_fixture();
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
                assert_current_schema(&connection);
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
        assert_current_schema(&connection);
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
    fn version_five_accessory_migration_preserves_data_and_rolls_back_failures() {
        for fail in [false,true] {
            let mut connection = memory();
            remove_v7_fixture_columns(&connection);
            connection.execute_batch("ALTER TABLE accessories DROP COLUMN image_file; PRAGMA user_version=5;
                INSERT INTO sessions(id,persona_id,title,craft_tier_1_used,created_at,updated_at) VALUES(1,1,'保留',1,'a','a');
                INSERT INTO accessories(id,session_id,name,display_text,description,cost,source,is_worn,created_at) VALUES(1,1,'星星','★','简介',20,'crafted',1,'a');
                UPDATE user_state SET coins=37;").unwrap();
            if fail { connection.execute("UPDATE settings SET value='bad' WHERE key='hungry_minutes'",[]).unwrap(); }
            let before = objects(&connection);
            let result = initialize(&mut connection);
            if fail {
                assert!(result.is_err());
                assert_eq!(objects(&connection),before);
                assert_eq!(connection.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),5);
            } else {
                result.unwrap();
                assert_current_schema(&connection);
                let item = crate::accessories::get(&connection,1,1).unwrap();
                assert!(item.image_file.is_none());assert!(item.is_worn);
                assert_eq!(item.display_text,"★");assert_eq!(item.cost,"20");
                assert_eq!(crate::currency::get_balance(&connection).unwrap(),37);
                assert_eq!(connection.query_row("SELECT craft_tier_1_used FROM sessions WHERE id=1",[],|r|r.get::<_,i64>(0)).unwrap(),1);
                initialize(&mut connection).unwrap();
                assert_eq!(crate::accessories::get(&connection,1,1).unwrap(),item);
            }
        }
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
        let paths = AppPaths::portable(root.clone());
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
        let paths = AppPaths::portable(root.clone());
        {
            let (c, _) = open(&paths).unwrap();
            c.execute("UPDATE user_state SET coins=17", []).unwrap();
        }
        {
            let (c, _) = open(&paths).unwrap();
            assert_eq!(
                c.query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                17
            );
        }
        let blocker = root.join("not-a-directory");
        std::fs::write(&blocker, b"keep").unwrap();
        assert!(open(&AppPaths::portable(blocker.clone()))
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
        test_database()
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

/// 启动和逻辑恢复复用同一内置Embedding补齐规则。
pub(crate) fn ensure_builtin_embedding(connection:&Connection)->AppResult<()> {
    connection.execute("INSERT INTO embedding_providers(name,kind,model,is_builtin) SELECT '内置中文 Embedding','local','BAAI/bge-small-zh-v1.5',1 WHERE NOT EXISTS(SELECT 1 FROM embedding_providers WHERE is_builtin=1)",[])?;
    connection.execute("UPDATE embedding_providers SET is_active=1 WHERE is_builtin=1 AND NOT EXISTS(SELECT 1 FROM embedding_providers WHERE is_active=1)",[])?;
    Ok(())
}

#[cfg(test)]
mod reminder_email_migration_tests {
    use super::*;
    #[test]
    fn reminder_email_upgrade_from_v8_preserves_existing_data() {
        let mut c=test_database();
        c.execute_batch("ALTER TABLE accessories DROP COLUMN render_layer; ALTER TABLE accessories DROP COLUMN layer_order; DROP TABLE pet_artworks; ALTER TABLE personas DROP COLUMN pet_image_enabled; ALTER TABLE personas DROP COLUMN pet_image_revision; ALTER TABLE accessories DROP COLUMN position_x; ALTER TABLE accessories DROP COLUMN position_y; ALTER TABLE accessories DROP COLUMN scale; ALTER TABLE sessions DROP COLUMN is_pinned; ALTER TABLE personas DROP COLUMN chat_bubble_style; ALTER TABLE personas DROP COLUMN chat_user_text_color; ALTER TABLE personas DROP COLUMN chat_assistant_text_color;").unwrap();
        c.execute_batch("DROP TABLE reminder_email_items; DROP TABLE reminder_email_deliveries; PRAGMA user_version=8;").unwrap();
        c.execute("INSERT INTO reminders(name,kind,content,status,fire_at,source,created_at) VALUES('旧提醒','once','保留','pending','2099-01-01T00:00:00Z','manual','2026-10-15T00:00:00Z')",[]).unwrap();
        initialize(&mut c).unwrap();
        assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
        assert_eq!(c.query_row("SELECT content FROM reminders WHERE name='旧提醒'",[],|r|r.get::<_,String>(0)).unwrap(),"保留");
        assert_eq!(c.query_row("SELECT count(*) FROM reminder_email_deliveries",[],|r|r.get::<_,i64>(0)).unwrap(),0);
        initialize(&mut c).unwrap();
        assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
    }
}

#[cfg(test)]mod pet_layers_migration_tests{
 use super::*;
 #[test]fn v19_upgrade_adds_error_preserving_rows_indexes_and_sequence(){
 let mut c=Connection::open_in_memory().unwrap();
 c.execute_batch(&SCHEMA.replace("'lifted','error'","'lifted'")).unwrap();
 c.execute_batch("PRAGMA user_version=19;INSERT INTO personas(id,name,prompt,initial_prompt,is_builtin) VALUES(1,'电子小猫','原设定','原设定',1);INSERT INTO pet_artworks(id,persona_id,kind,name,prompt,description,category,enabled,image_file,preview_file,revision,created_at,updated_at) VALUES(41,1,'expression','旧表情','提示','说明','chat',0,'old.png','preview.png',7,'a','b');INSERT INTO pet_artworks(id,persona_id,kind,name,category,created_at,updated_at) VALUES(999,1,'expression','删除项','chat','a','a');DELETE FROM pet_artworks WHERE id=999;").unwrap();
 initialize(&mut c).unwrap();
 assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
 let row:(String,String,String,i64,String,String,i64)=c.query_row("SELECT name,prompt,description,enabled,image_file,preview_file,revision FROM pet_artworks WHERE id=41",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).unwrap();
 assert_eq!(row,("旧表情".into(),"提示".into(),"说明".into(),0,"old.png".into(),"preview.png".into(),7));
 c.execute_batch("INSERT INTO pet_artworks(persona_id,kind,name,category,created_at,updated_at) VALUES(1,'expression','超载','error','a','a')").unwrap();assert!(c.last_insert_rowid()>999);
 assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE type='index' AND name IN('idx_pet_artworks_base','idx_pet_artworks_persona')",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
 initialize(&mut c).unwrap();assert_eq!(c.query_row("SELECT count(*) FROM pet_artworks",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
 #[test]fn v18_upgrade_preserves_accessories_and_adds_multiple_wear_and_lifted(){
 let mut c=test_database();
 c.execute("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(1,1,'迁移测试','a','a')",[]).unwrap();
 c.execute_batch("INSERT INTO accessories(id,session_id,name,display_text,cost,description,source,is_worn,created_at,position_x,position_y,scale) VALUES(101,1,'原挂件','✦',20,'保留','crafted',1,'a',125,-500,1500);ALTER TABLE accessories DROP COLUMN render_layer;ALTER TABLE accessories DROP COLUMN layer_order;CREATE UNIQUE INDEX idx_accessories_one_worn ON accessories(session_id) WHERE is_worn=1;PRAGMA user_version=18;").unwrap();
 initialize(&mut c).unwrap();assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
 let row:(String,i64,Option<i32>,Option<i32>,i32,String,i64)=c.query_row("SELECT name,is_worn,position_x,position_y,scale,render_layer,layer_order FROM accessories WHERE id=101",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).unwrap();assert_eq!(row,("原挂件".into(),1,Some(125),Some(-500),1500,"front".into(),0));
 c.execute_batch("INSERT INTO accessories(session_id,name,display_text,description,cost,source,is_worn,created_at) VALUES(1,'第二件','✦','多件',20,'crafted',1,'a');INSERT INTO pet_artworks(persona_id,kind,name,category,created_at,updated_at) VALUES(1,'expression','拎后颈','lifted','a','a');").unwrap();assert_eq!(c.query_row("SELECT count(*) FROM accessories WHERE is_worn=1",[],|r|r.get::<_,i64>(0)).unwrap(),2);
 }
}

#[cfg(test)]
mod wait_pet_migration_tests {
    use super::*;
    #[test]
    fn v22_wait_pet_upgrade_preserves_custom_artwork_and_deleted_id_highwater() {
        let mut c=test_database();c.execute_batch(PET_TOSS_MIGRATION).unwrap();
        c.execute_batch("INSERT INTO pet_artworks(id,persona_id,kind,name,category,enabled,image_file,preview_file,revision,created_at,updated_at) VALUES(37,1,'expression','自己的下落','toss_down',0,'assets/accessories/down.png','assets/accessories/preview.png',8,'a','b'),(101,1,'expression','删除项','normal',1,NULL,NULL,0,'a','b');DELETE FROM pet_artworks WHERE id=101;PRAGMA user_version=22;").unwrap();
        initialize(&mut c).unwrap();
        let row:(String,i64,String,String,i64)=c.query_row("SELECT category,enabled,image_file,preview_file,revision FROM pet_artworks WHERE id=37",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap();
        assert_eq!(row,("toss_down".into(),0,"assets/accessories/down.png".into(),"assets/accessories/preview.png".into(),8));
        c.execute("INSERT INTO pet_artworks(persona_id,kind,name,category,created_at,updated_at) VALUES(1,'expression','等摸摸','wait_pet','a','b')",[]).unwrap();assert!(c.last_insert_rowid()>101);
        assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
        initialize(&mut c).unwrap();assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
    }
}

#[cfg(test)]
mod souvenir_title_migration_tests {
    use super::*;
    #[test]
    fn v23_souvenir_title_upgrade_keeps_legacy_content_images_and_sequence() {
        let mut c=test_database();c.execute_batch("INSERT INTO sessions(id,persona_id,title,created_at,updated_at) VALUES(81,1,'旧会话','a','a');INSERT INTO souvenirs(id,session_id,content,note,is_favorite,created_at,image_file) VALUES(17,81,'原礼物','原备注',1,'a','assets/accessories/old.png'),(99,81,'删掉的','备注',0,'a',NULL);DELETE FROM souvenirs WHERE id=99;ALTER TABLE souvenirs DROP COLUMN title;PRAGMA user_version=23;").unwrap();
        initialize(&mut c).unwrap();let old=crate::souvenirs::get(&c,81,17).unwrap();assert!(old.title.is_none());assert_eq!(old.content,"原礼物");assert_eq!(old.note,"原备注");assert!(old.is_favorite);assert_eq!(old.image_file.as_deref(),Some("assets/accessories/old.png"));
        c.execute("INSERT INTO souvenirs(session_id,title,content,note,created_at) VALUES(81,'贝壳','一枚小贝壳','给你','b')",[]).unwrap();assert!(c.last_insert_rowid()>99);
        assert!(c.execute("UPDATE souvenirs SET title=''",[]).is_err());initialize(&mut c).unwrap();assert_eq!(c.pragma_query_value(None,"user_version",|r|r.get::<_,i64>(0)).unwrap(),VERSION);
        assert!(c.prepare("PRAGMA foreign_key_check").unwrap().query([]).unwrap().next().unwrap().is_none());
    }
}
