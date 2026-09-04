-- ============================================================
-- 颜喵 YanMew 数据库结构（SQLite）
-- 运行库文件：应用目录下 data/yanwenzi.db（便携版，与可执行文件同目录）
-- 本文件是建库脚本，仅描述结构，不包含运行期数据。
-- ============================================================

-- SQLite 默认不启用外键约束，本项目的 Rust 层在建立连接后执行：
-- PRAGMA foreign_keys = ON;
-- 会话/记忆/记事本使用 ON DELETE CASCADE 做数据库级级联删除。

-- ------------------------------------------------------------
-- 1. API 提供商
-- 官方 OpenAI 与自定义 OpenAI 兼容接口统一用本表管理。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS providers (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    name              TEXT    NOT NULL,                    -- 显示名称，用户自定义
    base_url          TEXT,                                -- 自定义接口地址；官方 OpenAI 为 NULL
    api_key_encrypted TEXT    NOT NULL,                    -- DPAPI 加密后的 Base64，不存明文
    model             TEXT    NOT NULL,                    -- 模型名，如 gpt-4o-mini / deepseek-chat
    is_active         INTEGER NOT NULL DEFAULT 0,          -- 是否为聊天当前提供商（全局唯一为 1）
    memory_is_active  INTEGER NOT NULL DEFAULT 0           -- 是否为记忆整合专用提供商；为 0 时跟随 is_active
);

-- ------------------------------------------------------------
-- 2. 人设（AI 角色）
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS personas (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    name        TEXT    NOT NULL,                    -- 人设名称
    prompt      TEXT    NOT NULL,                    -- 人设 prompt
    is_active   INTEGER NOT NULL DEFAULT 0,          -- 当前使用的人设（全局唯一为 1）
    expressions TEXT                                -- 分类表情库 JSON；未生成时为 NULL
);

-- ------------------------------------------------------------
-- 3. 会话
-- 每个会话归属一个人设；记忆与记事本都按会话隔离。
-- summary_cursor 用于记录“已总结到哪条消息”，避免重复生成短期记忆。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sessions (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    persona_id     INTEGER NOT NULL,                    -- 所属人设 -> personas.id
    title          TEXT    NOT NULL,                    -- 会话标题，可 AI 生成、用户重命名
    created_at     TEXT    NOT NULL,                    -- 创建时间 RFC3339
    updated_at     TEXT    NOT NULL,                    -- 最近更新时间 RFC3339
    summary_cursor INTEGER NOT NULL DEFAULT 0,          -- 已总结到的最后一条消息 id
    FOREIGN KEY (persona_id) REFERENCES personas(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 4. 消息
-- 完整保存会话历史；记忆是历史之外的“摘要层”，不影响原文保存。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    role       TEXT    NOT NULL,                    -- user / assistant
    content    TEXT    NOT NULL,                    -- 消息正文
    expression TEXT,                                -- AI 回复时使用的颜文字；用户消息为 NULL
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 5. 设置（键值对）
-- 常用键：
--   hungry_minutes            饥饿触发分钟数
--   bored_minutes             无聊触发分钟数
--   sleep_minutes             多久没互动进入睡觉
--   idle_bubble_minutes       空闲气泡触发分钟数
--   short_memory_min_chars    短期记忆最小字数
--   short_memory_max_chars    短期记忆最大字数
--   long_memory_min_chars     长期记忆最小字数
--   long_memory_max_chars     长期记忆最大字数
--   merge_memory_min_chars    手动合并记忆最小字数
--   merge_memory_max_chars    手动合并记忆最大字数
--   tool_call_rounds          AI 单次回复最多工具调用轮数，默认 3
--   notebook_max_chars        记事本最大字数，默认 4000
--   weather_qweather_api_key_encrypted  和风天气 API Key（DPAPI 加密后的 Base64，可选）
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,   -- 设置键
    value TEXT NOT NULL       -- 设置值，统一存字符串
);

-- ------------------------------------------------------------
-- 6. 短期记忆
-- 由“最近 30 条之外的旧消息”按连续 10 条一组生成。
-- 满 10 条短期记忆会整合成 1 条长期记忆，然后删除这 10 条。
-- 排序统一用 id 升序，不再单独维护序号。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS short_memories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    content    TEXT    NOT NULL,                    -- 记忆正文（中文事实记录）
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 7. 长期记忆
-- 由 10 条短期记忆整合生成，不设自动上限，全部进入聊天上下文。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS long_memories (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    content    TEXT    NOT NULL,                    -- 记忆正文（中文事实记录）
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 8. 记事本
-- 一个会话一个记事本；session_id 即主键，一行就是整份记事本。
-- content 按空行分段，段落从 1 开始编号，供 AI 结构化修改。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS notebooks (
    session_id INTEGER PRIMARY KEY,   -- 所属会话 -> sessions.id（一个会话一行）
    content    TEXT    NOT NULL,      -- 记事本全文
    updated_at TEXT    NOT NULL,      -- 最近更新时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 9. 提醒
-- kind: once / daily / weekly / monthly
-- schedule: 重复规则 JSON；一次性提醒为 NULL。
--   每天：{"repeat":"daily","time":"09:00"}
--   每周：{"repeat":"weekly","weekdays":[1,3,5],"time":"09:00"}（1=周一 … 7=周日）
--   每月：{"repeat":"monthly","days":[1,15],"time":"09:00"}
-- status: pending / done / expired
-- fire_at 始终保存下一次具体触发时间（RFC3339）；一次性提醒触发后置 NULL 避免重复触发。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS reminders (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    kind       TEXT    NOT NULL,                    -- once / daily / weekly / monthly
    schedule   TEXT,                                -- 重复规则 JSON；一次性为 NULL
    content    TEXT    NOT NULL,                    -- 提醒内容
    status     TEXT    NOT NULL,                    -- pending / done / expired
    fire_at    TEXT,                                -- 下次触发时间 RFC3339
    source     TEXT    NOT NULL,                    -- chat / manual
    created_at TEXT    NOT NULL                     -- 创建时间 RFC3339
);

-- ------------------------------------------------------------
-- 常用索引（加快按会话查询）
-- ------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_messages_session       ON messages (session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_persona       ON sessions (persona_id);
CREATE INDEX IF NOT EXISTS idx_short_memories_session ON short_memories (session_id);
CREATE INDEX IF NOT EXISTS idx_long_memories_session  ON long_memories (session_id);
CREATE INDEX IF NOT EXISTS idx_reminders_status_fire  ON reminders (status, fire_at);
