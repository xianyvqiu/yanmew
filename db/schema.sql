-- ============================================================
-- 颜喵 YanMew 数据库结构（SQLite）
-- 运行库文件：应用目录下 data/yanwenzi.db（便携版，与可执行文件同目录）
-- 本文件是最新完整建库脚本，仅描述结构，不包含运行期数据。
-- 新库由 Rust 在事务中执行本文件；旧库通过版本化迁移升级，不靠 IF NOT EXISTS 更新旧表。
-- 所有时间统一存 UTC RFC3339（固定精度）；本地日期判断在 Rust 层转换。
-- ============================================================

-- SQLite 默认不启用外键约束，本项目的 Rust 层在建立连接后执行：
-- PRAGMA foreign_keys = ON;
-- 会话级数据使用 ON DELETE CASCADE 做数据库级级联删除。

-- ============================================================
-- 表关系总览
--
-- 全局级（不属于任何会话）：
--   providers          独立表，无外键；聊天与记忆整合各选一个启用项
--   embedding_providers 独立表；默认启用随应用打包的本地模型，也可选联网接口
--   personas           顶层容器：人设 → 会话 → 会话级数据
--   user_profiles      独立表；被 sessions 引用，删除时置空而非删会话
--   settings           键值对，全局配置
--   user_state         单行表，全局喵币余额
--   currency_ledger    喵币流水，与 user_state 逻辑配套
--   festival_catalog   系统内置节日母表，独立，不随任何业务数据删除
--   reminders          全局提醒规则
--   reminder_occurrences → reminders.id，删除规则级联删除每次触发记录
--
-- 会话级（都由 sessions 派生，删除会话时级联删除）：
--   sessions           主表
--                        persona_id      → personas.id        级联删除
--                        user_profile_id → user_profiles.id  置空
--   chat_rounds         session_id    → sessions.id          级联删除
--   messages            session_id / round_id → sessions.id / chat_rounds.id
--   short_memories      session_id    → sessions.id          级联删除
--                        round_id      → chat_rounds.id       级联删除
--   long_memories       session_id    → sessions.id          级联删除
--   long_memory_sources → long_memories.id / short_memories.id，记录整合来源
--   long_memory_embeddings → long_memories.id / sessions.id，当前模型的可重建向量
--   notebooks           session_id    → sessions.id          级联删除（主键，一会话一行）
--   session_festivals   session_id    → sessions.id          级联删除
--                        festival_id   → festival_catalog.id  置空
--   travels             session_id    → sessions.id          级联删除
--   travel_journals     session_id / travel_id → sessions.id / travels.id
--   accessories         session_id    → sessions.id          级联删除
--                        travel_id     → travels.id          置空
--   letters             session_id    → sessions.id          级联删除
--                        travel_id     → travels.id          置空
--                        reply_to_id   → letters.id           置空（自引用，一封信只回一次）
--   souvenirs           session_id    → sessions.id          级联删除
--                        travel_id     → travels.id          置空
--
-- 级联行为约定：
--   删除人设   → 删除它名下所有 sessions 及全部会话级数据
--   删除会话   → 删除该会话的回合 / messages / 记忆 / 记事本 / 节日子表 / 饰品 / 旅行 / 游记 / 信件 / 伴手礼
--   删除用户设定 → 会话保留，只把 user_profile_id 置空（前端显示“未绑定”）
--   删除旅行记录 → 对应旅行游记级联删除；饰品 / 信件 / 伴手礼保留，travel_id 置空
--   节日母表   → 系统内置数据，不随任何业务删除；子表靠 festival_id 引用它
-- ============================================================

-- ------------------------------------------------------------
-- 1. API 提供商
-- 官方 OpenAI 与自定义 OpenAI 兼容接口统一用本表管理。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS providers (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    name              TEXT    NOT NULL,                    -- 显示名称，用户自定义
    base_url          TEXT,                                -- 自定义接口地址；官方 OpenAI 为 NULL
    api_key_encrypted TEXT,                                -- DPAPI Base64；未配置或导入后 NULL，不存明文
    model             TEXT    NOT NULL,                    -- 模型名，如 gpt-4o-mini / deepseek-chat
    is_active         INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),          -- 是否为聊天当前提供商（全局唯一为 1）
    memory_is_active  INTEGER NOT NULL DEFAULT 0 CHECK (memory_is_active IN (0, 1))           -- 是否为记忆整合专用提供商；全表无启用项时跟随聊天提供商
);

