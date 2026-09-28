-- 版本 2→3：亲密度封顶 999。由 db.rs 在一致性备份后、同一迁移事务内执行。
-- 保留会话 ID、子表及历史行为；超上限旧值收敛到新上限。
UPDATE sessions SET intimacy=999 WHERE intimacy>999;

-- 保持四档严格递增，为后续档位预留至少 1 点；已在范围内的值不改变。
UPDATE settings SET value=CAST(min(CAST(value AS INTEGER),996) AS TEXT)
WHERE key='intimacy_threshold_1' AND CAST(value AS INTEGER)>996;
UPDATE settings SET value=CAST(min(CAST(value AS INTEGER),997) AS TEXT)
WHERE key='intimacy_threshold_2' AND CAST(value AS INTEGER)>997;
UPDATE settings SET value=CAST(min(CAST(value AS INTEGER),998) AS TEXT)
WHERE key='intimacy_threshold_3' AND CAST(value AS INTEGER)>998;
UPDATE settings SET value=CAST(min(CAST(value AS INTEGER),999) AS TEXT)
WHERE key='intimacy_threshold_4' AND CAST(value AS INTEGER)>999;

CREATE TRIGGER IF NOT EXISTS sessions_intimacy_insert BEFORE INSERT ON sessions
WHEN typeof(NEW.intimacy) <> 'integer' OR NEW.intimacy NOT BETWEEN 0 AND 999
BEGIN
    SELECT RAISE(ABORT, 'intimacy must be an integer between 0 and 999');
END;
CREATE TRIGGER IF NOT EXISTS sessions_intimacy_update BEFORE UPDATE OF intimacy ON sessions
WHEN typeof(NEW.intimacy) <> 'integer' OR NEW.intimacy NOT BETWEEN 0 AND 999
BEGIN
    SELECT RAISE(ABORT, 'intimacy must be an integer between 0 and 999');
END;
