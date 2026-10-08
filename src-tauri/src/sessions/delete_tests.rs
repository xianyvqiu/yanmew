use super::*;
use rusqlite::types::Value;

const AT: &str = "2026-10-06T10:00:00.000Z";
const SESSION_TABLES: &[&str] = &[
    "session_pet_states", "chat_rounds", "messages", "short_memories",
    "long_memories", "long_memory_embeddings", "notebooks", "travels",
    "travel_journals", "letters", "souvenirs", "accessories", "session_festivals",
];
const GLOBAL_TABLES: &[&str] = &[
    "personas", "user_profiles", "providers", "embedding_providers", "user_state",
    "currency_ledger", "game_plugins", "game_permissions", "game_plugin_settings",
    "game_plugin_data", "game_runs", "reminders", "reminder_occurrences", "festival_catalog",
];

fn database() -> Connection {
    let connection = crate::db::test_database();
    connection
}

fn rows(connection: &Connection, table: &str, filter: &str) -> Vec<Vec<Value>> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM {table} {filter} ORDER BY rowid"))
        .unwrap();
    let columns = statement.column_count();
    statement
        .query_map([], |row| (0..columns).map(|column| row.get(column)).collect())
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}

fn count(connection: &Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

fn current(connection: &Connection, id: i64) {
    connection.execute(
        "INSERT INTO settings(key,value) VALUES('current_session_id',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [id.to_string()],
    ).unwrap();
}

fn session(connection: &Connection, persona_id: i64, updated_at: &str) -> i64 {
    connection.execute(
        "INSERT INTO sessions(persona_id,title,created_at,updated_at) VALUES(?1,'新会话',?2,?3)",
        params![persona_id, AT, updated_at],
    ).unwrap();
    connection.last_insert_rowid()
}

fn global_data(connection: &Connection) {
    connection.execute_batch(
        "UPDATE user_state SET coins=77;
         INSERT INTO user_profiles(id,name,content,created_at,updated_at)
         VALUES(1,'共同用户','用户设定','2026-10-06T10:00:00.000Z','2026-10-06T10:00:00.000Z');
         UPDATE settings SET value='dark' WHERE key='theme_mode';
         INSERT INTO game_plugins(plugin_id,name,author,installed_version,api_version,
           source_type,package_sha256,package_status,is_enabled,installed_at,updated_at)
         VALUES('test.plugin','测试插件','作者','1.0.0','1','local','sha256',
           'missing_package',0,'2026-10-06T10:00:00.000Z','2026-10-06T10:00:00.000Z');
         INSERT INTO game_permissions(plugin_id,permission,decision,plugin_version,decided_at)
         VALUES('test.plugin','wallet.modify','allowed','1.0.0','2026-10-06T10:00:00.000Z');
         INSERT INTO game_plugin_settings(plugin_id,setting_key,value_json,updated_at)
         VALUES('test.plugin','audio.enabled','true','2026-10-06T10:00:00.000Z');
         INSERT INTO game_plugin_data(plugin_id,data_key,value_json,storage_version,updated_at)
         VALUES('test.plugin','progress.main','{\"level\":2}',1,'2026-10-06T10:00:00.000Z');
         INSERT INTO game_runs(id,plugin_id,plugin_name,plugin_version,wallet_permission_granted,
           status,outcome,coin_delta,started_at,ended_at)
         VALUES(1,'test.plugin','测试插件','1.0.0',1,'completed','win',5,
           '2026-10-06T10:00:00.000Z','2026-10-06T10:01:00.000Z');
         INSERT INTO currency_ledger(delta,reason,game_run_id,plugin_id,plugin_version,
           plugin_display_reason,wallet_request_id,created_at)
         VALUES(5,'plugin_game',1,'test.plugin','1.0.0','胜利','wallet:1','2026-10-06T10:01:00.000Z');
         INSERT INTO currency_ledger(delta,reason,ref_id,created_at)
         VALUES(-20,'accessory_craft',999,'2026-10-06T10:00:00.000Z');
         INSERT INTO reminders(id,name,kind,content,status,fire_at,source,created_at)
         VALUES(1,'全局提醒','once','喝水','pending','2026-10-07T10:00:00.000Z',
           'manual','2026-10-06T10:00:00.000Z');
         INSERT INTO reminder_occurrences(reminder_id,scheduled_at,delivery_state,created_at)
         VALUES(1,'2026-10-06T09:00:00.000Z','queued','2026-10-06T09:00:00.000Z');
         INSERT INTO festival_catalog(id,code,name,year,calendar_type,solar_month,solar_day,blessing)
         VALUES(1,'test_festival','共同节日',2026,'solar',10,6,'节日快乐');"
    ).unwrap();
}

fn session_tree(connection: &Connection, session_id: i64, title: &str) {
    connection.execute("UPDATE sessions SET user_profile_id=1 WHERE id=?1", [session_id]).unwrap();
    let mut round_ids = Vec::new();
    let mut short_ids = Vec::new();
    for index in 0..2 {
        connection.execute(
            "INSERT INTO chat_rounds(session_id,status,created_at,completed_at) VALUES(?1,'completed',?2,?2)",
            params![session_id, AT],
        ).unwrap();
        let round_id = connection.last_insert_rowid();
        round_ids.push(round_id);
        for role in ["user", "assistant"] {
            connection.execute(
                "INSERT INTO messages(session_id,round_id,role,content,created_at) VALUES(?1,?2,?3,'聊天原文',?4)",
                params![session_id, round_id, role, AT],
            ).unwrap();
        }
        connection.execute(
            "INSERT INTO short_memories(session_id,round_id,content,integration_state,created_at,updated_at)
             VALUES(?1,?2,'短期事实',?3,?4,?4)",
            params![session_id, round_id, if index == 0 { "integrated" } else { "pending" }, AT],
        ).unwrap();
        short_ids.push(connection.last_insert_rowid());
    }
    connection.execute(
        "INSERT INTO long_memories(session_id,title,content,source_count,activated_by_round_id,created_at,updated_at)
         VALUES(?1,?2,'长期事实',1,?3,?4,?4)",
        params![session_id, title, round_ids[1], AT],
    ).unwrap();
    let memory_id = connection.last_insert_rowid();
    connection.execute(
        "INSERT INTO long_memory_sources(long_memory_id,short_memory_id,source_order) VALUES(?1,?2,1)",
        params![memory_id, short_ids[0]],
    ).unwrap();
    connection.execute(
        "INSERT INTO long_memory_embeddings(long_memory_id,session_id,model_key,dimensions,vector,content_hash,updated_at)
         VALUES(?1,?2,'test:1',1,?3,'hash',?4)",
        params![memory_id, session_id, 1.0_f32.to_le_bytes().to_vec(), AT],
    ).unwrap();
    connection.execute(
        "INSERT INTO notebooks(session_id,content,updated_at) VALUES(?1,'完整记事本',?2)",
        params![session_id, AT],
    ).unwrap();
    for kind in ["text", "accessory"] {
        let snapshot = if kind == "text" {
            "{\"schema_version\":1,\"kind\":\"text\",\"content\":\"礼物\",\"note\":\"旅行礼物\"}"
        } else {
            "{\"schema_version\":1,\"kind\":\"accessory\",\"name\":\"星星\",\"display_text\":\"★\",\"description\":\"旅行饰品\"}"
        };
        connection.execute(
            "INSERT INTO travels(session_id,status,started_at,planned_return_at,actual_return_at,
               midway_letter_at,gift_kind,facts_state,destination,activities_json,facts_version,
               facts_ready_at,gift_snapshot_json,gift_state,journal_state,created_at)
             VALUES(?1,'returned',?2,?2,?2,?2,?3,'ready','公园',
               '[{\"elapsed_minutes\":0,\"description\":\"散步\"}]',1,?2,?4,'done','done',?2)",
            params![session_id, AT, kind, snapshot],
        ).unwrap();
        let travel_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO travel_journals(session_id,travel_id,title,content,created_at,updated_at)
             VALUES(?1,?2,'公园旅行','旅行记录',?3,?3)",
            params![session_id, travel_id, AT],
        ).unwrap();
        if kind == "text" {
            connection.execute(
                "INSERT INTO souvenirs(session_id,travel_id,content,note,created_at) VALUES(?1,?2,'礼物','旅行礼物',?3)",
                params![session_id, travel_id, AT],
            ).unwrap();
            connection.execute(
                "INSERT INTO letters(session_id,travel_id,sender,content,reply_due_at,reply_state,
                   letter_kind,has_been_replied,created_at)
                 VALUES(?1,?2,'user','用户信',?3,'completed','user',1,?3)",
                params![session_id, travel_id, AT],
            ).unwrap();
            let letter_id = connection.last_insert_rowid();
            connection.execute(
                "INSERT INTO letters(session_id,travel_id,sender,content,reply_to_id,letter_kind,created_at)
                 VALUES(?1,?2,'ai','回复信',?3,'reply',?4)",
                params![session_id, travel_id, letter_id, AT],
            ).unwrap();
        } else {
            connection.execute(
                "INSERT INTO accessories(session_id,travel_id,name,display_text,description,source,created_at)
                 VALUES(?1,?2,'星星','★','旅行饰品','travel',?3)",
                params![session_id, travel_id, AT],
            ).unwrap();
        }
    }
    connection.execute(
        "INSERT INTO session_festivals(session_id,festival_id,kind,name,year,calendar_type,
           solar_month,solar_day,blessing,source,created_at,updated_at)
         VALUES(?1,1,'festival','共同节日',2026,'solar',10,6,'节日快乐','user',?2,?2)",
        params![session_id, AT],
    ).unwrap();
}