-- Embedding 提供商与聊天/记忆整合提供商分开管理。
-- 首次启动写入并启用一个不可删除的内置本地项：BAAI/bge-small-zh-v1.5。
-- online 项调用 OpenAI-compatible /embeddings 接口；API Key 同样使用 DPAPI 加密。
CREATE TABLE IF NOT EXISTS embedding_providers (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT    NOT NULL,
    kind              TEXT    NOT NULL CHECK (kind IN ('local', 'online')),
    base_url          TEXT,
    api_key_encrypted TEXT,
    model             TEXT    NOT NULL,
    is_builtin        INTEGER NOT NULL DEFAULT 0 CHECK (is_builtin IN (0, 1)),
    is_active         INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),
    CHECK (
        (kind = 'local' AND base_url IS NULL AND api_key_encrypted IS NULL)
        OR
        (kind = 'online' AND base_url IS NOT NULL)
    )
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
    self_evolve_enabled INTEGER NOT NULL DEFAULT 1 CHECK (self_evolve_enabled IN (0, 1)),          -- 人设自调开关：1 开启，0 关闭
    last_evolved_at     TEXT,                                -- 最近一次 AI 调整时间 RFC3339，可空
    color               TEXT    NOT NULL DEFAULT '#ffb6c1',  -- 桌宠本体颜色（十六进制），按人设绑定
    chat_bg_file        TEXT,                                -- 聊天窗背景图相对文件名，可空；图片保存在 data/chat_background/
    chat_bg_opacity     INTEGER NOT NULL DEFAULT 85 CHECK (chat_bg_opacity BETWEEN 0 AND 100),         -- 聊天窗背景透明度百分比 0–100
    chat_bg_blur        INTEGER NOT NULL DEFAULT 0 CHECK (chat_bg_blur BETWEEN 0 AND 40),          -- 聊天窗背景模糊像素 0–40
    is_active           INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),          -- 当前使用的人设（全局唯一为 1）
    expressions         TEXT                                 -- 分类表情库 JSON；未生成时为 NULL
);

-- ------------------------------------------------------------
-- 2.1 用户设定
-- 用户自己写的个人设定/背景，可创建多个；会话可选绑定一个，未绑定则为空。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS user_profiles (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    name       TEXT    NOT NULL,                    -- 用户设定名称
    content    TEXT    NOT NULL,                    -- 用户设定正文
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    updated_at TEXT    NOT NULL                     -- 最近更新时间 RFC3339
);
-- ------------------------------------------------------------
-- 3. 会话
-- 每个会话归属一个人设；回合、消息、记忆、记事本、节日子表、饰品、旅行、游记、信件、伴手礼都按会话隔离。
-- 旅行相关字段记录当前会话的在线出行计时，只在应用开启、该会话为当前会话且桌宠在家空闲时累计。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sessions (
    id                            INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    persona_id                    INTEGER NOT NULL,                    -- 所属人设 -> personas.id
    user_profile_id               INTEGER,                             -- 绑定的用户设定 -> user_profiles.id，可空
    title                         TEXT    NOT NULL,                    -- 会话标题，可 AI 生成、用户重命名
    created_at                    TEXT    NOT NULL,                    -- 创建时间 RFC3339
    updated_at                    TEXT    NOT NULL,                    -- 最近更新时间 RFC3339
    memory_revision               INTEGER NOT NULL DEFAULT 0 CHECK (memory_revision >= 0), -- 消息/记忆变更版本，事务提交前校验
    craft_tier_1_used              INTEGER NOT NULL DEFAULT 0 CHECK (craft_tier_1_used IN (0, 1)), -- 第一档是否已制作
    craft_tier_2_used              INTEGER NOT NULL DEFAULT 0 CHECK (craft_tier_2_used IN (0, 1)), -- 第二档是否已制作
    craft_tier_3_used              INTEGER NOT NULL DEFAULT 0 CHECK (craft_tier_3_used IN (0, 1)), -- 第三档是否已制作
    intimacy                      INTEGER NOT NULL DEFAULT 0 CHECK (intimacy >= 0),          -- 当前会话亲密度
    last_return_at                TEXT,                                -- 最近一次旅行回来时间 RFC3339，可空
    active_seconds_since_return   INTEGER NOT NULL DEFAULT 0 CHECK (active_seconds_since_return >= 0),          -- 本次回来后累计的在线秒数
    next_travel_after_seconds     INTEGER CHECK (next_travel_after_seconds > 0),                             -- 下次可出行的在线秒数阈值，可空
    FOREIGN KEY (persona_id) REFERENCES personas(id) ON DELETE CASCADE,
    FOREIGN KEY (user_profile_id) REFERENCES user_profiles(id) ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 4. 聊天回合与消息
-- 一轮由一条用户消息和一条 AI 回复组成。主动消息 round_id 为 NULL，不计入回合或短期记忆。
-- pending：只有用户消息；completed：AI 正常回复完成；interrupted：回复被中断，不生成短期记忆。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS chat_rounds (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   INTEGER NOT NULL,
    status       TEXT    NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'interrupted')),
    created_at   TEXT    NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    round_id   INTEGER,                             -- 所属完整聊天轮次；主动消息为 NULL
    role       TEXT    NOT NULL CHECK (role IN ('user', 'assistant')),                    -- user / assistant
    content    TEXT    NOT NULL,                    -- 消息正文
    expression TEXT,                                -- AI 回复时使用的颜文字；用户消息为 NULL
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (round_id) REFERENCES chat_rounds(id) ON DELETE CASCADE,
    UNIQUE (round_id, role)
);

