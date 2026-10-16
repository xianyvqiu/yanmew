-- ============================================================
-- 颜喵 YanMew 数据库结构（SQLite）
-- 运行库文件：可执行文件所在目录的 data/yanwenzi.db（便携版 data 子目录）
-- 安装版运行库：当前用户 LocalAppData/YanMew/data/yanwenzi.db；由平台路径适配选择，结构与便携版相同。
-- 本文件是最新完整建库脚本，仅描述结构，不包含运行期数据。
-- 新库由 Rust 在事务中执行本文件；旧库通过版本化迁移升级，不靠 IF NOT EXISTS 更新旧表。
-- 当前包含已确定的完整 V1 业务表（含小游戏插件）；语音表在后续扩展阶段加入。
-- 所有时间统一存 UTC RFC3339（固定精度）；本地日期判断在 Rust 层转换。
-- ============================================================

-- 本项目启用并依赖 SQLite 外键约束。
-- 由于 SQLite 每个新连接默认可能未开启外键检查，
-- Rust 数据库层必须在建立每个连接后、开启任何事务前执行：
-- PRAGMA foreign_keys = ON;
-- 并读取 PRAGMA foreign_keys 确认返回 1，失败则拒绝继续使用该连接。
-- 会话级数据使用 ON DELETE CASCADE 做数据库级级联删除。

-- ============================================================
-- 表关系总览
--
-- 全局级（不属于任何会话）：
--   providers          独立表，无外键；聊天与记忆整合各选一个启用项
--   embedding_providers 独立表；默认启用随应用打包的本地模型，也可选联网接口
--   personas           顶层容器：人设 → 会话 → 会话级数据；唯一内置默认人设不可删除
--   user_profiles      独立表；被 sessions 引用，删除时置空而非删会话
--   settings           键值对，全局配置
--   user_state         单行表，全局喵币余额
--   game_plugins       当前插件登记；内置石头剪刀布与本地安装插件共用
--   game_permissions   → game_plugins.plugin_id，当前用户授权状态；独立插件数据导入后为待确认
--   game_plugin_settings → game_plugins.plugin_id，插件设置
--   game_plugin_data   → game_plugins.plugin_id，插件全局私有存档
--   game_runs          历史运行记录，保存插件身份快照，不绑定会话
--   currency_ledger    喵币流水，与 user_state 逻辑配套；插件流水引用历史运行记录
--   festival_catalog   系统内置节日母表，独立，不随任何业务数据删除
--   reminders          全局提醒规则
--   reminder_occurrences → reminders.id，删除规则级联删除每次触发记录
--   reminder_email_deliveries 全局本机邮件任务，不进入备份
--   reminder_email_items → 邮件任务与触发记录，两个外键均级联删除，occurrence_id唯一
--
-- 会话级（都由 sessions 派生，删除会话时级联删除）：
--   sessions           主表
--                        persona_id      → personas.id        级联删除
--                        user_profile_id → user_profiles.id  置空
--   chat_rounds         session_id    → sessions.id          级联删除
--   session_pet_states session_id    → sessions.id          一对一，级联删除；有效计时与睡眠恢复
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
-- 历史记录（不随插件卸载删除）：
--   game_runs           保存插件 ID / 名称 / 版本快照
--   currency_ledger     保存插件 ID / 版本 / 请求 ID；已记账运行记录由外键限制删除
--
-- 级联行为约定：
--   删除非内置人设 → 删除它名下所有 sessions 及全部会话级数据；内置默认人设由 Rust 禁止删除
--   删除会话   → 删除该会话的回合 / messages / 记忆 / 记事本 / 节日子表 / 饰品 / 旅行 / 游记 / 信件 / 伴手礼
--   删除用户设定 → 会话保留，只把 user_profile_id 置空（前端显示“未绑定”）
--   删除旅行记录 → 对应旅行游记级联删除；饰品 / 信件 / 伴手礼保留，travel_id 置空
--   删除插件登记 → 删除它的当前授权、设置和私有存档；历史运行记录与喵币流水保留
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
    created_at TEXT, -- 创建UTC时间；升级前未知时间保留NULL
    updated_at TEXT -- 最近编辑UTC时间；升级前未知时间保留NULL
);

-- Embedding 提供商与聊天/记忆整合提供商分开管理。
-- 首次启动写入并启用一个不可删除的内置本地项：BAAI/bge-small-zh-v1.5。
-- online 项调用 OpenAI-compatible /embeddings 接口；API Key 同样使用 DPAPI 加密。
CREATE TABLE IF NOT EXISTS embedding_providers (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,   -- Embedding 提供商自增主键
    name              TEXT    NOT NULL,                    -- 设置页显示名称
    kind              TEXT    NOT NULL CHECK (kind IN ('local', 'online')), -- local=本地模型，online=联网 Embedding API
    base_url          TEXT,                                -- 联网 API 地址；本地项为 NULL
    api_key_encrypted TEXT,                                -- 联网 API Key 的 DPAPI Base64 密文；本地项或无密钥时为 NULL
    model             TEXT    NOT NULL,                    -- 模型标识；内置项默认 BAAI/bge-small-zh-v1.5
    is_builtin        INTEGER NOT NULL DEFAULT 0 CHECK (is_builtin IN (0, 1)), -- 1=随应用提供且不可删除，0=用户配置
    is_active         INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)), -- 1=当前使用项，0=未使用；全局最多一个 1
    CHECK (is_builtin = 0 OR kind = 'local'),
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
-- is_builtin 标记系统自带默认人设“电子小猫”；只允许一条，由 Rust 禁止删除。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS personas (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    name                TEXT    NOT NULL,                    -- 人设名称
    prompt              TEXT    NOT NULL,                    -- 当前人设 prompt
    initial_prompt      TEXT    NOT NULL,                    -- 用户手动保存的初始设定
    self_evolve_enabled INTEGER NOT NULL DEFAULT 1 CHECK (self_evolve_enabled IN (0, 1)),          -- 人设自调开关：1 开启，0 关闭
    last_evolved_at     TEXT,                                -- 最近一次 AI 调整时间 RFC3339，可空
    color               TEXT    NOT NULL DEFAULT '#ffb6c1',  -- 桌宠本体颜色（十六进制），按人设绑定
    chat_bg_file        TEXT,                                -- 人设级聊天背景相对文件名，可空；受管 jpg/png/webp ≤10MiB，位于 AppPaths 的 chat_background/，新文件落盘后才提交引用
    chat_bg_opacity     INTEGER NOT NULL DEFAULT 85 CHECK (chat_bg_opacity BETWEEN 0 AND 100),         -- 聊天窗背景透明度百分比 0–100
    chat_bg_blur        INTEGER NOT NULL DEFAULT 0 CHECK (chat_bg_blur BETWEEN 0 AND 40),          -- 聊天窗背景模糊像素 0–40
    is_active           INTEGER NOT NULL DEFAULT 0 CHECK (is_active IN (0, 1)),          -- 1=当前使用人设（全局唯一），0=非当前人设
    is_builtin          INTEGER NOT NULL DEFAULT 0 CHECK (is_builtin IN (0, 1)),         -- 1=系统内置默认人设且不可删除，0=用户人设
    expressions         TEXT,                                -- 分类表情库 JSON；未生成时为 NULL
    created_at TEXT, -- 创建UTC时间；升级前未知时间保留NULL
    updated_at TEXT, -- 最近编辑UTC时间；升级前未知时间保留NULL
    chat_bubble_style TEXT NOT NULL DEFAULT 'solid' CHECK (chat_bubble_style IN ('solid','transparent','outline','frosted')), -- 人设级消息气泡：solid默认；transparent透明；outline仅描边；frosted毛玻璃。
    chat_user_text_color TEXT CHECK (chat_user_text_color IS NULL OR (length(chat_user_text_color)=7 AND substr(chat_user_text_color,1,1)='#' AND substr(chat_user_text_color,2) NOT GLOB '*[^0-9A-Fa-f]*')), -- 用户正文颜色，#RRGGBB；NULL跟随主题。
    chat_assistant_text_color TEXT CHECK (chat_assistant_text_color IS NULL OR (length(chat_assistant_text_color)=7 AND substr(chat_assistant_text_color,1,1)='#' AND substr(chat_assistant_text_color,2) NOT GLOB '*[^0-9A-Fa-f]*')), -- AI正文颜色，#RRGGBB；NULL跟随主题。
    pet_image_enabled INTEGER NOT NULL DEFAULT 0 CHECK(pet_image_enabled IN (0,1)), -- 0颜文字/1图片模式，开启前要求正式初始图
    pet_image_revision INTEGER NOT NULL DEFAULT 0 CHECK(pet_image_revision>=0) -- 创作库版本，淘汰旧请求
);