fn fts_count(connection: &Connection, term: &str) -> i64 {
    connection.query_row(
        "SELECT COUNT(*) FROM long_memories_fts WHERE long_memories_fts MATCH ?1",
        [term], |row| row.get(0),
    ).unwrap()
}

#[test]
fn delete_full_session_tree_keeps_other_sessions_and_global_records() {
    let mut connection = database();
    global_data(&connection);
    let target = session(&connection, 1, AT);
    let survivor = session(&connection, 1, AT);
    session_tree(&connection, target, "目标猫猫日常");
    session_tree(&connection, survivor, "保留猫猫日常");
    current(&connection, target);
    assert_eq!(fts_count(&connection, "目标猫猫"), 1);
    assert_eq!(fts_count(&connection, "保留猫猫"), 1);
    let globals: Vec<_> = GLOBAL_TABLES.iter().map(|table| rows(&connection, table, "")).collect();
    let settings = rows(&connection, "settings", "WHERE key <> 'current_session_id'");
    let survivor_row = rows(&connection, "sessions", &format!("WHERE id={survivor}"));
    let survivor_tree: Vec<_> = SESSION_TABLES.iter().map(|table| {
        rows(&connection, table, &format!("WHERE session_id={survivor}"))
    }).collect();
    let survivor_sources = rows(&connection, "long_memory_sources",
        &format!("WHERE long_memory_id IN (SELECT id FROM long_memories WHERE session_id={survivor})"));

    assert_eq!(delete(&mut connection, 1, target).unwrap(), Some(survivor.to_string()));
    assert!(get(&connection, 1, target).is_err());
    for (index, table) in SESSION_TABLES.iter().enumerate() {
        assert!(rows(&connection, table, &format!("WHERE session_id={target}")).is_empty(), "{table}");
        assert_eq!(rows(&connection, table, &format!("WHERE session_id={survivor}")), survivor_tree[index], "{table}");
    }
    assert_eq!(rows(&connection, "sessions", &format!("WHERE id={survivor}")), survivor_row);
    assert_eq!(rows(&connection, "long_memory_sources", ""), survivor_sources);
    for (index, table) in GLOBAL_TABLES.iter().enumerate() {
        assert_eq!(rows(&connection, table, ""), globals[index], "{table}");
    }
    assert_eq!(rows(&connection, "settings", "WHERE key <> 'current_session_id'"), settings);
    assert_eq!(fts_count(&connection, "目标猫猫"), 0);
    assert_eq!(fts_count(&connection, "保留猫猫"), 1);
    assert_eq!(count(&connection, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 0);
}

#[test]
fn delete_selects_latest_same_persona_and_leaves_last_session_empty() {
    let mut connection = database();
    connection.execute(
        "INSERT INTO personas(id,name,prompt,initial_prompt) VALUES(2,'另一人设','设定','设定')", [],
    ).unwrap();
    let selected = session(&connection, 1, "2026-10-06T10:00:00.000Z");
    let non_current = session(&connection, 1, "2026-10-06T09:00:00.000Z");
    let older_id = session(&connection, 1, "2026-10-06T11:00:00.000Z");
    let newer_id = session(&connection, 1, "2026-10-06T11:00:00.000Z");
    let other_persona = session(&connection, 2, "2026-10-06T12:00:00.000Z");
    current(&connection, selected);
    assert_eq!(delete(&mut connection, 1, non_current).unwrap(), Some(selected.to_string()));
    assert!(get(&connection, 1, selected).unwrap().is_current);
    assert_eq!(delete(&mut connection, 1, selected).unwrap(), Some(newer_id.to_string()));
    assert!(get(&connection, 1, newer_id).unwrap().is_current);
    assert_eq!(delete(&mut connection, 1, newer_id).unwrap(), Some(older_id.to_string()));
    assert_eq!(delete(&mut connection, 1, older_id).unwrap(), None);
    assert!(list(&connection, 1).unwrap().is_empty());
    assert_eq!(count(&connection, "SELECT COUNT(*) FROM settings WHERE key='current_session_id'"), 0);
    assert_eq!(count(&connection, "SELECT COUNT(*) FROM sessions"), 1);
    assert!(!get(&connection, 2, other_persona).unwrap().is_current);
}

#[test]
fn wrong_owner_and_failed_delete_preserve_memory_indexes_and_current_selection() {
    let mut connection = database();
    global_data(&connection);
    let target = session(&connection, 1, AT);
    let survivor = session(&connection, 1, AT);
    session_tree(&connection, target, "目标猫猫日常");
    current(&connection, target);
    let mut tables = vec!["sessions", "long_memory_sources", "settings"];
    tables.extend_from_slice(SESSION_TABLES);
    tables.extend_from_slice(GLOBAL_TABLES);
    let before: Vec<_> = tables.iter().map(|table| rows(&connection, table, "")).collect();
    assert_eq!(delete(&mut connection, 2, target).unwrap_err().code, "CONTEXT_CHANGED");
    for (index, table) in tables.iter().enumerate() {
        assert_eq!(rows(&connection, table, ""), before[index], "{table}");
    }
    connection.execute_batch(&format!(
        "CREATE TRIGGER fail_session_delete BEFORE DELETE ON sessions
         WHEN OLD.id={target} BEGIN SELECT RAISE(ABORT,'forced session delete failure'); END;"
    )).unwrap();
    assert!(delete(&mut connection, 1, target).is_err());
    for (index, table) in tables.iter().enumerate() {
        assert_eq!(rows(&connection, table, ""), before[index], "{table}");
    }
    assert_eq!(fts_count(&connection, "目标猫猫"), 1);
    assert_eq!(count(&connection, "SELECT COUNT(*) FROM pragma_foreign_key_check"), 0);
    connection.execute_batch("DROP TRIGGER fail_session_delete;").unwrap();
    assert_eq!(delete(&mut connection, 1, target).unwrap(), Some(survivor.to_string()));
    assert_eq!(fts_count(&connection, "目标猫猫"), 0);
}