-- ------------------------------------------------------------
-- 5. 设置（键值对）
-- 常用键：
--   hungry_minutes                    饥饿触发分钟数
--   bored_minutes                     无聊触发分钟数
--   sleep_min_minutes                 睡眠最短分钟数，默认 60
--   sleep_max_minutes                 睡眠最长分钟数，默认 120
--   short_memory_min_chars            短期记忆最小字数
--   short_memory_max_chars            短期记忆最大字数
--   long_memory_min_chars             长期记忆最小字数
--   long_memory_max_chars             长期记忆最大字数
--   memory_merge_count                 生成一条长期记忆使用的短期记忆数 k，默认 20
--   recent_message_count               进入上下文的最近原文消息数，默认 30
--   recent_long_memory_count           直接进入上下文的最近长期记忆数 m，默认 30
--   travel_journal_min_chars           旅行游记最小字数
--   travel_journal_max_chars           旅行游记最大字数
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
--   current_session_id                当前会话 ID；启动校验，导入重映射
--   window_pos_x                      桌宠窗上次位置 X，启动时恢复；默认屏幕右下角
--   window_pos_y                      桌宠窗上次位置 Y，启动时恢复；默认屏幕右下角
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,   -- 设置键
    value TEXT NOT NULL       -- 设置值，统一存字符串
);

