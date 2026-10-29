use super::*;
use crate::{models::MessageRole, sessions};

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0))
        .unwrap()
}

#[test]
fn fresh_database_creates_one_real_guide_without_provider_or_rewards() {
    let mut connection = Connection::open_in_memory().unwrap();
    assert!(initialize(&mut connection).unwrap());
    let context = crate::persona::context(&connection, 0).unwrap();
    let persona_id = crate::error::parse_id(&context.persona_id).unwrap();
    let colors: (String, Option<String>, Option<String>) = connection.query_row("SELECT color,chat_user_text_color,chat_assistant_text_color FROM personas WHERE id=?1", [persona_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(colors, ("#ffb6c1".into(), None, None));
    let session_id = crate::error::parse_id(context.session_id.as_ref().unwrap()).unwrap();
    let session = sessions::get(&connection, persona_id, session_id).unwrap();
    assert_eq!(session.title, "初次见面");
    assert!(session.is_current);
    assert_eq!(session.user_profile_id, None);
    assert_eq!(session.intimacy, 0);
    assert_eq!(session.memory_revision, 0);
    let page = sessions::messages(&connection, persona_id, session_id, None, 30).unwrap();
    assert_eq!(page.items.len(), 1);
    let guide = &page.items[0];
    assert_eq!(guide.role, MessageRole::Assistant);
    assert_eq!(guide.round_id, None);
    assert_eq!(guide.content, sessions::INITIAL_GUIDE);
    assert!(chrono::DateTime::parse_from_rfc3339(&guide.created_at).is_ok());
    for table in ["providers", "user_profiles", "chat_rounds", "short_memories", "long_memories"] {
        assert_eq!(count(&connection, table), 0, "{table}");
    }
    assert_eq!(count(&connection, "sessions"), 1);
    assert_eq!(count(&connection, "session_pet_states"), 1);
    let state = crate::pet_store::read(&connection, session_id).unwrap();
    assert_eq!(state.hunger_elapsed_ms, 0);
    assert_eq!(state.idle_elapsed_ms, 0);
    assert_eq!(state.last_proactive_sent_at, None);
    assert_eq!(connection.query_row("SELECT coins FROM user_state", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert!(!initialize(&mut connection).unwrap());
    assert_eq!(count(&connection, "messages"), 1);
}

#[test]
fn existing_empty_database_and_custom_selection_are_not_backfilled() {
    let mut connection = test_database();
    assert!(!initialize(&mut connection).unwrap());
    assert_eq!(count(&connection, "sessions"), 0);
    connection.execute_batch("UPDATE personas SET is_active=0;
        INSERT INTO personas(id,name,prompt,initial_prompt,is_active) VALUES(57,'我的人设','正文','正文',1);").unwrap();
    let session = sessions::create(&mut connection, 57).unwrap();
    assert!(!initialize(&mut connection).unwrap());
    assert_eq!(crate::persona::context(&connection, 0).unwrap().session_id, Some(session.id));
    assert_eq!(count(&connection, "sessions"), 1);
    assert_eq!(count(&connection, "messages"), 0);
}

#[test]
fn edited_guide_persists_and_tail_deletion_does_not_resurrect_it() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    runtime.read(|store, _| {
        let mut fresh = Connection::open_in_memory()?;
        assert!(initialize(&mut fresh)?);
        store.connection = fresh;
        Ok(())
    }).unwrap();
    let edited = sessions::messages::edit(&runtime, 1, 1, 0, 1, sessions::INITIAL_GUIDE, "我修改后的指南").unwrap();
    runtime.read(|store, _| {
        assert!(!initialize(&mut store.connection)?);
        assert_eq!(sessions::messages(&store.connection, 1, 1, None, 30)?.items[0].content, edited.content);
        Ok(())
    }).unwrap();
    sessions::messages::delete_tail(&runtime, 1, 1, 0, 1, &edited.content).unwrap();
    runtime.read(|store, _| {
        assert!(!initialize(&mut store.connection)?);
        assert_eq!(count(&store.connection, "messages"), 0);
        assert_eq!(count(&store.connection, "sessions"), 1);
        assert_eq!(count(&store.connection, "short_memories"), 0);
        assert_eq!(count(&store.connection, "long_memories"), 0);
        sessions::delete(&mut store.connection, 1, 1)?;
        assert!(!initialize(&mut store.connection)?);
        assert_eq!(count(&store.connection, "sessions"), 0);
        assert_eq!(crate::persona::context(&store.connection, 0)?.session_id, None);
        Ok(())
    }).unwrap();
}

#[test]
fn ordinary_new_sessions_do_not_receive_a_guide() {
    let mut connection = Connection::open_in_memory().unwrap();
    assert!(initialize(&mut connection).unwrap());
    let second = sessions::create(&mut connection, 1).unwrap();
    assert_eq!(second.title, "新会话");
    assert!(sessions::messages(&connection, 1, second.id.parse().unwrap(), None, 30).unwrap().items.is_empty());
    assert!(!initialize(&mut connection).unwrap());
    assert_eq!(count(&connection, "messages"), 1);
}

#[test]
fn guide_creation_uses_builtin_marker_and_rolls_back_on_message_failure() {
    let mut connection = test_database();
    connection.execute_batch("PRAGMA defer_foreign_keys=ON; BEGIN; UPDATE personas SET id=57 WHERE is_builtin=1; UPDATE persona_food_settings SET persona_id=57 WHERE persona_id=1; COMMIT;").unwrap();
    connection.execute_batch("CREATE TRIGGER reject_guide BEFORE INSERT ON messages
        BEGIN SELECT RAISE(ABORT,'test guide write failure'); END;").unwrap();
    {
        let transaction = connection.transaction().unwrap();
        assert!(sessions::create_initial_in_transaction(&transaction).is_err());
        // 事务离开作用域自动回滚，包括已经写入的会话、当前选择和状态触发器结果。
    }
    assert_eq!(count(&connection, "sessions"), 0);
    assert_eq!(count(&connection, "session_pet_states"), 0);
    assert_eq!(count(&connection, "messages"), 0);
    assert_eq!(count(&connection, "settings"), crate::config::defaults().len() as i64);
    connection.execute_batch("DROP TRIGGER reject_guide").unwrap();
    let transaction = connection.transaction().unwrap();
    sessions::create_initial_in_transaction(&transaction).unwrap();
    transaction.commit().unwrap();
    assert_eq!(sessions::list(&connection, 57).unwrap()[0].title, "初次见面");
}

#[test]
fn guide_does_not_count_as_a_round_and_cannot_skip_later_history() {
    let runtime = crate::runtime::test_runtime("http://127.0.0.1:9/v1");
    runtime.read(|store, _| {
        let mut fresh = Connection::open_in_memory()?;
        initialize(&mut fresh)?;
        store.connection = fresh;
        let pending = sessions::start_round(&mut store.connection, 1, 1, "你好")?;
        sessions::complete_round(&mut store.connection, 1, 1, pending.round.id.parse().unwrap(), "你好呀", None)?;
        assert_eq!(count(&store.connection, "chat_rounds"), 1);
        assert_eq!(sessions::get(&store.connection, 1, 1)?.intimacy, 1);
        assert_eq!(sessions::get(&store.connection, 1, 1)?.title, "初次见面");
        assert_eq!(count(&store.connection, "messages"), 3);
        Ok(())
    }).unwrap();
    assert_eq!(sessions::messages::delete_tail(&runtime, 1, 1, 0, 1, sessions::INITIAL_GUIDE).unwrap_err().code, "ROUND_NOT_TAIL");
}

#[test]
fn tutorial_only_marks_fresh_data_and_completion_survives_restart() {
    let mut fresh = Connection::open_in_memory().unwrap();
    initialize(&mut fresh).unwrap();
    assert!(crate::config::onboarding_pending(&fresh).unwrap());
    initialize(&mut fresh).unwrap();
    assert!(crate::config::onboarding_pending(&fresh).unwrap());
    crate::config::finish_onboarding(&fresh).unwrap();
    crate::config::finish_onboarding(&fresh).unwrap();
    initialize(&mut fresh).unwrap();
    assert!(!crate::config::onboarding_pending(&fresh).unwrap());
    let mut old = test_database();
    initialize(&mut old).unwrap();
    assert!(!crate::config::onboarding_pending(&old).unwrap());
}