-- ------------------------------------------------------------
-- 2.1 用户设定
-- 用户自己写的个人设定/背景，可创建多个；会话可选绑定一个，未绑定则为空。
-- 全局共享资料；名称和正文 trim 后非空，手动正文不受 AI 生成字数范围限制。
-- 编辑/删除正在使用的资料或修改绑定，Rust 先中断受影响回复再保存；删除仅 SET NULL，不删除聊天或记忆。
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
    craft_tier_1_used              INTEGER NOT NULL DEFAULT 0 CHECK (craft_tier_1_used IN (0, 1)), -- 1=第一档制作机会已使用，0=尚未使用
    craft_tier_2_used              INTEGER NOT NULL DEFAULT 0 CHECK (craft_tier_2_used IN (0, 1)), -- 1=第二档制作机会已使用，0=尚未使用
    craft_tier_3_used              INTEGER NOT NULL DEFAULT 0 CHECK (craft_tier_3_used IN (0, 1)), -- 1=第三档制作机会已使用，0=尚未使用
    intimacy                      INTEGER NOT NULL DEFAULT 0 CHECK (intimacy >= 0),          -- 当前会话亲密度，整数 0–999；上下限及整数类型由下方触发器共同保护
    last_return_at                TEXT,                                -- 最近一次旅行回来时间 RFC3339，可空
    travel_trigger_remaining_seconds INTEGER CHECK (travel_trigger_remaining_seconds > 0), -- 下次出行剩余的有效在线秒数；出门中可空
    is_pinned                     INTEGER NOT NULL DEFAULT 0 CHECK (is_pinned IN (0, 1)), -- 会话列表置顶：0=普通，1=置顶；不改变更新时间或当前会话
    FOREIGN KEY (persona_id) REFERENCES personas(id) ON DELETE CASCADE,
    FOREIGN KEY (user_profile_id) REFERENCES user_profiles(id) ON DELETE SET NULL
);

-- 会话亲密度范围保护；使用触发器便于旧库原地升级，不重建会话及其外键子树。
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
    proactive_next_check_idle_ms INTEGER CHECK (proactive_next_check_idle_ms >= 0), -- 下一次抽签对应的 idle_elapsed_ms；bored 时 NULL 表示本周期已成功发送，其他分支为 NULL
    last_proactive_sent_at TEXT, -- 最近主动消息成功保存的 UTC RFC3339；用于同会话 30 分钟冷却，互动及删除消息不清除
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

-- ------------------------------------------------------------
-- 4. 聊天回合与消息
-- 一轮由一条用户消息和一条 AI 回复组成。主动消息 round_id 为 NULL，不计入回合或短期记忆。
-- pending：只有用户消息；completed：AI 正常回复完成；interrupted：回复被中断，不生成短期记忆。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS chat_rounds (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,   -- 聊天回合自增主键
    session_id   INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    status       TEXT    NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'completed', 'interrupted')), -- pending=只有用户消息、等待 AI；completed=AI 正常回复完成；interrupted=回复被中断或只保存部分回复，不生成短期记忆；pending 仅为新回合默认值
    created_at   TEXT    NOT NULL,                    -- 用户消息落库并创建回合的时间 UTC RFC3339
    completed_at TEXT,                                -- 进入 completed/interrupted 的时间；pending 时为 NULL
    stop_reason TEXT CHECK (stop_reason IS NULL OR stop_reason IN ('length_limit', 'user_cancelled', 'timeout', 'repetition', 'invalid_encoding', 'tool_limit', 'provider_error', 'context_changed', 'app_shutdown', 'recovered_after_restart', 'unknown')), -- 仅 interrupted 非空：length_limit=输出长度上限；user_cancelled=用户暂停或编辑后不再续写；timeout=首响应/无进展/整轮超时；repetition=持续重复；invalid_encoding=非法编码；tool_limit=工具上限或无进展；provider_error=供应商错误；context_changed=上下文切换；app_shutdown=退出；recovered_after_restart=启动恢复未完成回合；unknown=旧库停止原因未知；pending/completed 为 NULL
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- 状态与停止原因共同校验；原地迁移沿用相同触发器，不重建消息及记忆子树。
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

CREATE TABLE IF NOT EXISTS messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    round_id   INTEGER,                             -- 所属完整聊天轮次；主动消息为 NULL
    role       TEXT    NOT NULL CHECK (role IN ('user', 'assistant')),                    -- user=用户消息，assistant=AI 消息
    content    TEXT    NOT NULL,                    -- 消息正文
    expression TEXT,                                -- AI 回复时使用的颜文字；用户消息为 NULL
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (round_id) REFERENCES chat_rounds(id) ON DELETE CASCADE,
    UNIQUE (round_id, role)
);

