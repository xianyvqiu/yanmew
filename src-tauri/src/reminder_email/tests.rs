use super::*;

fn input(revision: u64, secret: SecretUpdate) -> Input {
    Input {
        revision,
        secret,
        connection: ConnectionConfig {
            email: "user@qq.com".into(),
            host: "smtp.qq.com".into(),
            port: 465,
            encryption: "tls".into(),
            username: "user@qq.com".into(),
            from: "user@qq.com".into(),
            to: "user@qq.com".into(),
        },
    }
}
fn configured() -> Runtime {
    let runtime = crate::runtime::test_runtime("https://example.invalid");
    runtime
        .read(|s, p| {
            save(
                &mut s.connection,
                p,
                input(
                    0,
                    SecretUpdate::Set {
                        value: "secret-test-only".into(),
                    },
                ),
            )?;
            Ok(())
        })
        .unwrap();
    runtime
}
fn challenge(runtime: &Runtime, code: &str) {
    let expected = runtime
        .read(|s, _| fingerprint(&s.connection, &load(&s.connection)?))
        .unwrap();
    *runtime.reminder_email.challenge.lock().unwrap() = Some(Challenge {
        digest: code_digest(code, &expected),
        fingerprint: expected,
        expires: Instant::now() + Duration::from_secs(600),
        attempts: 0,
        delivered: true,
    });
}
pub(super) fn verified() -> Runtime {
    let runtime = configured();
    challenge(&runtime, "123456");
    verify(&runtime, "123456", 1).unwrap();
    runtime
}
pub(super) fn occurrence(c: &Connection, name: &str, at: &str) -> i64 {
    c.execute("INSERT INTO reminders(name,kind,content,status,fire_at,source,created_at) VALUES(?1,'once',?2,'pending',?3,'manual',?3)",params![name,format!("{name}详情"),at]).unwrap();
    let rule = c.last_insert_rowid();
    c.execute("INSERT INTO reminder_occurrences(reminder_id,scheduled_at,status,delivery_state,created_at) VALUES(?1,?2,'pending','queued',?2)",params![rule,at]).unwrap();
    c.last_insert_rowid()
}
#[test]
fn configuration_requires_binding_and_changes_invalidate_old_code() {
    let runtime = configured();
    assert_eq!(
        runtime
            .read(|s, _| enable(&mut s.connection, true, 1, Utc::now()))
            .err()
            .unwrap()
            .code,
        "EMAIL_NOT_VERIFIED"
    );
    challenge(&runtime, "123456");
    runtime
        .read(|s, p| {
            let mut value = input(1, SecretUpdate::Keep);
            value.connection.to = "other@qq.com".into();
            save(&mut s.connection, p, value)
        })
        .unwrap();
    assert_eq!(
        verify(&runtime, "123456", 1).err().unwrap().code,
        "EMAIL_CODE_EXPIRED"
    );
    assert!(!runtime.read(|s, _| view(&s.connection)).unwrap().enabled);
}
#[test]
fn verification_expires_and_five_wrong_codes_exhaust_challenge() {
    let runtime = configured();
    challenge(&runtime, "123456");
    runtime
        .reminder_email
        .challenge
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .expires = Instant::now() - Duration::from_secs(1);
    assert_eq!(
        verify(&runtime, "123456", 1).err().unwrap().code,
        "EMAIL_CODE_EXPIRED"
    );
    challenge(&runtime, "123456");
    for _ in 0..4 {
        assert_eq!(
            verify(&runtime, "000000", 1).err().unwrap().code,
            "EMAIL_CODE_WRONG"
        );
    }
    assert_eq!(
        verify(&runtime, "000000", 1).err().unwrap().code,
        "EMAIL_CODE_EXPIRED"
    );
    assert_eq!(
        verify(&runtime, "123456", 1).err().unwrap().code,
        "EMAIL_CODE_REQUIRED"
    );
}
#[test]
fn off_retains_binding_but_cancels_old_tasks_and_enable_does_not_replay_history() {
    let runtime = verified();
    runtime
        .read(|s, _| {
            let now = Utc::now() + chrono::Duration::seconds(1);
            let at = stamp(now);
            let id = occurrence(&s.connection, "喝水", &at);
            enqueue(&s.connection, &[id], false, now)?;
            let old = occurrence(
                &s.connection,
                "启用前旧事项",
                &stamp(now - chrono::Duration::days(1)),
            );
            enqueue(&s.connection, &[old], false, now)?;
            assert_eq!(list_count(&s.connection), 1);
            let disabled = enable(&mut s.connection, false, 1, now)?;
            assert!(disabled.verified);
            assert!(disabled.has_secret);
            assert_eq!(disabled.deliveries[0].state, "cancelled");
            let enabled = enable(&mut s.connection, true, disabled.revision, now)?;
            assert!(enabled.enabled);
            assert_eq!(enabled.deliveries[0].state, "cancelled");
            Ok(())
        })
        .unwrap();
}
fn list_count(c: &Connection) -> i64 {
    c.query_row("SELECT count(*) FROM reminder_email_deliveries", [], |r| {
        r.get(0)
    })
    .unwrap()
}
#[test]
fn offline_batch_is_unique_and_deleted_sources_are_pruned_before_claim() {
    let runtime = verified();
    runtime.read(|s,_|{
        let now=Utc::now()+chrono::Duration::seconds(1);let at=stamp(now);
        let first=occurrence(&s.connection,"喝水",&at);let second=occurrence(&s.connection,"吃饭",&at);
        enqueue(&s.connection,&[first,second],true,now)?;enqueue(&s.connection,&[first,second],true,now)?;
        assert_eq!(list_count(&s.connection),1);
        s.connection.execute("DELETE FROM reminders WHERE id=(SELECT reminder_id FROM reminder_occurrences WHERE id=?1)",[first])?;Ok(())
    }).unwrap();
    // claim复用运行时的密钥解密和配置校验，正文只留下仍存在的原始事项。
    let task = outbox::test_claim(&runtime).unwrap().unwrap();
    assert!(task.1.contains("吃饭详情"));
    assert!(!task.1.contains("喝水"));
    runtime
        .read(|s, _| {
            outbox::recover(&s.connection)?;
            assert_eq!(view(&s.connection)?.deliveries[0].state, "uncertain");
            Ok(())
        })
        .unwrap();
    let error = runtime
        .read(|s, _| outbox::retry(&mut s.connection, task.0, 1, false))
        .err()
        .unwrap();
    assert_eq!(error.code, "EMAIL_CONFIRM_RESEND");
    runtime
        .read(|s, _| {
            s.connection.execute("DELETE FROM reminders", [])?;
            outbox::retry(&mut s.connection, task.0, 1, true)?;
            Ok(())
        })
        .unwrap();
    assert!(outbox::test_claim(&runtime).unwrap().is_none());
    assert_eq!(
        runtime.read(|s, _| view(&s.connection)).unwrap().deliveries[0].state,
        "cancelled"
    );
}
#[test]
fn reminder_dispatch_registers_only_new_due_events_in_same_transaction() {
    let runtime = verified();
    runtime.read(|s,_|{
        let now=Utc::now();let fire=stamp(now+chrono::Duration::seconds(10));
        s.connection.execute("INSERT INTO reminders(name,kind,content,status,fire_at,source,created_at) VALUES('到点事项','once','原始详情','pending',?1,'manual',?2)",params![fire,stamp(now)])?;
        assert_eq!(crate::reminders::dispatch(&mut s.connection,now+chrono::Duration::seconds(20),&Utc,false)?,1);
        assert_eq!(crate::reminders::dispatch(&mut s.connection,now+chrono::Duration::seconds(21),&Utc,false)?,0);
        assert_eq!(list_count(&s.connection),1);assert_eq!(view(&s.connection)?.deliveries[0].state,"pending");Ok(())
    }).unwrap();
}
#[test]
fn public_backup_resets_binding_and_excludes_credential_keys() {
    let runtime = verified();
    runtime
        .read(|s, _| {
            let raw: String = s.connection.query_row(
                "SELECT value FROM settings WHERE key=?1",
                [CONFIG_KEY],
                |r| r.get(0),
            )?;
            let safe = public_config(&raw)?;
            let imported: Config = serde_json::from_str(&safe).unwrap();
            assert!(!safe.contains("secret-test-only"));
            assert!(!imported.enabled);
            assert!(!imported.verified);
            assert!(imported.enabled_since.is_none());
            assert_eq!(imported.revision, 0);
            assert_eq!(imported.connection.host, "smtp.qq.com");
            Ok(())
        })
        .unwrap();
}
