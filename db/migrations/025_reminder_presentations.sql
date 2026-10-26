-- 旧记录保持legacy，不补生成、补记聊天或补发邮件。
ALTER TABLE reminder_occurrences ADD COLUMN presentation_state TEXT NOT NULL DEFAULT 'legacy' CHECK (presentation_state IN ('legacy','pending','generating','ready'));
ALTER TABLE reminder_occurrences ADD COLUMN presentation_persona_id INTEGER REFERENCES personas(id) ON DELETE SET NULL;
ALTER TABLE reminder_occurrences ADD COLUMN presentation_session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL;
ALTER TABLE reminder_occurrences ADD COLUMN presentation_input TEXT CHECK (presentation_input IS NULL OR json_valid(presentation_input));
ALTER TABLE reminder_occurrences ADD COLUMN presentation_text TEXT;
ALTER TABLE reminder_occurrences ADD COLUMN presentation_message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_reminder_presentation_pending ON reminder_occurrences(presentation_state,id);