-- ------------------------------------------------------------
-- 6. 短期记忆
-- 每个 completed 回合生成且只生成一条；整合后仍永久保留。
-- integration_state：pending=尚未整合成长期记忆，integrated=已经整合。
-- 短期记忆和长期记忆是中性事实记录，不使用人设口吻，不需要任何氛围总结或主观评价，必须只是如同剧本一样的叙事描述。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS short_memories (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id        INTEGER NOT NULL,
    round_id          INTEGER NOT NULL UNIQUE,
    content           TEXT    NOT NULL,
    integration_state TEXT    NOT NULL DEFAULT 'pending' CHECK (integration_state IN ('pending', 'integrated')),
    created_at        TEXT    NOT NULL,
    updated_at        TEXT    NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (round_id) REFERENCES chat_rounds(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 7. 长期记忆
-- 当第 k+1 条短期记忆出现时，由此前连续 k 条 pending 短期记忆生成。
-- covered_through_message_id 指向第 k 轮 AI 消息；activated_by_round_id 指向触发整合的第 k+1 轮。
-- 标题与正文同次生成，均为中性事实记录，并共同参与混合检索。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS long_memories (
    id                         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id                 INTEGER NOT NULL,
    title                      TEXT    NOT NULL CHECK (length(trim(title)) > 0),
    content                    TEXT    NOT NULL,
    source_count               INTEGER NOT NULL CHECK (source_count > 0),
    covered_through_message_id INTEGER NOT NULL,
    activated_by_round_id      INTEGER NOT NULL UNIQUE,
    created_at                 TEXT    NOT NULL,
    updated_at                 TEXT    NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (covered_through_message_id) REFERENCES messages(id) ON DELETE RESTRICT,
    FOREIGN KEY (activated_by_round_id) REFERENCES chat_rounds(id) ON DELETE CASCADE
);

-- 长期记忆全文索引。trigram 支持中文连续字符的子串匹配；不足三个字符的查询由 Rust 层补充普通匹配。
-- 此表是可重建数据，逻辑备份不导出；可执行 INSERT INTO long_memories_fts(long_memories_fts) VALUES('rebuild') 重建。
CREATE VIRTUAL TABLE IF NOT EXISTS long_memories_fts USING fts5(
    title,
    content,
    session_id UNINDEXED,
    content = 'long_memories',
    content_rowid = 'id',
    tokenize = 'trigram'
);

CREATE TRIGGER IF NOT EXISTS long_memories_fts_insert AFTER INSERT ON long_memories BEGIN
    INSERT INTO long_memories_fts(rowid, title, content, session_id)
    VALUES (new.id, new.title, new.content, new.session_id);
END;

CREATE TRIGGER IF NOT EXISTS long_memories_fts_delete AFTER DELETE ON long_memories BEGIN
    INSERT INTO long_memories_fts(long_memories_fts, rowid, title, content, session_id)
    VALUES ('delete', old.id, old.title, old.content, old.session_id);
END;

CREATE TRIGGER IF NOT EXISTS long_memories_fts_update AFTER UPDATE OF title, content, session_id ON long_memories BEGIN
    INSERT INTO long_memories_fts(long_memories_fts, rowid, title, content, session_id)
    VALUES ('delete', old.id, old.title, old.content, old.session_id);
    INSERT INTO long_memories_fts(rowid, title, content, session_id)
    VALUES (new.id, new.title, new.content, new.session_id);
END;

-- 当前激活 Embedding 模型为每条长期记忆保存一份向量缓存。
-- vector 使用归一化 float32 BLOB；model_key 标识 provider 类型、地址、模型及维度组合。
-- 向量是可重建数据，不进入导出包；标题或正文改变、模型切换或导入后重新生成。
CREATE TABLE IF NOT EXISTS long_memory_embeddings (
    long_memory_id INTEGER PRIMARY KEY,
    session_id     INTEGER NOT NULL,
    model_key      TEXT    NOT NULL,
    dimensions     INTEGER NOT NULL CHECK (dimensions > 0),
    vector         BLOB    NOT NULL,
    content_hash   TEXT    NOT NULL,
    updated_at     TEXT    NOT NULL,
    FOREIGN KEY (long_memory_id) REFERENCES long_memories(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS long_memory_sources (
    long_memory_id  INTEGER NOT NULL,
    short_memory_id INTEGER NOT NULL UNIQUE,
    source_order    INTEGER NOT NULL CHECK (source_order > 0),
    PRIMARY KEY (long_memory_id, short_memory_id),
    UNIQUE (long_memory_id, source_order),
    FOREIGN KEY (long_memory_id) REFERENCES long_memories(id) ON DELETE CASCADE,
    FOREIGN KEY (short_memory_id) REFERENCES short_memories(id) ON DELETE RESTRICT
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
-- fire_at 保存下一次待入队触发时间；同事务建立 occurrence 后才推进/置空。
-- 重复提醒 status 始终为 pending；完成/过期仅更新本次 occurrence，永久停止须删除 reminders。
-- 一次性提醒完成/过期时同步更新本表；原到期时间由 occurrence.scheduled_at 永久保留。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS reminders (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    kind       TEXT    NOT NULL CHECK (kind IN ('once', 'daily', 'weekly', 'monthly')),                    -- once / daily / weekly / monthly
    schedule   TEXT,                                -- 重复规则 JSON；一次性为 NULL
    content    TEXT    NOT NULL,                    -- 提醒内容
    status     TEXT    NOT NULL CHECK (status IN ('pending', 'done', 'expired')),                    -- pending / done / expired
    fire_at    TEXT,                                -- 下次触发时间 RFC3339
    source     TEXT    NOT NULL CHECK (source IN ('chat', 'manual')), -- 来源
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    CHECK ((kind = 'once' AND schedule IS NULL) OR (kind <> 'once' AND schedule IS NOT NULL AND status = 'pending'))
);

-- 每次提醒触发记录；与调度游标更新在同一事务内写入。
-- awaiting_choice：关闭期间错过的一次性提醒，等待用户补提醒/标过期。
-- queued：等待前端领取；shown：前端已确认展示，未完成记录仍可在列表处理。
CREATE TABLE IF NOT EXISTS reminder_occurrences (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    reminder_id    INTEGER NOT NULL,
    scheduled_at   TEXT NOT NULL,                 -- 本次原定触发时间 UTC RFC3339
    status         TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'done', 'expired')),
    delivery_state TEXT NOT NULL CHECK (delivery_state IN ('awaiting_choice', 'queued', 'shown')),
    completed_at   TEXT,
    created_at     TEXT NOT NULL,
    UNIQUE (reminder_id, scheduled_at),
    FOREIGN KEY (reminder_id) REFERENCES reminders(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 10. 用户全局状态
-- 单行表，主键固定为 1；喵币是全局用户级数据，不随会话删除。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS user_state (
    id         INTEGER PRIMARY KEY CHECK (id = 1),  -- 固定单行，主键为 1
    coins      INTEGER NOT NULL DEFAULT 0 CHECK (coins >= 0), -- 当前喵币余额
    updated_at TEXT    NOT NULL                     -- 最近更新时间 RFC3339
);

-- ------------------------------------------------------------
-- 11. 喵币账单
-- 所有喵币变动都记一条流水，用于余额核对、每日签到/游戏次数判断。
-- reason: daily_sign_in / sign_in_festival / game_win / game_loss / game_draw / accessory_craft / accessory_sell
-- ref_id：制作/卖出对应 accessories.id（删除后仅作历史引用）；签到/游戏为 NULL。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS currency_ledger (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    delta      INTEGER NOT NULL,                    -- 变动数量，可为负数
    reason     TEXT    NOT NULL CHECK (reason IN ('daily_sign_in', 'sign_in_festival', 'game_win', 'game_loss', 'game_draw', 'accessory_craft', 'accessory_sell')), -- 变动原因
    ref_id     INTEGER,                             -- 关联业务 id，可空
    created_at TEXT    NOT NULL                     -- 创建时间 RFC3339
);

-- ------------------------------------------------------------
-- 12. 旅行
-- status: away=出门中，returned=正常返回，cancelled=切换会话取消。
-- 各生成状态独立持久化；删除已生成结果不重置 done，避免重新生成。
-- midway_letter_at 出发时确定；gift_kind 在正常返回事务中抽取并固定。
-- planned_return_at 用于判断关闭应用后是否已经到返回时间。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travels (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id        INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    status            TEXT    NOT NULL CHECK (status IN ('away', 'returned', 'cancelled')), -- 旅行状态
    started_at        TEXT    NOT NULL,                    -- 出发时间 RFC3339
    planned_return_at TEXT    NOT NULL,                    -- 计划返回时间 RFC3339
    actual_return_at  TEXT,                                -- 正常返回时间；离线到期使用计划返回时间
    cancelled_at      TEXT,                                -- 切换会话取消时间；取消不视为正常返回
    midway_letter_at  TEXT NOT NULL,                       -- 出发时确定为行程中点
    midway_letter_state TEXT NOT NULL DEFAULT 'pending' CHECK (midway_letter_state IN ('pending', 'done', 'cancelled')),
    return_letter_state TEXT NOT NULL DEFAULT 'pending' CHECK (return_letter_state IN ('pending', 'done', 'cancelled')),
    gift_state        TEXT NOT NULL DEFAULT 'pending' CHECK (gift_state IN ('pending', 'done', 'cancelled')),
    journal_state     TEXT NOT NULL DEFAULT 'pending' CHECK (journal_state IN ('pending', 'done', 'cancelled')),
    gift_kind         TEXT CHECK (gift_kind IN ('text', 'accessory')), -- 正常返回时确定一次
    created_at        TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 13. 旅行游记
-- 每次正常返回的旅行一篇；按人设口吻详细总结整次旅行，不参与短期或长期记忆整合。
-- 标题用一句话极简概括何时去了哪里、做了什么、带回什么伴手礼、去了多久。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travel_journals (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL,
    travel_id  INTEGER NOT NULL UNIQUE,
    title      TEXT    NOT NULL,
    content    TEXT    NOT NULL,
    created_at TEXT    NOT NULL,
    updated_at TEXT    NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id) REFERENCES travels(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 14. 饰品
-- 每个饰品属于一个会话；source: crafted=制作，travel=旅行带回。
-- v1 只有头顶装饰槽；is_worn=1 表示当前佩戴。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS accessories (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id   INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    name         TEXT    NOT NULL,                    -- 饰品名称
    display_text TEXT    NOT NULL,                    -- 头顶显示的文字/emoji/颜文字
    description  TEXT    NOT NULL,                    -- AI 按人设口吻写的简介
    cost         INTEGER NOT NULL DEFAULT 0 CHECK (cost >= 0),          -- 制作费用；旅行带回为 0
    source       TEXT    NOT NULL CHECK (source IN ('crafted', 'travel')), -- 来源
    travel_id    INTEGER,                             -- 旅行带回时为 travels.id，制作为 NULL
    is_worn      INTEGER NOT NULL DEFAULT 0 CHECK (is_worn IN (0, 1)),          -- 是否佩戴：1 佩戴，0 未佩戴
    created_at   TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)  REFERENCES travels(id)   ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 15. 信件
-- sender: user=用户写的信，ai=AI 写的信；AI 生成的信件内容应包含可爱颜文字。
-- reply_to_id 非空时表示这是对某封信的回复；每封信最多回复一次。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS letters (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id   INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    travel_id    INTEGER,                             -- 关联旅行 -> travels.id，可空
    sender       TEXT    NOT NULL CHECK (sender IN ('user', 'ai')),                    -- user / ai
    content      TEXT    NOT NULL,                    -- 信件正文
    reply_to_id  INTEGER,                             -- 回复目标；删除目标后置空
    has_been_replied INTEGER NOT NULL DEFAULT 0 CHECK (has_been_replied IN (0, 1)), -- 已成功回复过，删回复不重置
    reply_due_at TEXT,                                -- 用户信自动回信时间，首次发送时抽取并保存
    reply_state  TEXT NOT NULL DEFAULT 'none' CHECK (reply_state IN ('none', 'pending', 'completed', 'cancelled')), -- AI 信为 none
    letter_kind  TEXT NOT NULL CHECK (letter_kind IN ('user', 'midway', 'return', 'reply')), -- 区分旅行信与回信
    is_favorite  INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),          -- 是否收藏：1 收藏，0 未收藏
    is_read      INTEGER NOT NULL DEFAULT 0 CHECK (is_read IN (0, 1)),          -- 用户是否已读：1 已读，0 未读
    created_at   TEXT    NOT NULL,                    -- 写信/回信时间 RFC3339
    CHECK ((sender = 'user' AND letter_kind = 'user' AND reply_state <> 'none' AND reply_due_at IS NOT NULL) OR (sender = 'ai' AND letter_kind <> 'user' AND reply_state = 'none' AND reply_due_at IS NULL)),
    FOREIGN KEY (session_id)  REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)   REFERENCES travels(id)  ON DELETE SET NULL,
    FOREIGN KEY (reply_to_id) REFERENCES letters(id)  ON DELETE SET NULL
);

-- 每封信只能被回复一次：reply_to_id 非空时唯一。
CREATE UNIQUE INDEX IF NOT EXISTS idx_letters_reply_to_unique
    ON letters(reply_to_id)
    WHERE reply_to_id IS NOT NULL;

-- ------------------------------------------------------------
-- 16. 伴手礼
-- 只保存普通文字礼物；旅行带回的可佩戴饰品存 accessories 表。
-- 不做背包、数量、容量或使用机制。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS souvenirs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id  INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    travel_id   INTEGER,                             -- 来源旅行 -> travels.id，可空
    content     TEXT    NOT NULL,                    -- 文字礼物内容
    note        TEXT    NOT NULL,                    -- AI 按人设口吻写的备注
    is_favorite INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),          -- 是否收藏：1 收藏，0 未收藏
    created_at  TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)  REFERENCES travels(id)  ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 17. 节日母表（系统级内置）
-- 首次启动由项目内置静态数据写入，并直接查内置农历表把当年的公历月日填好。
-- 不联网、不调用任何日历接口、失败不进休眠。
-- calendar_type：solar=公历节日，lunar=农历节日，solar_term=节气（清明、冬至）。
-- 公历节日月日不变，每年只更新 year；
-- 农历节日与节气每年查内置农历/节气表推出公历月日后写回，行本身不删除。
-- 除夕这类“腊月最后一天”的节日，农历日（廿九或三十）同样每年查内置农历表推出后写回。
-- code 是稳定标识，用于首次启动幂等写入；子表通过 festival_id 关联本表。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS festival_catalog (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    code          TEXT    NOT NULL UNIQUE,             -- 稳定标识，如 solar_new_year / lunar_spring_festival
    name          TEXT    NOT NULL,                    -- 节日名
    year          INTEGER NOT NULL,                    -- 本年公历年份：日期已按哪一年查表更新，不是生日出生年
    calendar_type TEXT    NOT NULL CHECK (calendar_type IN ('solar', 'lunar', 'solar_term')), -- 历法类型
    solar_month   INTEGER,                             -- 公历月；农历节日与节气按内置表推出本年月日后写入
    solar_day     INTEGER,                             -- 公历日；农历节日与节气按内置表推出本年月日后写入
    lunar_month   INTEGER,                             -- 农历月；仅农历节日填写，公历节日与节气为 NULL
    lunar_day     INTEGER,                             -- 农历日；仅农历节日填写，公历节日与节气为 NULL
    is_leap_month INTEGER NOT NULL DEFAULT 0 CHECK (is_leap_month IN (0, 1)),          -- 农历是否闰月
    solar_term    TEXT,                                -- 节气名，如 清明 / 冬至；非节气为 NULL
    blessing      TEXT    NOT NULL,                    -- 内置祝福词，签到彩蛋直接展示，不调用 AI
    is_memorial   INTEGER NOT NULL DEFAULT 0 CHECK (is_memorial IN (0, 1)),          -- 纪念性节日（如清明）：1 时文案不使用“快乐”
    note          TEXT                                 -- 备注
);

-- ------------------------------------------------------------
-- 18. 会话节日子表（会话级）
-- 由 AI 或用户维护的节日、生日、纪念日、自定义日子。
-- 与母表同样只存公历月日与农历月日；农历行每年查内置表推出公历月日后更新。
-- festival_id 可空：来源于母表时填写，AI/用户自定义时为空。
-- 行不随年份删除；AI 可删除自己不需要的行。
-- 子表与母表一起参与签到、日常聊天和无聊主动消息上下文。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS session_festivals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id    INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    festival_id   INTEGER,                             -- 关联母表 -> festival_catalog.id，可空
    kind          TEXT    NOT NULL CHECK (kind IN ('festival', 'birthday', 'anniversary', 'custom')), -- 类型
    name          TEXT    NOT NULL,                    -- 名称
    year          INTEGER NOT NULL,                    -- 本年公历年份：日期已按哪一年查表更新，不是生日出生年
    calendar_type TEXT    NOT NULL CHECK (calendar_type IN ('solar', 'lunar')), -- 历法类型
    solar_month   INTEGER,                             -- 公历月；农历日子按内置表推出本年月日后写入
    solar_day     INTEGER,                             -- 公历日；农历日子按内置表推出本年月日后写入
    lunar_month   INTEGER,                             -- 农历月；仅农历日子填写，公历日子为 NULL
    lunar_day     INTEGER,                             -- 农历日；仅农历日子填写，公历日子为 NULL
    is_leap_month INTEGER NOT NULL DEFAULT 0 CHECK (is_leap_month IN (0, 1)),          -- 农历是否闰月
    blessing      TEXT NOT NULL CHECK (length(trim(blessing)) > 0), -- 祝福词：AI 新增时必须填写，手动新增也必填
    note          TEXT,                                -- 备注
    source        TEXT    NOT NULL CHECK (source IN ('ai', 'user')), -- 来源
    created_at    TEXT    NOT NULL,                    -- 创建时间 RFC3339
    updated_at    TEXT    NOT NULL,                    -- 最近更新时间 RFC3339
    FOREIGN KEY (session_id)  REFERENCES sessions(id)         ON DELETE CASCADE,
    FOREIGN KEY (festival_id) REFERENCES festival_catalog(id) ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 常用索引（加快按会话查询）
-- ------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_messages_session        ON messages (session_id);
CREATE INDEX IF NOT EXISTS idx_chat_rounds_session      ON chat_rounds (session_id, id);
CREATE INDEX IF NOT EXISTS idx_sessions_persona        ON sessions (persona_id);
CREATE INDEX IF NOT EXISTS idx_sessions_user_profile   ON sessions (user_profile_id);
CREATE INDEX IF NOT EXISTS idx_short_memories_session  ON short_memories (session_id);
CREATE INDEX IF NOT EXISTS idx_long_memories_session   ON long_memories (session_id);
CREATE INDEX IF NOT EXISTS idx_long_memory_embeddings_session ON long_memory_embeddings (session_id, long_memory_id);
CREATE INDEX IF NOT EXISTS idx_reminders_status_fire   ON reminders (status, fire_at);
CREATE INDEX IF NOT EXISTS idx_currency_ledger_created ON currency_ledger (created_at);
CREATE INDEX IF NOT EXISTS idx_accessories_session     ON accessories (session_id);
CREATE INDEX IF NOT EXISTS idx_travels_session_status  ON travels (session_id, status);
CREATE INDEX IF NOT EXISTS idx_travel_journals_session ON travel_journals (session_id, id);
CREATE INDEX IF NOT EXISTS idx_letters_session         ON letters (session_id);
CREATE INDEX IF NOT EXISTS idx_souvenirs_session       ON souvenirs (session_id);
CREATE INDEX IF NOT EXISTS idx_session_festivals_session ON session_festivals (session_id);

-- 全局当前项最多一个；允许无提供商或无显式记忆提供商。
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_one_active ON providers(is_active) WHERE is_active = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_one_memory_active ON providers(memory_is_active) WHERE memory_is_active = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_embedding_providers_one_active ON embedding_providers(is_active) WHERE is_active = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_personas_one_active ON personas(is_active) WHERE is_active = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accessories_one_worn ON accessories(session_id) WHERE is_worn = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_travels_one_away ON travels(session_id) WHERE status = 'away';
CREATE INDEX IF NOT EXISTS idx_reminder_occurrences_pending ON reminder_occurrences(status, delivery_state);
CREATE INDEX IF NOT EXISTS idx_short_memories_state ON short_memories(session_id, integration_state, id);
CREATE INDEX IF NOT EXISTS idx_long_memory_sources_long ON long_memory_sources(long_memory_id, source_order);

-- 跨表规则由 Rust 命令在事务中检查：round/message/记忆来源、旅行/游记/信件/礼物/饰品必须属于同一会话。
-- 切换当前项先验证目标存在，再在同一事务内清除旧项并启用新项。
-- 余额检查、签到/游戏次数检查、饰品增删、档位使用标记和流水写入必须共用同一事务。
-- settings 在 Rust 层逐键校验：非负值、最小值 <= 最大值、亲密度档位严格递增。
-- 日期、重复规则 JSON、颜色及饰品字符限制在 Rust 层验证；前端校验不替代后端校验。

CREATE INDEX IF NOT EXISTS idx_letters_reply_pending ON letters(reply_state, reply_due_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_letters_travel_kind ON letters(travel_id, letter_kind)
    WHERE travel_id IS NOT NULL AND letter_kind IN ('midway', 'return');
-- journal_state 是旅行游记生成是否完成的永久依据；用户编辑游记不改变 done，取消旅行不生成游记。