-- ------------------------------------------------------------
-- 邮件设置键（不属于模型提供商）：
-- reminder_email_config：JSON，version=1；connection={email,host,port,encryption,username,from,to}。
--   email=基础发信邮箱；host=SMTP主机；port=端口，默认465；encryption=tls隐式TLS/starttls强制升级，默认tls。
--   username=SMTP登录账号、from=实际发件地址，默认同email；to=收信地址，新表单为空须单独填写，不自动代填；未知邮箱host不猜测。
--   revision=整数，默认0，连接/密钥/开关变动时递增；enabled=false、verified=false；
--   enabledSince=null或UTC ISO8601，仅登记启用后触发的提醒；导入重置revision/开关/绑定/启用时间。
-- reminder_email_secret：Windows DPAPI加密授权码字符串，默认无此键；不回显、不导出。
-- 验证码摘要、10分钟期限、5次错误及60秒重发冷却只在内存，不写入settings或备份。
-- 5. 全局设置（键值对）
-- 下列是当前 V1 完整键目录；值的格式、默认值、范围和成对校验同时登记在《数据库与持久化设计》2.7 节。
--   hungry_minutes                    饥饿触发分钟数，默认 120
--   bored_minutes                     无聊触发分钟数，默认 45
--   sleep_idle_minutes                无互动入睡分钟数，默认 45，用户可修改
--   sleep_min_minutes                 睡眠最短分钟数，默认 60
--   sleep_max_minutes                 睡眠最长分钟数，默认 120
--   short_memory_min_chars            短期记忆最小字数，默认 12
--   short_memory_max_chars            短期记忆最大字数，默认 100
--   long_memory_min_chars             长期记忆最小字数，默认 150
--   long_memory_max_chars             长期记忆最大字数，默认 250
--   memory_merge_count                 生成一条长期记忆使用的短期记忆数 k，默认 20
--   recent_message_count               进入上下文的最近原文消息数，默认 30
--   recent_long_memory_count           直接进入上下文的最近长期记忆数 m，默认 30
--   travel_journal_min_chars           旅行游记最小字数，默认 250
--   travel_journal_max_chars           旅行游记最大字数，默认 350
--   tool_call_rounds                  AI 单次回复最多工具调用轮数，默认 3
--   notebook_max_chars                记事本建议最大字数，默认 4000；只约束 AI 维护，不设数据库硬限制
--   intimacy_threshold_1              第一档亲密度，默认 10；四档均为 1–999 整数且严格递增
--   intimacy_threshold_2              第二档亲密度，默认 50；范围及排序同上
--   intimacy_threshold_3              第三档亲密度，默认 150；范围及排序同上
--   intimacy_threshold_4              第四档亲密度，默认 300；范围及排序同上
--   accessory_cost_1                  第一档饰品价格，默认 20
--   accessory_cost_2                  第二档饰品价格，默认 50
--   accessory_cost_3                  第三档饰品价格，默认 120
--   accessory_cost_4                  第四档及以后饰品价格，默认 300
--   sign_in_min_coins                 每日签到最小喵币，默认 1
--   sign_in_max_coins                 每日签到最大喵币，默认 10
--   travel_min_online_hours           出行最短有效在线间隔，默认 2
--   travel_max_online_hours           出行最长有效在线间隔，默认 4
--   travel_min_duration_minutes       出行最短分钟数，默认 60
--   travel_max_duration_minutes       出行最长分钟数，默认 180
--   travel_accessory_chance_percent   旅行带回饰品概率，默认 10
--   letter_reply_min_minutes          回信最短延迟，默认 10
--   letter_reply_max_minutes          回信最长延迟，默认 30
--   weather_qweather_api_host          和风天气用户专属 HTTPS API Host，可选
--   weather_qweather_api_key_encrypted  和风天气 API Key（DPAPI 加密后的 Base64，可选）
--   current_session_id                当前会话 ID；启动校验，导入重映射
--   window_pos_x                      主宠窗左上角物理像素 X，规范有符号 i32 文本；无默认行，与 Y 同事务保存；可为负数
--   window_pos_y                      主宠窗左上角物理像素 Y，规范有符号 i32 文本；缺失/残缺/无效坐标回退当前工作区右下角
--   窗口位置只由原生拖动服务写入，不能经通用 save_settings 修改；显示器/DPI 临时回位不覆盖用户首选位置
--   theme_mode                       主程序主题：system / light / dark，默认 system；不传递给小游戏插件
--   provider_roles_revision          通用用途配置冲突版本；未登记为0，每次连接或用途保存递增
--   image_generation_shared_revision 生图配置版本；只在生图用途或其连接变更时递增
--   image_generation_config          生图专用版本化JSON；默认version=1/revision=0/nextId=1/enabled=false/activeId=null/providers=[]
--                                    providers=[{id,name,protocol,baseUrl,model}]，protocol仅openai-compatible，baseUrl为HTTPS根地址；按根地址追加/images/generations
--                                    revision每次保存/选择/开关/删除递增，写入必须匹配expectedRevision；nextId递增不复用，ID及公开nextId用十进制字符串
--   image_generation_provider_<id>_api_key_encrypted  生图专用DPAPI密文；缺行=无Key，只能由生图仓储与配置同事务写入，不经通用settings或插件接口
--                                    不借用聊天/记忆/Embedding密钥；全量备份只带非密钥配置，导入强制关闭；不导出任何生图密钥明文/密文
--   web_search_config                搜索专用版本化 JSON，未登记=关闭且空列表；version=1，revision=配置变更序号，nextId=下一个十进制 ID
--                                    enabled=全局开关，activeId=当前配置 ID/null，providers=[{id,name,kind,baseUrl}]；kind=tavily/bocha/custom（Tavily 兼容）
--   web_search_provider_<id>_api_key_encrypted  对应搜索配置的 DPAPI 密文；只允许专用仓储同事务写入，缺行=没有 Key
--                                    所有搜索密钥（含密文）排除备份；全量导入保留元数据但强制关闭搜索，重新填 Key 后由用户启用
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT NOT NULL PRIMARY KEY, -- 全局设置键；只允许本文件上方及《数据库与持久化设计》2.7 节登记的 V1 键名
    value TEXT NOT NULL       -- 设置值；按登记键解析为整数、枚举、HTTPS URL、专用版本化 JSON 或 DPAPI Base64 密文
);

