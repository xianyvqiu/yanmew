-- ============================================================
-- 颜喵 YanMew 数据库结构（SQLite）
-- 运行库文件：应用目录下 data/yanwenzi.db（便携版，与可执行文件同目录）
-- 本文件是建库脚本，仅描述结构，不包含运行期数据。
-- ============================================================

-- SQLite 默认不启用外键约束，本项目的 Rust 层在建立连接后执行：
-- PRAGMA foreign_keys = ON;
-- 会话级数据使用 ON DELETE CASCADE 做数据库级级联删除。

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
-- prompt 是当前人设，AI 自调会直接改写。
-- initial_prompt 是用户手动保存的初始设定，AI 不会改写。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS personas (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    name                TEXT    NOT NULL,                    -- 人设名称
    prompt              TEXT    NOT NULL,                    -- 当前人设 prompt
    initial_prompt      TEXT    NOT NULL,                    -- 用户手动保存的初始设定
    self_evolve_enabled INTEGER NOT NULL DEFAULT 1,          -- 人设自调开关：1 开启，0 关闭
    last_evolved_at     TEXT,                                -- 最近一次 AI 调整时间 RFC3339，可空
    color               TEXT    NOT NULL DEFAULT '#ffb6c1',  -- 桌宠本体颜色（十六进制），按人设绑定
    chat_bg_file        TEXT,                                -- 聊天窗背景图相对文件名，可空；图片保存在 data/chat_background/
    chat_bg_opacity     INTEGER NOT NULL DEFAULT 85,         -- 聊天窗背景透明度百分比 0–100
    chat_bg_blur        INTEGER NOT NULL DEFAULT 0           -- 聊天窗背景模糊像素 0–40
    is_active           INTEGER NOT NULL DEFAULT 0,          -- 当前使用的人设（全局唯一为 1）
    expressions         TEXT                                 -- 分类表情库 JSON；未生成时为 NULL
);

-- ------------------------------------------------------------
-- 3. 会话
-- 每个会话归属一个人设；记忆、记事本、饰品、旅行、信件、伴手礼都按会话隔离。
-- summary_cursor 用于记录“已总结到哪条消息”，避免重复生成短期记忆。
-- 旅行相关字段记录当前会话的在线出行计时，只在应用开启且宠物在家时累计。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sessions (
    id                            INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    persona_id                    INTEGER NOT NULL,                    -- 所属人设 -> personas.id
    title                         TEXT    NOT NULL,                    -- 会话标题，可 AI 生成、用户重命名
    created_at                    TEXT    NOT NULL,                    -- 创建时间 RFC3339
    updated_at                    TEXT    NOT NULL,                    -- 最近更新时间 RFC3339
    summary_cursor                INTEGER NOT NULL DEFAULT 0,          -- 已总结到的最后一条消息 id
    intimacy                      INTEGER NOT NULL DEFAULT 0,          -- 当前会话亲密度
    last_return_at                TEXT,                                -- 最近一次旅行回来时间 RFC3339，可空
    active_seconds_since_return   INTEGER NOT NULL DEFAULT 0,          -- 本次回来后累计的在线秒数
    next_travel_after_seconds     INTEGER,                             -- 下次可出行的在线秒数阈值，可空
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
--   hungry_minutes                    饥饿触发分钟数
--   bored_minutes                     无聊触发分钟数
--   sleep_minutes                     多久没互动进入睡觉
--   idle_bubble_minutes               空闲气泡触发分钟数
--   short_memory_min_chars            短期记忆最小字数
--   short_memory_max_chars            短期记忆最大字数
--   long_memory_min_chars             长期记忆最小字数
--   long_memory_max_chars             长期记忆最大字数
--   merge_memory_min_chars            手动合并记忆最小字数
--   merge_memory_max_chars            手动合并记忆最大字数
--   tool_call_rounds                  AI 单次回复最多工具调用轮数，默认 3
--   notebook_max_chars                记事本最大字数，默认 4000
--   intimacy_threshold_1              第一档亲密度，默认 10
--   intimacy_threshold_2              第二档亲密度，默认 50
--   intimacy_threshold_3              第三档亲密度，默认 150
--   intimacy_threshold_4              第四档亲密度，默认 300
--   accessory_cost_1                  第一档饰品价格，默认 20
--   accessory_cost_2                  第二档饰品价格，默认 50
--   accessory_cost_3                  第三档饰品价格，默认 120
--   accessory_cost_4                  第四档及以后饰品价格，默认 300
--   daily_game_limit                  石头剪刀布每日次数，默认 20
--   sign_in_min_coins                 每日签到最小喵币，默认 1
--   sign_in_max_coins                 每日签到最大喵币，默认 10
--   game_win_coins                    石头剪刀布胜利奖励，默认 5
--   game_loss_coins                   石头剪刀布失败扣除，默认 4
--   travel_min_online_hours           出行最短在线间隔，默认 4
--   travel_max_online_hours           出行最长在线间隔，默认 24
--   travel_min_duration_minutes       出行最短分钟数，默认 60
--   travel_max_duration_minutes       出行最长分钟数，默认 180
--   travel_accessory_chance_percent   旅行带回饰品概率，默认 10
--   letter_reply_min_minutes          回信最短延迟，默认 10
--   letter_reply_max_minutes          回信最长延迟，默认 30
--   weather_qweather_api_key_encrypted  和风天气 API Key（DPAPI 加密后的 Base64，可选）
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,   -- 设置键
    value TEXT NOT NULL       -- 设置值，统一存字符串
);

