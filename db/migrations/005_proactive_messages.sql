-- 复用会话空闲累计，只增加抽签位置与成功冷却，不新建任务表。
ALTER TABLE session_pet_states ADD COLUMN proactive_next_check_idle_ms INTEGER CHECK (proactive_next_check_idle_ms >= 0);
ALTER TABLE session_pet_states ADD COLUMN last_proactive_sent_at TEXT;
-- 旧版本尚未实现主动发送；已在无聊中的会话从现有进度再等 5 分钟。
UPDATE session_pet_states SET proactive_next_check_idle_ms=idle_elapsed_ms+300000 WHERE idle_branch='bored';