-- ------------------------------------------------------------
-- 6. 短期记忆
-- 每个 completed 回合生成且只生成一条；整合后仍永久保留。
-- integration_state：pending=尚未整合成长期记忆，integrated=已经整合。
-- 短期记忆和长期记忆是中性事实记录，不使用人设口吻，不需要任何氛围总结或主观评价，必须只是如同剧本一样的叙事描述。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS short_memories (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,   -- 短期记忆自增主键
    session_id        INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    round_id          INTEGER NOT NULL UNIQUE,             -- 来源完整回合 -> chat_rounds.id；一个回合最多一条短期记忆
    content           TEXT    NOT NULL,                    -- 该轮用户消息与 AI 回复的中性事实概括
    integration_state TEXT    NOT NULL DEFAULT 'pending' CHECK (integration_state IN ('pending', 'integrated')), -- pending=尚未成为长期记忆来源；integrated=已被 long_memory_sources 引用；pending 仅为新短期记忆默认值
    created_at        TEXT    NOT NULL,                    -- 首次生成时间 UTC RFC3339
    updated_at        TEXT    NOT NULL,                    -- 用户修改或重新整合后的更新时间 UTC RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (round_id) REFERENCES chat_rounds(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 7. 长期记忆
-- 当第 k+1 条短期记忆出现时，由此前连续 k 条 pending 短期记忆生成。
-- 实际覆盖边界由 long_memory_sources 中 source_order 最大的短期记忆及其来源回合推导。
-- activated_by_round_id 指向触发整合的第 k+1 轮，用于删除触发轮时精确回退。
-- 标题与正文同次生成，均为中性事实记录，并共同参与混合检索。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS long_memories (
    id                         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 长期记忆自增主键
    session_id                 INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    title                      TEXT    NOT NULL CHECK (length(trim(title)) > 0), -- 用于列表和检索的一句简短中性标题
    content                    TEXT    NOT NULL,                    -- 由连续 k 条短期记忆整合出的中性事实正文
    source_count               INTEGER NOT NULL CHECK (source_count > 0), -- 实际整合的短期记忆数量，正常等于生成时的 k
    activated_by_round_id      INTEGER NOT NULL UNIQUE,             -- 触发本条长期记忆生成的第 k+1 回合 -> chat_rounds.id
    created_at                 TEXT    NOT NULL,                    -- 首次生成时间 UTC RFC3339
    updated_at                 TEXT    NOT NULL,                    -- 用户修改或重新整合后的更新时间 UTC RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (activated_by_round_id) REFERENCES chat_rounds(id) ON DELETE CASCADE
);

-- 长期记忆全文索引。trigram 支持中文连续字符的子串匹配；不足三个字符的查询由 Rust 层补充普通匹配。
-- 此表是可重建数据，逻辑备份不导出；可执行 INSERT INTO long_memories_fts(long_memories_fts) VALUES('rebuild') 重建。
CREATE VIRTUAL TABLE IF NOT EXISTS long_memories_fts USING fts5(
    title,                -- long_memories.title 的可重建全文索引列
    content,              -- long_memories.content 的可重建全文索引列
    session_id UNINDEXED, -- 所属会话筛选值，不参与分词索引
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
    long_memory_id INTEGER PRIMARY KEY,   -- 对应长期记忆 -> long_memories.id；每条长期记忆一份当前向量
    session_id     INTEGER NOT NULL,      -- 所属会话 -> sessions.id，便于按会话检索
    model_key      TEXT    NOT NULL,      -- local:<manifest SHA256>:<维度> 或 online:<端点+模型 SHA256>:<维度>；不含密钥
    dimensions     INTEGER NOT NULL CHECK (dimensions > 0), -- 向量维度
    vector         BLOB    NOT NULL,      -- 归一化 float32 向量二进制，字节数必须为 dimensions × 4
    content_hash   TEXT    NOT NULL,      -- 标题与正文内容哈希，用于判断向量是否需要重建
    updated_at     TEXT    NOT NULL,      -- 最近生成向量的时间 UTC RFC3339
    CHECK (typeof(vector) = 'blob' AND length(vector) = dimensions * 4),
    FOREIGN KEY (long_memory_id) REFERENCES long_memories(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS long_memory_sources (
    long_memory_id  INTEGER NOT NULL,        -- 目标长期记忆 -> long_memories.id
    short_memory_id INTEGER NOT NULL UNIQUE, -- 来源短期记忆 -> short_memories.id；每条短期记忆最多被整合一次
    source_order    INTEGER NOT NULL CHECK (source_order > 0), -- 该短期记忆在本次 k 条来源中的顺序；最大值对应长期记忆的实际覆盖边界
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
    content    TEXT    NOT NULL,      -- 记事本全文；notebook_max_chars 是 AI 维护软限制，数据库不截断、不拒绝超限内容
    updated_at TEXT    NOT NULL,      -- 最近更新时间 RFC3339
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 9. 提醒
-- kind: once / daily / weekly / monthly
-- schedule: 重复规则 JSON；一次性提醒为 NULL。
--   每天：{"version":1,"repeat":"daily","time":"09:00"}
--   每周：{"version":1,"repeat":"weekly","weekdays":[1,3,5],"time":"09:00"}（1=周一 … 7=周日）
--   每月：{"version":1,"repeat":"monthly","days":[1,15],"time":"09:00"}
--   后端追加 next_local_at（如 "2026-09-29T09:00:00"），保存下一次当地日历游标；不接受用户修改。
--   缺 version 的旧数据按 1 读取；缺 next_local_at 时沿用 fire_at，首次推进时补齐。
-- 循环时间跟随电脑当前时区；时区变化时重算待触发游标的 UTC，已入队 scheduled_at 不变。
-- 月度不存在日期、夏令时不存在时刻均跳过；重复当地时刻只使用第一次。
-- status: pending / done / expired
-- fire_at 保存下一次待入队触发时间；同事务建立 occurrence 后才推进/置空。
-- 重复提醒 status 始终为 pending；完成/过期仅更新本次 occurrence，永久停止须删除 reminders。
-- 一次性最新触发记录完成/过期且无未来计划时同步本表；旧记录不覆盖重新安排的提醒。
-- 原到期时间由 occurrence.scheduled_at 永久保留。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS reminders (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    name       TEXT    NOT NULL CHECK (length(trim(name)) > 0), -- 提醒名，用于气泡与离线过期汇总
    kind       TEXT    NOT NULL CHECK (kind IN ('once', 'daily', 'weekly', 'monthly')),                    -- once=一次性，daily=每天，weekly=每周，monthly=每月
    schedule   TEXT,                                -- 重复规则 JSON；一次性为 NULL
    content    TEXT    NOT NULL,                    -- 提醒内容
    status     TEXT    NOT NULL CHECK (status IN ('pending', 'done', 'expired')),                    -- pending=规则仍待触发，done=一次性提醒已完成，expired=一次性提醒已过期；重复规则始终 pending
    fire_at    TEXT,                                -- 下次触发时间 RFC3339
    source     TEXT    NOT NULL CHECK (source IN ('chat', 'manual')), -- chat=由聊天创建，manual=用户在提醒页手动创建
    created_at TEXT    NOT NULL,                    -- 创建时间 RFC3339
    CHECK ((kind = 'once' AND schedule IS NULL) OR (kind <> 'once' AND schedule IS NOT NULL AND status = 'pending'))
);

-- 每次提醒触发记录；与调度游标更新在同一事务内写入。
-- missed：应用关闭期间错过，等待下次启动统一汇总；queued：在线到期后等待前端领取。
-- shown：提醒气泡或离线汇总已确认展示；状态未完成/已过期的记录仍可在提醒页面处理。
CREATE TABLE IF NOT EXISTS reminder_occurrences (
    id             INTEGER PRIMARY KEY AUTOINCREMENT, -- 单次提醒触发记录自增主键
    reminder_id    INTEGER NOT NULL,                  -- 所属提醒规则 -> reminders.id
    scheduled_at   TEXT NOT NULL,                 -- 本次原定触发时间 UTC RFC3339
    status         TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'done', 'expired')), -- pending=本次未处理；done=用户完成本次；expired=本次已过期；pending 仅为新触发记录默认值
    delivery_state TEXT NOT NULL CHECK (delivery_state IN ('missed', 'queued', 'shown')), -- missed=关机期间错过待汇总；queued=在线到期待展示；shown=已展示或已纳入离线汇总
    completed_at   TEXT,                            -- 本次进入 done/expired 的时间；pending 时为 NULL
    created_at     TEXT NOT NULL,                   -- 触发记录创建时间 UTC RFC3339
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
-- 11. 小游戏插件
-- 石头剪刀布是随便携版提供的第一方插件，也登记在本组表中。
-- game_plugins 只表示当前登记状态；历史运行记录自行保存插件快照，不依赖登记行。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS game_plugins (
    plugin_id         TEXT    PRIMARY KEY CHECK (length(trim(plugin_id)) > 0), -- manifest 中稳定且全局唯一的插件 ID
    name              TEXT    NOT NULL CHECK (length(trim(name)) > 0), -- 当前安装版本的插件显示名称
    description       TEXT    NOT NULL DEFAULT '', -- 当前安装版本的插件简介
    author            TEXT    NOT NULL CHECK (length(trim(author)) > 0), -- manifest 声明的作者
    installed_version TEXT    NOT NULL CHECK (length(trim(installed_version)) > 0), -- 当前已安装插件版本
    api_version       TEXT    NOT NULL CHECK (length(trim(api_version)) > 0), -- 插件要求的颜喵宿主 API 版本
    storage_version   INTEGER NOT NULL DEFAULT 1 CHECK (storage_version > 0), -- 插件私有存档结构版本
    source_type       TEXT    NOT NULL CHECK (source_type IN ('bundled', 'local')), -- bundled=便携版自带，local=用户安装
    package_path      TEXT,                         -- 相对插件目录的路径；缺包时为 NULL
    package_sha256    TEXT    NOT NULL CHECK (length(trim(package_sha256)) > 0), -- 当前插件包 SHA-256，用于完整性和版本识别
    repository        TEXT,                         -- manifest 提供的 GitHub 仓库地址，可空
    homepage          TEXT,                         -- 项目主页，可空
    release_page      TEXT,                         -- Release 下载页，可空
    network_origins_json TEXT NOT NULL DEFAULT '[]', -- manifest 声明的 HTTPS origin 列表；由 Rust 校验 JSON、格式和数量
    is_bundled        INTEGER NOT NULL DEFAULT 0 CHECK (is_bundled IN (0, 1)), -- 1=便携版内置插件，0=用户安装插件；须与 source_type 一致
    package_status    TEXT    NOT NULL DEFAULT 'available' CHECK (package_status IN ('available', 'missing_package')), -- available=插件包可用；missing_package=仅恢复了使用数据、等待用户重装包
    is_enabled        INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)), -- 1=允许加载，0=停用；缺包时必须为 0
    installed_at      TEXT    NOT NULL, -- 首次安装或内置登记时间 UTC RFC3339
    updated_at        TEXT    NOT NULL, -- 插件登记最近更新时间 UTC RFC3339
    CHECK (
        (source_type = 'bundled' AND is_bundled = 1)
        OR (source_type = 'local' AND is_bundled = 0)
    ),
    CHECK (
        (package_status = 'available' AND package_path IS NOT NULL AND length(trim(package_path)) > 0)
        OR (package_status = 'missing_package' AND package_path IS NULL AND is_enabled = 0)
    )
);

-- manifest 中声明的是权限申请；本表保存用户对当前插件版本的授权状态。
-- 独立插件数据包导入时，钱包和联网权限统一写为 pending，重新获得用户决定前不可调用。
CREATE TABLE IF NOT EXISTS game_permissions (
    plugin_id      TEXT NOT NULL, -- 所属当前插件登记 -> game_plugins.plugin_id
    permission     TEXT NOT NULL CHECK (length(trim(permission)) > 0), -- 权限名；V1 为 wallet.modify 或 network.fetch
    permission_scope TEXT NOT NULL DEFAULT '',       -- wallet.modify 为空；network.fetch 保存获授权的 HTTPS origin
    decision       TEXT NOT NULL CHECK (decision IN ('pending', 'allowed', 'denied')), -- pending=待用户确认；allowed=允许；denied=拒绝
    plugin_version TEXT NOT NULL CHECK (length(trim(plugin_version)) > 0), -- 作出决定时对应的插件版本，升级后可要求重新确认
    decided_at     TEXT, -- 用户允许或拒绝的时间；pending 时为 NULL
    PRIMARY KEY (plugin_id, permission, permission_scope),
    CHECK (
        (decision = 'pending' AND decided_at IS NULL)
        OR (decision IN ('allowed', 'denied') AND decided_at IS NOT NULL)
    ),
    FOREIGN KEY (plugin_id) REFERENCES game_plugins(plugin_id) ON DELETE CASCADE
);

-- 插件自己的设置，不占用主程序 settings；允许键由 manifest.settings_schema 声明。
-- setting_key 示例：audio.enabled、audio.volume、game.difficulty、game.confirm_action、accessibility.reduce_motion。
-- value_json 可保存 JSON 布尔、数字、字符串、数组或对象；Rust 按 manifest 类型/范围校验后写入。
CREATE TABLE IF NOT EXISTS game_plugin_settings (
    plugin_id   TEXT NOT NULL, -- 所属当前插件登记 -> game_plugins.plugin_id
    setting_key TEXT NOT NULL CHECK (length(trim(setting_key)) > 0), -- 插件声明的稳定设置键，例如 audio.volume；没有覆盖值时读取 manifest 默认值
    value_json  TEXT NOT NULL, -- 当前用户设置的 JSON 值，例如 true、80、"normal"；须符合 settings_schema
    updated_at  TEXT NOT NULL, -- 最近更新时间 UTC RFC3339
    PRIMARY KEY (plugin_id, setting_key),
    FOREIGN KEY (plugin_id) REFERENCES game_plugins(plugin_id) ON DELETE CASCADE
);

-- 小游戏插件与人设、会话无关；存档只按插件 ID 和数据键隔离。
-- data_key 示例：progress.main、stats.overall、records.high_scores、unlocks.items、tutorial.state、state.current。
-- value_json 是插件自有结构；主程序只校验合法 JSON、容量与 storage_version，不解释游戏业务含义。
CREATE TABLE IF NOT EXISTS game_plugin_data (
    plugin_id       TEXT    NOT NULL, -- 所属当前插件登记 -> game_plugins.plugin_id
    data_key        TEXT    NOT NULL CHECK (length(trim(data_key)) > 0), -- 插件稳定存档键，例如 stats.overall；同一插件内唯一
    value_json      TEXT    NOT NULL, -- 插件私有存档 JSON，例如 {"wins":12,"losses":8}；不得作为钱包或账单权威数据
    storage_version INTEGER NOT NULL CHECK (storage_version > 0), -- 写入该存档时使用的插件存档结构版本
    updated_at      TEXT    NOT NULL, -- 最近更新时间 UTC RFC3339
    PRIMARY KEY (plugin_id, data_key),
    FOREIGN KEY (plugin_id) REFERENCES game_plugins(plugin_id) ON DELETE CASCADE
);

-- 历史运行记录故意不外键关联 game_plugins；卸载或清除登记后仍可审计。
-- 运行记录是应用全局数据，不保存 persona_id 或 session_id。
CREATE TABLE IF NOT EXISTS game_runs (
    id                        INTEGER PRIMARY KEY AUTOINCREMENT, -- 运行记录自增主键
    plugin_id                 TEXT    NOT NULL CHECK (length(trim(plugin_id)) > 0), -- 打开时插件 ID 快照，卸载后仍保留
    plugin_name               TEXT    NOT NULL CHECK (length(trim(plugin_name)) > 0), -- 打开时插件显示名称快照
    plugin_version            TEXT    NOT NULL CHECK (length(trim(plugin_version)) > 0), -- 打开时插件版本快照
    wallet_permission_granted INTEGER NOT NULL CHECK (wallet_permission_granted IN (0, 1)), -- 1=打开时已获 wallet.modify 授权，0=本次运行纯玩；中途授权不改变本次运行
    status                    TEXT    NOT NULL CHECK (status IN ('running', 'completed', 'cancelled', 'error')), -- running=进行中；completed=正常结束运行；cancelled=用户取消；error=异常结束
    outcome                   TEXT, -- 插件给出的本次运行结果标识；未结束或无结果时可为 NULL
    details_json              TEXT, -- 插件提供的本次运行明细 JSON，可含 schema_version、mode、选择/得分、result、duration_ms；可空且不得决定钱包事实
    coin_delta                INTEGER NOT NULL DEFAULT 0, -- 本次运行累计实际喵币变化；未授权运行为 0
    started_at                TEXT    NOT NULL, -- 打开时间 UTC RFC3339
    ended_at                  TEXT, -- 结束时间；running 时为 NULL
    UNIQUE (id, plugin_id, plugin_version),
    CHECK (
        (status = 'running' AND ended_at IS NULL)
        OR (status <> 'running' AND ended_at IS NOT NULL)
    )
);

-- ------------------------------------------------------------
-- 12. 喵币账单
-- 所有喵币变动都记一条流水，并与 user_state 的余额更新放在同一事务。
-- 插件每次 wallet.changeCoins() 写一条 plugin_game 流水；平局 delta=0 也保留。
-- ref_id：制作/卖出对应 accessories.id（删除后仅作历史引用）；其他原因为 NULL。
-- plugin_id 不设 game_plugins 外键，确保卸载插件后账单仍可读。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS currency_ledger (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT, -- 喵币流水自增主键
    delta                 INTEGER NOT NULL, -- 本次余额变化量；正数增加、负数扣除、插件平局可为 0
    reason                TEXT    NOT NULL CHECK (reason IN ('daily_sign_in', 'sign_in_festival', 'plugin_game', 'accessory_craft', 'accessory_sell')), -- daily_sign_in=普通签到；sign_in_festival=节日签到；plugin_game=插件游戏；accessory_craft=制作饰品；accessory_sell=卖出饰品
    ref_id                INTEGER, -- 制作/卖出时保存 accessories.id 的历史引用；其他原因及插件游戏为 NULL
    game_run_id           INTEGER, -- 插件游戏流水所属运行记录 -> game_runs.id；非插件流水为 NULL
    plugin_id             TEXT, -- 插件游戏时保存插件 ID 快照；非插件流水为 NULL
    plugin_version        TEXT, -- 插件游戏时保存插件版本快照；非插件流水为 NULL
    plugin_display_reason TEXT, -- 插件展示给用户的本次变动原因；插件流水必填
    wallet_request_id     TEXT, -- 插件生成的幂等请求 ID；同一插件不可重复记账
    created_at            TEXT    NOT NULL, -- 流水创建时间 UTC RFC3339
    CHECK (
        (
            reason = 'plugin_game'
            AND game_run_id IS NOT NULL
            AND plugin_id IS NOT NULL
            AND length(trim(plugin_id)) > 0
            AND plugin_version IS NOT NULL
            AND length(trim(plugin_version)) > 0
            AND plugin_display_reason IS NOT NULL
            AND length(trim(plugin_display_reason)) > 0
            AND wallet_request_id IS NOT NULL
            AND length(trim(wallet_request_id)) > 0
            AND ref_id IS NULL
        )
        OR (
            reason <> 'plugin_game'
            AND game_run_id IS NULL
            AND plugin_id IS NULL
            AND plugin_version IS NULL
            AND plugin_display_reason IS NULL
            AND wallet_request_id IS NULL
        )
    ),
    FOREIGN KEY (game_run_id, plugin_id, plugin_version) REFERENCES game_runs(id, plugin_id, plugin_version) ON DELETE NO ACTION,
    FOREIGN KEY (game_run_id) REFERENCES game_runs(id) ON DELETE RESTRICT
);

-- ------------------------------------------------------------
-- 13. 出行记录（游记、信件、伴手礼与旅行饰品的共同关联根节点）
-- status: away=出门中，returned=正常返回，cancelled=切换会话取消。
-- 各生成状态独立持久化；删除已生成结果不重置 done，避免重新生成。
-- midway_letter_at 出发时确定；gift_kind 在正常返回事务中抽取并固定。
-- planned_return_at 用于判断关闭应用后是否已经到返回时间。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travels (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id        INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    status            TEXT    NOT NULL CHECK (status IN ('away', 'returned', 'cancelled')), -- away=出门中，returned=正常返回，cancelled=切换会话或人设后取消
    started_at        TEXT    NOT NULL,                    -- 出发时间 RFC3339
    planned_return_at TEXT    NOT NULL,                    -- 计划返回时间 RFC3339
    actual_return_at  TEXT,                                -- 正常返回时间；离线到期使用计划返回时间
    cancelled_at      TEXT,                                -- 切换会话取消时间；取消不视为正常返回
    midway_letter_at  TEXT NOT NULL,                       -- 出发时确定为行程中点
    midway_letter_state TEXT NOT NULL DEFAULT 'pending' CHECK (midway_letter_state IN ('pending', 'done', 'cancelled')), -- 途中信生成状态：pending=待生成，done=已生成，cancelled=不再生成
    return_letter_state TEXT NOT NULL DEFAULT 'pending' CHECK (return_letter_state IN ('pending', 'done', 'cancelled')), -- 回来信生成状态：pending=待生成，done=已生成，cancelled=不再生成
    gift_state        TEXT NOT NULL DEFAULT 'pending' CHECK (gift_state IN ('pending', 'done', 'cancelled')), -- 伴手礼生成/发放状态：pending=待处理，done=已发放，cancelled=不再发放
    journal_state     TEXT NOT NULL DEFAULT 'pending' CHECK (journal_state IN ('pending', 'done', 'cancelled')), -- 游记生成状态：pending=待生成，done=已生成过，cancelled=不再生成；删除游记不重置 done
    gift_kind         TEXT CHECK (gift_kind IN ('text', 'accessory')), -- text=普通文字伴手礼，accessory=可佩戴旅行饰品；正常返回时确定一次
    facts_state       TEXT NOT NULL DEFAULT 'pending' CHECK (facts_state IN ('pending', 'ready', 'cancelled')), -- 固定出行事实状态：pending=待生成，ready=已固定，cancelled=未生成且不再生成
    destination       TEXT, -- 固定目的地；facts_state=ready 时非空
    activities_json   TEXT, -- 活动时间线数组，elapsed_minutes 与 description；Rust 校验各元素
    facts_version     INTEGER NOT NULL DEFAULT 0 CHECK (facts_version >= 0), -- 固定事实首次保存为 1；异步快照校验
    facts_ready_at    TEXT, -- 固定出行事实完成时间；facts_state 非 ready 时为 NULL
    gift_snapshot_json TEXT, -- 实际发放结果快照，与礼物行及 gift_state=done 同事务写入；删礼物仍保留
    created_at        TEXT    NOT NULL,                    -- 创建时间 RFC3339
    UNIQUE (id, session_id), -- 为旅行子项的同会话复合外键提供父键
    CHECK (
        (facts_state = 'ready' AND destination IS NOT NULL AND length(trim(destination)) > 0
         AND activities_json IS NOT NULL AND facts_version = 1 AND facts_ready_at IS NOT NULL
         AND CASE WHEN json_valid(activities_json) THEN json_type(activities_json) = 'array' AND json_array_length(activities_json) > 0 ELSE 0 END)
        OR (facts_state IN ('pending', 'cancelled') AND destination IS NULL AND activities_json IS NULL AND facts_version = 0 AND facts_ready_at IS NULL)
    ),
    CHECK (
        (status = 'away' AND actual_return_at IS NULL AND cancelled_at IS NULL AND gift_kind IS NULL AND facts_state <> 'cancelled')
        OR (status = 'returned' AND actual_return_at IS NOT NULL AND cancelled_at IS NULL AND gift_kind IS NOT NULL AND facts_state <> 'cancelled')
        OR (status = 'cancelled' AND actual_return_at IS NULL AND cancelled_at IS NOT NULL AND gift_kind IS NULL AND facts_state <> 'pending')
    ),
    CHECK (
        (gift_state = 'done' AND status = 'returned' AND facts_state = 'ready' AND gift_snapshot_json IS NOT NULL
         AND CASE WHEN json_valid(gift_snapshot_json) THEN
             json_type(gift_snapshot_json) = 'object'
             AND json_extract(gift_snapshot_json, '$.schema_version') IS 1
             AND json_extract(gift_snapshot_json, '$.kind') IS gift_kind
         ELSE 0 END)
        OR (gift_state <> 'done' AND gift_snapshot_json IS NULL)
    ),
    CHECK (journal_state <> 'done' OR (status = 'returned' AND facts_state = 'ready' AND gift_state = 'done')),
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 14. 旅行游记
-- 每次正常返回的旅行一篇；按人设口吻详细总结整次旅行，不参与短期或长期记忆整合。
-- 标题用一句话极简概括何时去了哪里、做了什么、带回什么伴手礼、去了多久。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS travel_journals (
    id         INTEGER PRIMARY KEY AUTOINCREMENT, -- 旅行游记自增主键
    session_id INTEGER NOT NULL,                  -- 所属会话 -> sessions.id
    travel_id  INTEGER NOT NULL UNIQUE,           -- 来源出行记录 -> travels.id；每次出行最多一篇游记
    title      TEXT    NOT NULL,                  -- 一句话精炼概括时间、地点、活动、伴手礼和时长
    content    TEXT    NOT NULL,                  -- AI 按人设口吻写的完整旅行总结，不参与长期记忆整合
    created_at TEXT    NOT NULL,                  -- 首次生成时间 UTC RFC3339
    updated_at TEXT    NOT NULL,                  -- 用户修改后的最近更新时间 UTC RFC3339
    FOREIGN KEY (travel_id, session_id) REFERENCES travels(id, session_id) ON DELETE NO ACTION,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id) REFERENCES travels(id) ON DELETE CASCADE
);

-- ------------------------------------------------------------
-- 15. 饰品
-- 每个饰品属于一个会话；source: crafted=制作，travel=旅行带回。
-- 同时最多佩戴一件；is_worn=1表示当前佩戴。NULL坐标沿用头顶，非NULL坐标为相对桌宠的位置。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS accessories (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id   INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    name         TEXT    NOT NULL,                    -- 饰品名称
    display_text TEXT    NOT NULL,                    -- 头顶视觉兜底：图标字符/Emoji/颜文字/符号组合，不是名称或简介
    description  TEXT    NOT NULL,                    -- AI 按人设口吻写的简介
    cost         INTEGER NOT NULL DEFAULT 0 CHECK (cost >= 0),          -- 制作费用；旅行带回为 0
    source       TEXT    NOT NULL CHECK (source IN ('crafted', 'travel')), -- crafted=使用喵币制作，travel=旅行带回
    travel_id    INTEGER,                             -- 旅行带回时为 travels.id，制作为 NULL
    is_worn      INTEGER NOT NULL DEFAULT 0 CHECK (is_worn IN (0, 1)),          -- 是否佩戴：1 佩戴，0 未佩戴
    created_at   TEXT    NOT NULL,                    -- 创建时间 RFC3339
    image_file   TEXT,                                -- 可选透明饰品PNG受管相对路径；NULL显示display_text，不存远程URL或绝对路径
    position_x INTEGER CHECK(position_x BETWEEN -1500 AND 1500), -- 饰品中心相对桌宠中心X，宽度千分比；NULL默认头顶
    position_y INTEGER CHECK(position_y BETWEEN -1500 AND 1500), -- 饰品中心相对桌宠中心Y，高度千分比；NULL默认头顶
    scale INTEGER NOT NULL DEFAULT 1000 CHECK(scale BETWEEN 200 AND 3000), -- 饰品缩放千分比，1000默认
    FOREIGN KEY (travel_id, session_id) REFERENCES travels(id, session_id) ON DELETE NO ACTION,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)  REFERENCES travels(id)   ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 16. 信件
-- sender: user=用户写的信，ai=AI 写的信；AI 生成的信件内容应包含可爱颜文字。
-- reply_to_id 非空时表示这是对某封信的回复；每封信最多回复一次。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS letters (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id   INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    travel_id    INTEGER,                             -- 关联旅行 -> travels.id，可空
    sender       TEXT    NOT NULL CHECK (sender IN ('user', 'ai')),                    -- user=用户来信，ai=AI 途中信、回来信或回信
    content      TEXT    NOT NULL,                    -- 信件正文
    reply_to_id  INTEGER,                             -- 回复目标；删除目标后置空
    has_been_replied INTEGER NOT NULL DEFAULT 0 CHECK (has_been_replied IN (0, 1)), -- 1=历史上已成功回复，0=尚未回复；删除回复不重置
    reply_due_at TEXT,                                -- 用户信自动回信时间，首次发送时抽取并保存
    reply_state  TEXT NOT NULL DEFAULT 'none' CHECK (reply_state IN ('none', 'pending', 'completed', 'cancelled')), -- none=AI 信无需自动回信，pending=用户信待回，completed=已回信，cancelled=切换会话或旅行返回后取消
    letter_kind  TEXT NOT NULL CHECK (letter_kind IN ('user', 'midway', 'return', 'reply')), -- user=用户来信，midway=途中信，return=回来信，reply=对用户信的回复
    is_favorite  INTEGER NOT NULL DEFAULT 0 CHECK (is_favorite IN (0, 1)),          -- 是否收藏：1 收藏，0 未收藏
    is_read      INTEGER NOT NULL DEFAULT 0 CHECK (is_read IN (0, 1)),          -- 用户是否已读：1 已读，0 未读
    created_at   TEXT    NOT NULL,                    -- 写信/回信时间 RFC3339
    CHECK ((sender = 'user' AND letter_kind = 'user' AND reply_state <> 'none' AND reply_due_at IS NOT NULL) OR (sender = 'ai' AND letter_kind <> 'user' AND reply_state = 'none' AND reply_due_at IS NULL)),
    FOREIGN KEY (travel_id, session_id) REFERENCES travels(id, session_id) ON DELETE NO ACTION,
    FOREIGN KEY (session_id)  REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)   REFERENCES travels(id)  ON DELETE SET NULL,
    FOREIGN KEY (reply_to_id) REFERENCES letters(id)  ON DELETE SET NULL
);

-- 每封信只能被回复一次：reply_to_id 非空时唯一。
CREATE UNIQUE INDEX IF NOT EXISTS idx_letters_reply_to_unique
    ON letters(reply_to_id)
    WHERE reply_to_id IS NOT NULL;

-- ------------------------------------------------------------
-- 17. 伴手礼
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
    FOREIGN KEY (travel_id, session_id) REFERENCES travels(id, session_id) ON DELETE NO ACTION,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (travel_id)  REFERENCES travels(id)  ON DELETE SET NULL
);

