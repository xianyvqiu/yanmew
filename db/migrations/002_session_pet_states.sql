-- 会话桌宠状态：有效运行计时与真实时间睡眠分开保存。
CREATE TABLE IF NOT EXISTS session_pet_states (
    session_id          INTEGER PRIMARY KEY, -- 所属会话，主键兼外键；一会话一行，删除会话级联删除
    hunger_elapsed_ms   INTEGER NOT NULL DEFAULT 0 CHECK (hunger_elapsed_ms >= 0), -- 距上次喂食累计的有效运行毫秒，离线及非当前时间不累计
    idle_elapsed_ms     INTEGER NOT NULL DEFAULT 0 CHECK (idle_elapsed_ms >= 0), -- 本周期无互动的有效运行毫秒，合法互动或醒来归零
    idle_branch         TEXT NOT NULL DEFAULT 'none' CHECK (idle_branch IN ('none', 'bored', 'sleeping')), -- none=尚未选择；bored=本周期无聊；sleeping=本周期睡眠
    sleep_until         TEXT, -- 睡眠结束 UTC RFC3339，仅 sleeping 非空；关闭与切换期间仍自然经过
    last_fed_at         TEXT, -- 最近实际喂食 UTC RFC3339；未知为 NULL，不用于推算离线计时
    last_interacted_at  TEXT, -- 最近实际互动 UTC RFC3339；未知为 NULL，点击不清饥饿
    state_revision      INTEGER NOT NULL DEFAULT 0 CHECK (state_revision >= 0), -- 本状态写入版本，拒绝重复或迟到提交，不影响 memory_revision
    updated_at          TEXT NOT NULL, -- 最近状态提交 UTC RFC3339；初始值取会话创建时刻
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    CHECK ((idle_branch = 'sleeping' AND sleep_until IS NOT NULL AND length(sleep_until) > 0)
        OR (idle_branch != 'sleeping' AND sleep_until IS NULL))
);

CREATE TRIGGER IF NOT EXISTS sessions_create_pet_state
AFTER INSERT ON sessions
BEGIN
    INSERT INTO session_pet_states(session_id, updated_at)
    VALUES (NEW.id, NEW.created_at);
END;

-- 已有会话的历史行为未知：只补零计时与空操作时间，不根据离线天数推算。
INSERT INTO session_pet_states(session_id, updated_at)
SELECT id, created_at FROM sessions;