-- ------------------------------------------------------------
-- 6. 短期记忆
-- 由“最近 30 条之外的旧消息”按连续 10 条一组生成；旅行返回也会生成一条。
-- 满 10 条短期记忆会整合成 1 条长期记忆，然后删除这 10 条。
-- 排序统一用 id 升序，不再单独维护序号。
-- source_type：chat=聊天摘要，travel=旅行总结。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS short_memories (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id    INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    content       TEXT    NOT NULL,                    -- 记忆正文（中文事实记录）
    source_type   TEXT    NOT NULL DEFAULT 'chat',     -- chat / travel
    source_ref_id INTEGER,                             -- 旅行总结时为 travels.id，聊天摘要为 NULL
    created_at    TEXT    NOT NULL,                    -- 创建时间 RFC3339
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
-- 10. 用户全局状态
-- 单行表，主键固定为 1；喵币是全局用户级数据，不随会话删除。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS user_state (
    id         INTEGER PRIMARY KEY CHECK (id = 1),  -- 固定单行，主键为 1
    coins      INTEGER NOT NULL DEFAULT 0,          -- 当前喵币余额
    updated_at TEXT    NOT NULL                     -- 最近更新时间 RFC3339
);

-- ------------------------------------------------------------
-- 11. 喵币账单
-- 所有喵币变动都记一条流水，用于余额核对、每日签到/游戏次数判断。
-- reason: daily_sign_in / game_win / game_loss / game_draw / accessory_craft
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS currency_ledger (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    delta      INTEGER NOT NULL,                    -- 变动数量，可为负数
    reason     TEXT    NOT NULL,                    -- 变动原因
    ref_id     INTEGER,                             -- 关联业务 id，可空
    created_at TEXT    NOT NULL                     -- 创建时间 RFC3339
);

-- ------------------------------------------------------------
-- 12. 旅行
-- status: away=出门中，returned=已回来。
-- planned_return_at 用于判断关闭应用后是否已经到返回时间。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travels (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id        INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    status            TEXT    NOT NULL,                    -- away / returned
    started_at        TEXT    NOT NULL,                    -- 出发时间 RFC3339
    planned_return_at TEXT    NOT NULL,                    -- 计划返回时间 RFC3339
    actual_return_at  TEXT,                                -- 实际返回时间 RFC3339，可空
    created_at        TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 13. 饰品
-- 每个饰品属于一个会话；source: crafted=制作，travel=旅行带回。
-- v1 只有头顶装饰槽；is_worn=1 表示当前佩戴。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS accessories (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id   INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    name         TEXT    NOT NULL,                    -- 饰品名称
    display_text TEXT    NOT NULL,                    -- 头顶显示的文字/emoji/颜文字
    description  TEXT    NOT NULL,                    -- AI 按人设口吻写的简介
    cost         INTEGER NOT NULL DEFAULT 0,          -- 制作费用；旅行带回为 0
    source       TEXT    NOT NULL,                    -- crafted / travel
    travel_id    INTEGER,                             -- 旅行带回时为 travels.id，制作为 NULL
    is_worn      INTEGER NOT NULL DEFAULT 0,          -- 是否佩戴：1 佩戴，0 未佩戴
    created_at   TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)  REFERENCES travels(id)   ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 14. 信件
-- sender: user=用户写的信，ai=AI 写的信。
-- reply_to_id 非空时表示这是对某封信的回复；每封信最多回复一次。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS letters (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id   INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    travel_id    INTEGER,                             -- 关联旅行 -> travels.id，可空
    sender       TEXT    NOT NULL,                    -- user / ai
    content      TEXT    NOT NULL,                    -- 信件正文
    reply_to_id  INTEGER,                             -- 回复的信件 -> letters.id，可空
    is_favorite  INTEGER NOT NULL DEFAULT 0,          -- 是否收藏：1 收藏，0 未收藏
    is_read      INTEGER NOT NULL DEFAULT 0,          -- 用户是否已读：1 已读，0 未读
    created_at   TEXT    NOT NULL,                    -- 写信/回信时间 RFC3339
    FOREIGN KEY (session_id)  REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)   REFERENCES travels(id)  ON DELETE SET NULL,
    FOREIGN KEY (reply_to_id) REFERENCES letters(id)  ON DELETE SET NULL
);

-- 每封信只能被回复一次：reply_to_id 非空时唯一。
CREATE UNIQUE INDEX IF NOT EXISTS idx_letters_reply_to_unique
    ON letters(reply_to_id)
    WHERE reply_to_id IS NOT NULL;

-- ------------------------------------------------------------
-- 15. 伴手礼
-- 只保存普通文字礼物；旅行带回的可佩戴饰品存 accessories 表。
-- 不做背包、数量、容量或使用机制。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS souvenirs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id  INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    travel_id   INTEGER,                             -- 来源旅行 -> travels.id，可空
    content     TEXT    NOT NULL,                    -- 文字礼物内容
    is_favorite INTEGER NOT NULL DEFAULT 0,          -- 是否收藏：1 收藏，0 未收藏
    created_at  TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)  REFERENCES travels(id)  ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 常用索引（加快按会话查询）
-- ------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_messages_session        ON messages (session_id);
CREATE INDEX IF NOT EXISTS idx_sessions_persona        ON sessions (persona_id);
CREATE INDEX IF NOT EXISTS idx_short_memories_session  ON short_memories (session_id);
CREATE INDEX IF NOT EXISTS idx_long_memories_session   ON long_memories (session_id);
CREATE INDEX IF NOT EXISTS idx_reminders_status_fire   ON reminders (status, fire_at);
CREATE INDEX IF NOT EXISTS idx_currency_ledger_created ON currency_ledger (created_at);
CREATE INDEX IF NOT EXISTS idx_accessories_session     ON accessories (session_id);
CREATE INDEX IF NOT EXISTS idx_travels_session_status  ON travels (session_id, status);
CREATE INDEX IF NOT EXISTS idx_letters_session         ON letters (session_id);
CREATE INDEX IF NOT EXISTS idx_souvenirs_session       ON souvenirs (session_id);