-- ------------------------------------------------------------
-- 18. 节日母表（系统级内置）
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
    calendar_type TEXT    NOT NULL CHECK (calendar_type IN ('solar', 'lunar', 'solar_term')), -- solar=公历节日，lunar=农历节日，solar_term=节气
    solar_month   INTEGER,                             -- 公历月；农历节日与节气按内置表推出本年月日后写入
    solar_day     INTEGER,                             -- 公历日；农历节日与节气按内置表推出本年月日后写入
    lunar_month   INTEGER,                             -- 农历月；仅农历节日填写，公历节日与节气为 NULL
    lunar_day     INTEGER,                             -- 农历日；仅农历节日填写，公历节日与节气为 NULL
    is_leap_month INTEGER NOT NULL DEFAULT 0 CHECK (is_leap_month IN (0, 1)),          -- 1=农历闰月，0=普通月份或非农历记录
    solar_term    TEXT,                                -- 节气名，如 清明 / 冬至；非节气为 NULL
    blessing      TEXT    NOT NULL,                    -- 内置祝福词，签到彩蛋直接展示，不调用 AI
    is_memorial   INTEGER NOT NULL DEFAULT 0 CHECK (is_memorial IN (0, 1)),          -- 1=纪念性节日且文案不使用“快乐”，0=普通节日
    note          TEXT                                 -- 备注
);

