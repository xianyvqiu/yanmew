-- v3 → v4：原地增加聊天预算与中断原因，保留所有已有消息、记忆和密钥。
ALTER TABLE providers ADD COLUMN max_output_tokens INTEGER NOT NULL DEFAULT 32768 CHECK (max_output_tokens > 0);
ALTER TABLE providers ADD COLUMN context_window_override INTEGER CHECK (context_window_override > 0);
ALTER TABLE chat_rounds ADD COLUMN stop_reason TEXT CHECK (stop_reason IS NULL OR stop_reason IN ('length_limit', 'user_cancelled', 'timeout', 'repetition', 'invalid_encoding', 'tool_limit', 'provider_error', 'context_changed', 'app_shutdown', 'recovered_after_restart', 'unknown'));

UPDATE chat_rounds SET stop_reason = 'unknown' WHERE status = 'interrupted';

CREATE TRIGGER IF NOT EXISTS chat_rounds_stop_reason_insert BEFORE INSERT ON chat_rounds
WHEN (NEW.status = 'interrupted' AND NEW.stop_reason IS NULL)
    OR (NEW.status <> 'interrupted' AND NEW.stop_reason IS NOT NULL)
BEGIN
    SELECT RAISE(ABORT, 'stop_reason must be present only for interrupted rounds');
END;
CREATE TRIGGER IF NOT EXISTS chat_rounds_stop_reason_update BEFORE UPDATE OF status, stop_reason ON chat_rounds
WHEN (NEW.status = 'interrupted' AND NEW.stop_reason IS NULL)
    OR (NEW.status <> 'interrupted' AND NEW.stop_reason IS NOT NULL)
BEGIN
    SELECT RAISE(ABORT, 'stop_reason must be present only for interrupted rounds');
END;