-- ------------------------------------------------------------
-- 19. 会话节日子表（会话级）
-- 由 AI 或用户维护的节日、生日、纪念日、自定义日子。
-- 与母表同样只存公历月日与农历月日；农历行每年查内置表推出公历月日后更新。
-- 新增仅保存会话自己的固定日期，不复制母表；festival_id固定NULL。
-- festival_id列保留旧数据/备份兼容，不提供新建关联入口。
-- 行不随年份删除；AI 可删除自己不需要的行。
-- 子表与母表一起参与签到、日常聊天和无聊主动消息上下文。
-- ------------------------------------------------------------
CREATE TABLE IF NOT EXISTS session_festivals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
    session_id    INTEGER NOT NULL,                    -- 所属会话 -> sessions.id
    festival_id   INTEGER,                             -- 旧数据/备份兼容母表引用；当前新增为NULL
    kind          TEXT    NOT NULL CHECK (kind IN ('festival', 'birthday', 'anniversary', 'custom')), -- festival=节日，birthday=生日，anniversary=纪念日，custom=其他自定义日子
    name          TEXT    NOT NULL,                    -- 名称
    year          INTEGER NOT NULL,                    -- 本年公历年份：日期已按哪一年查表更新，不是生日出生年
    calendar_type TEXT    NOT NULL CHECK (calendar_type IN ('solar', 'lunar')), -- solar=公历日期，lunar=农历日期
    solar_month   INTEGER,                             -- 公历月；农历日子按内置表推出本年月日后写入
    solar_day     INTEGER,                             -- 公历日；农历日子按内置表推出本年月日后写入
    lunar_month   INTEGER,                             -- 农历月；仅农历日子填写，公历日子为 NULL
    lunar_day     INTEGER,                             -- 农历日；仅农历日子填写，公历日子为 NULL
    is_leap_month INTEGER NOT NULL DEFAULT 0 CHECK (is_leap_month IN (0, 1)),          -- 1=农历闰月，0=普通月份或公历记录
    blessing      TEXT NOT NULL CHECK (length(trim(blessing)) > 0), -- 祝福词：AI 新增时必须填写，手动新增也必填
    note          TEXT,                                -- 备注
    source        TEXT    NOT NULL CHECK (source IN ('ai', 'user')), -- ai=AI 创建，user=用户手动创建
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
CREATE INDEX IF NOT EXISTS idx_currency_ledger_game_run ON currency_ledger (game_run_id);
CREATE INDEX IF NOT EXISTS idx_game_plugins_status     ON game_plugins (package_status, is_enabled);
CREATE INDEX IF NOT EXISTS idx_game_runs_plugin_started ON game_runs (plugin_id, started_at);
CREATE INDEX IF NOT EXISTS idx_accessories_session     ON accessories (session_id);
CREATE INDEX IF NOT EXISTS idx_travels_session_status  ON travels (session_id, status);
CREATE INDEX IF NOT EXISTS idx_travel_journals_session ON travel_journals (session_id, id);
CREATE INDEX IF NOT EXISTS idx_letters_session         ON letters (session_id);
CREATE INDEX IF NOT EXISTS idx_souvenirs_session       ON souvenirs (session_id);
CREATE INDEX IF NOT EXISTS idx_session_festivals_session ON session_festivals (session_id);

-- 全局当前项最多一个；允许无提供商或无显式记忆提供商。
CREATE UNIQUE INDEX IF NOT EXISTS idx_embedding_providers_one_active ON embedding_providers(is_active) WHERE is_active = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_personas_one_active ON personas(is_active) WHERE is_active = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_personas_one_builtin ON personas(is_builtin) WHERE is_builtin = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accessories_one_worn ON accessories(session_id) WHERE is_worn = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_travels_one_away ON travels(session_id) WHERE status = 'away';
CREATE INDEX IF NOT EXISTS idx_reminder_occurrences_pending ON reminder_occurrences(status, delivery_state);
CREATE INDEX IF NOT EXISTS idx_short_memories_state ON short_memories(session_id, integration_state, id);
CREATE INDEX IF NOT EXISTS idx_long_memory_sources_long ON long_memory_sources(long_memory_id, source_order);
CREATE UNIQUE INDEX IF NOT EXISTS idx_currency_ledger_plugin_request
    ON currency_ledger(plugin_id, wallet_request_id)
    WHERE reason = 'plugin_game';

-- 跨表规则由 Rust 命令在事务中检查：round/message/记忆来源、旅行/游记/信件/礼物/饰品必须属于同一会话。
-- 切换当前项先验证目标存在，再在同一事务内清除旧项并启用新项。
-- 余额检查、签到/游戏次数检查、插件运行记录、饰品增删、档位使用标记和流水写入必须共用同一事务。
-- settings 在 Rust 层逐键校验：非负值、最小值 <= 最大值、亲密度档位严格递增。
-- 日期、重复规则 JSON、颜色及饰品字符限制在 Rust 层验证；前端校验不替代后端校验。

CREATE INDEX IF NOT EXISTS idx_letters_reply_pending ON letters(reply_state, reply_due_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_letters_travel_kind ON letters(travel_id, letter_kind)
    WHERE travel_id IS NOT NULL AND letter_kind IN ('midway', 'return');
-- journal_state 是旅行游记生成是否完成的永久依据；用户编辑或删除游记不改变 done，取消旅行不生成游记。

-- 每次出行最多一件普通礼物或一件旅行饰品；跨两表的类型选择由 gift_kind 和返回事务校验。
CREATE UNIQUE INDEX IF NOT EXISTS idx_souvenirs_one_per_travel ON souvenirs(travel_id) WHERE travel_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_accessories_one_per_travel ON accessories(travel_id) WHERE travel_id IS NOT NULL;
-- 同会话复合外键检查和按出行读取内容使用这些索引。
CREATE INDEX IF NOT EXISTS idx_letters_travel_session ON letters(travel_id, session_id);
CREATE INDEX IF NOT EXISTS idx_travels_pending_facts ON travels(status, id) WHERE facts_state = 'pending';

CREATE UNIQUE INDEX IF NOT EXISTS idx_embedding_providers_one_builtin ON embedding_providers(is_builtin) WHERE is_builtin = 1;

-- 各连接的模型设置独立保存；切换当前项不会清除之前的模型与聊天预算。
CREATE TABLE IF NOT EXISTS provider_models (
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE, -- 所属通用连接
    purpose TEXT NOT NULL CHECK (purpose IN ('chat','memory','image','vision')), -- chat聊天 / memory记忆 / image生图 / vision识图
    model TEXT NOT NULL CHECK (length(trim(model))>0), -- 该连接在此用途的模型名称
    max_output_tokens INTEGER NOT NULL DEFAULT 32768 CHECK (max_output_tokens>0), -- 聊天默认32768；其他用途保留默认值
    context_window_override INTEGER CHECK (context_window_override>0), -- NULL自动解析；聊天可手动覆盖
    PRIMARY KEY(provider_id,purpose)
);
-- 当前用途选择；模型与预算只存provider_models，不在此重复存储。
CREATE TABLE IF NOT EXISTS provider_roles (
    purpose TEXT PRIMARY KEY CHECK (purpose IN ('chat','memory','image','vision')), -- chat聊天 / memory记忆 / image生图 / vision识图；记忆/识图无行时跟随聊天
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE RESTRICT, -- 当前连接；被用途引用时禁止删除
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)), -- 生图0关闭/1开启；其他用途保持1
    FOREIGN KEY(provider_id,purpose) REFERENCES provider_models(provider_id,purpose) ON DELETE RESTRICT
);
-- 只供原有内部查询读取；字段均从连接和模型配置计算，不重复持久化。
CREATE VIEW IF NOT EXISTS provider_effective AS
SELECT p.id,p.name,p.base_url,p.api_key_encrypted,
       COALESCE(m.model,'未配置') AS model,
       EXISTS(SELECT 1 FROM provider_roles r WHERE r.purpose='chat' AND r.provider_id=p.id) AS is_active,
       EXISTS(SELECT 1 FROM provider_roles r WHERE r.purpose='memory' AND r.provider_id=p.id) AS memory_is_active,
       COALESCE(m.max_output_tokens,32768) AS max_output_tokens,
       m.context_window_override,p.created_at,p.updated_at
FROM providers p LEFT JOIN provider_models m ON m.provider_id=p.id AND m.purpose='chat';
CREATE TABLE IF NOT EXISTS message_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT, -- 附件主键
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE, -- 所属用户消息；删除消息级联清引用
    filename TEXT NOT NULL, -- 用户原文件名，仅用于展示，不作为文件路径
    image_file TEXT NOT NULL, -- chat_images目录内的受管文件名
    media_type TEXT NOT NULL CHECK (media_type IN ('image/png','image/jpeg','image/webp')), -- 实际格式：image/png / image/jpeg / image/webp
    caption TEXT, -- 已生成的图片理解事实；NULL表示尚无理解结果
    created_at TEXT NOT NULL -- 保存UTC时间
);
CREATE INDEX idx_message_images_message ON message_images(message_id,id);

-- 邮件队列是本机运行数据，不导出；旧版升级不补登记历史提醒。
CREATE TABLE IF NOT EXISTS reminder_email_deliveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT, -- 本机邮件任务ID。
    kind TEXT NOT NULL CHECK(kind IN ('due','offline_summary')), -- due到点；offline_summary离线汇总。
    recipient TEXT NOT NULL, -- 登记时的收件地址，发送前复核配置版本。
    subject TEXT NOT NULL, -- 原始提醒生成的主题快照，不经过AI润色。
    body TEXT NOT NULL, -- 原始提醒正文快照；发送前剔除已删除的提醒。
    config_revision INTEGER NOT NULL, -- 创建任务时SMTP配置修订号。
    state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','sending','retry_wait','sent','failed','uncertain','cancelled')), -- pending待发送；sending发送中；retry_wait等待重试；sent已发送；failed失败；uncertain结果不确定；cancelled取消。
    attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0), -- 已开始发送次数；自动最多3次。
    next_retry_at TEXT, -- UTC ISO8601重试时间；非retry_wait时为空。
    error TEXT, -- 可展示的中文错误，不含授权码和SMTP原始响应。
    created_at TEXT NOT NULL, -- UTC ISO8601登记时间。
    sent_at TEXT -- SMTP确认成功的UTC时间；失败或不确定为空。
);
CREATE INDEX IF NOT EXISTS idx_reminder_email_queue ON reminder_email_deliveries(state,next_retry_at,id);
CREATE TABLE IF NOT EXISTS reminder_email_items (
    delivery_id INTEGER NOT NULL REFERENCES reminder_email_deliveries(id) ON DELETE CASCADE, -- 所属邮件任务。
    occurrence_id INTEGER NOT NULL UNIQUE REFERENCES reminder_occurrences(id) ON DELETE CASCADE, -- 真实提醒触发ID；唯一约束防止重复登记。
    name TEXT NOT NULL, -- 触发时的提醒名称快照。
    content TEXT NOT NULL, -- 触发时的提醒详情快照。
    scheduled_time TEXT NOT NULL, -- 触发时转换为电脑当地时区的可读时间快照。
    PRIMARY KEY(delivery_id,occurrence_id)
);

CREATE TABLE IF NOT EXISTS pet_artworks (
 id INTEGER PRIMARY KEY AUTOINCREMENT, -- 创作条目主键，AI仅能选择已有正式条目
 persona_id INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE, -- 所属人设，删除人设级联删除条目
 kind TEXT NOT NULL CHECK(kind IN ('base','expression')), -- base唯一初始形象/expression表情
 name TEXT NOT NULL CHECK(length(trim(name))>0), -- 用户可编辑名称
 prompt TEXT NOT NULL DEFAULT '', -- 用户自由填写的图像生成要求
 description TEXT NOT NULL DEFAULT '', -- 给AI参考的使用时机，不作为指令
 category TEXT NOT NULL DEFAULT 'normal' CHECK(category IN ('normal','click','hungry','eating','sleepy','bored','thinking','chat')), -- 状态或聊天分类
 enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)), -- 0不参与表情选择/1允许选择
 image_file TEXT, -- 已接受正式透明PNG受管相对路径，NULL尚无正式图
 preview_file TEXT, -- 尚待用户接受的透明PNG；重新生成失败保留旧预览
 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0), -- 本条内容版本，修改后拒绝迟到生成和旧保存
 created_at TEXT NOT NULL, -- 创建时间RFC3339
 updated_at TEXT NOT NULL -- 最近修改时间RFC3339
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_pet_artworks_base ON pet_artworks(persona_id) WHERE kind='base';
CREATE INDEX IF NOT EXISTS idx_pet_artworks_persona ON pet_artworks(persona_id,id);
