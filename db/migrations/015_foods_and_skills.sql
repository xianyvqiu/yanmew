CREATE TEMP TABLE currency_seq_v15(value INTEGER);
INSERT INTO currency_seq_v15 SELECT seq FROM sqlite_sequence WHERE name='currency_ledger';
CREATE TABLE currency_ledger_v15 (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT, -- 喵币流水自增主键
    delta                 INTEGER NOT NULL, -- 本次余额变化量；正数增加、负数扣除、插件平局可为 0
    reason                TEXT    NOT NULL CHECK (reason IN ('daily_sign_in', 'sign_in_festival', 'plugin_game', 'accessory_craft', 'accessory_sell', 'food_purchase')), -- daily_sign_in=普通签到；sign_in_festival=节日签到；plugin_game=插件游戏；accessory_craft=制作饰品；accessory_sell=卖出饰品；food_purchase=购买食品
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
INSERT INTO currency_ledger_v15(id,delta,reason,ref_id,game_run_id,plugin_id,plugin_version,plugin_display_reason,wallet_request_id,created_at) SELECT id,delta,reason,ref_id,game_run_id,plugin_id,plugin_version,plugin_display_reason,wallet_request_id,created_at FROM currency_ledger;
DROP TABLE currency_ledger;
ALTER TABLE currency_ledger_v15 RENAME TO currency_ledger;
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT MAX(value) FROM currency_seq_v15),0)) WHERE name='currency_ledger';
DROP TABLE currency_seq_v15;

CREATE INDEX IF NOT EXISTS idx_currency_ledger_created ON currency_ledger (created_at);

CREATE INDEX IF NOT EXISTS idx_currency_ledger_game_run ON currency_ledger (game_run_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_currency_ledger_plugin_request
    ON currency_ledger(plugin_id, wallet_request_id)
    WHERE reason = 'plugin_game';
-- 食物和 Skill 属于全局，偏好和绑定属于人设；亲密度继续属于会话。
CREATE TABLE IF NOT EXISTS food_catalog (
 id TEXT PRIMARY KEY, -- 稳定食物代码，不随显示名改变
 name TEXT NOT NULL, -- 食物显示名称
 asset_key TEXT NOT NULL, -- 随包图片资源键
 is_unlimited INTEGER NOT NULL CHECK(is_unlimited IN(0,1)), -- 1免费无限token，0付费有限
 price INTEGER NOT NULL CHECK(price>=0), -- 单份喵币，token为0，其余统一5
 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0), -- 报价版本
 created_at TEXT NOT NULL, -- 上架时间UTC
 updated_at TEXT NOT NULL, -- 最近目录更新UTC
 CHECK((is_unlimited=1 AND id='token' AND price=0) OR (is_unlimited=0 AND price>0))
);
CREATE TABLE IF NOT EXISTS food_inventory (
 food_id TEXT PRIMARY KEY REFERENCES food_catalog(id) ON DELETE RESTRICT, -- 有限食物，禁止无限token行
 quantity INTEGER NOT NULL CHECK(quantity>=0), -- 剩余份数
 updated_at TEXT NOT NULL -- 最近购买或投喂UTC
);
CREATE TABLE IF NOT EXISTS persona_food_settings (
 persona_id INTEGER PRIMARY KEY REFERENCES personas(id) ON DELETE CASCADE, -- 删除人设只删除偏好
 default_food_id TEXT NOT NULL DEFAULT 'token' REFERENCES food_catalog(id) ON DELETE RESTRICT, -- 用户星标默认选择
 favorite_food_id TEXT REFERENCES food_catalog(id) ON DELETE RESTRICT, -- 喜好可修改，NULL没有喜好
 updated_at TEXT NOT NULL -- 最近设置时间UTC
);
CREATE TABLE IF NOT EXISTS food_actions (
 action_id TEXT PRIMARY KEY, -- 一次操作唯一幂等标识
 kind TEXT NOT NULL CHECK(kind IN('purchase','feed')), -- purchase购买/feed投喂，仅成功操作
 food_id TEXT NOT NULL REFERENCES food_catalog(id) ON DELETE RESTRICT, -- 食物代码
 quantity INTEGER NOT NULL CHECK(quantity>0), -- 购买份数或投喂固定1
 persona_id INTEGER REFERENCES personas(id) ON DELETE SET NULL, -- 目标人设，删除后留回执
 session_id INTEGER REFERENCES sessions(id) ON DELETE SET NULL, -- 亲密度所属会话，删除后留回执
 food_name TEXT NOT NULL, -- 食物名称快照
 target_name TEXT NOT NULL DEFAULT '', -- 人设名字快照
 unit_price INTEGER NOT NULL CHECK(unit_price>=0), -- 购买时单价，投喂为0
 ledger_id INTEGER UNIQUE REFERENCES currency_ledger(id) ON DELETE RESTRICT, -- 购买的唯一扣款流水
 favorite_bonus INTEGER NOT NULL DEFAULT 0 CHECK(favorite_bonus IN(0,1)), -- 0未抽中，1抽中喜好额外亲密度
 intimacy_delta INTEGER NOT NULL DEFAULT 0 CHECK(intimacy_delta BETWEEN 0 AND 2), -- 封顶后的实际增加
 created_at TEXT NOT NULL, -- 完成时间UTC
 CHECK((kind='purchase' AND ledger_id IS NOT NULL AND intimacy_delta=0 AND favorite_bonus=0 AND persona_id IS NULL AND session_id IS NULL) OR (kind='feed' AND quantity=1 AND ledger_id IS NULL AND unit_price=0))
);
CREATE TABLE IF NOT EXISTS skills (
 id INTEGER PRIMARY KEY AUTOINCREMENT, -- Skill主键
 title TEXT NOT NULL CHECK(length(trim(title))>0), -- 卡片标题
 summary TEXT NOT NULL DEFAULT '', -- 卡片简介
 body TEXT NOT NULL CHECK(length(trim(body))>0), -- 完整提示词正文
 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0), -- 乐观编辑版本
 created_at TEXT NOT NULL, -- 创建UTC
 updated_at TEXT NOT NULL -- 最近保存UTC
);
CREATE TABLE IF NOT EXISTS persona_skills (
 persona_id INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE, -- 绑定人设
 skill_id INTEGER NOT NULL REFERENCES skills(id) ON DELETE CASCADE, -- 绑定Skill，删除只解除关系
 created_at TEXT NOT NULL, -- 稳定注入顺序UTC
 PRIMARY KEY(persona_id,skill_id)
);
CREATE INDEX IF NOT EXISTS idx_persona_skills_skill ON persona_skills(skill_id);
CREATE TRIGGER IF NOT EXISTS food_inventory_finite_insert BEFORE INSERT ON food_inventory WHEN (SELECT is_unlimited FROM food_catalog WHERE id=NEW.food_id)=1 BEGIN SELECT RAISE(ABORT,'无限食物不保存库存'); END;
CREATE TRIGGER IF NOT EXISTS food_inventory_finite_update BEFORE UPDATE ON food_inventory WHEN (SELECT is_unlimited FROM food_catalog WHERE id=NEW.food_id)=1 BEGIN SELECT RAISE(ABORT,'无限食物不保存库存'); END;

-- 待机表情分类：只扩充 CHECK，保留条目ID、图片、版本、时间及自增高水位。
CREATE TEMP TABLE pet_artworks_sequence_v15(value INTEGER); -- 原表自增高水位，避免重建后复用已删除ID
INSERT INTO pet_artworks_sequence_v15 SELECT seq FROM sqlite_sequence WHERE name='pet_artworks';
CREATE TABLE pet_artworks_v15 (
 id INTEGER PRIMARY KEY AUTOINCREMENT, -- 创作条目主键，AI仅能选择已有正式条目
 persona_id INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE, -- 所属人设，删除人设级联删除条目
 kind TEXT NOT NULL CHECK(kind IN ('base','expression')), -- base唯一初始形象/expression表情
 name TEXT NOT NULL CHECK(length(trim(name))>0), -- 用户可编辑名称
 prompt TEXT NOT NULL DEFAULT '', -- 用户自由填写的图像生成要求
 description TEXT NOT NULL DEFAULT '', -- 给AI参考的使用时机，不作为指令
 category TEXT NOT NULL DEFAULT 'normal' CHECK(category IN ('normal','standby','click','hungry','eating','sleepy','bored','thinking','chat','craving','craving_favorite','eating_favorite')), -- normal=默认，standby=待机，click=点击，hungry=饥饿，eating=进食，sleepy=困倦，bored=无聊，thinking=思考，chat=聊天，craving=等待投喂，craving_favorite=期待最爱，eating_favorite=喜欢地品尝
 enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)), -- 0不参与表情选择/1允许选择
 image_file TEXT, -- 已接受正式透明PNG受管相对路径，NULL尚无正式图
 preview_file TEXT, -- 尚待用户接受的透明PNG；重新生成失败保留旧预览
 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0), -- 本条内容版本，修改后拒绝迟到生成和旧保存
 created_at TEXT NOT NULL, -- 创建时间RFC3339
 updated_at TEXT NOT NULL -- 最近修改时间RFC3339
);
INSERT INTO pet_artworks_v15(id,persona_id,kind,name,prompt,description,category,enabled,image_file,preview_file,revision,created_at,updated_at) SELECT id,persona_id,kind,name,prompt,description,category,enabled,image_file,preview_file,revision,created_at,updated_at FROM pet_artworks;
DROP TABLE pet_artworks;
ALTER TABLE pet_artworks_v15 RENAME TO pet_artworks;
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT MAX(value) FROM pet_artworks_sequence_v15),0)) WHERE name='pet_artworks';
DROP TABLE pet_artworks_sequence_v15;
CREATE UNIQUE INDEX idx_pet_artworks_base ON pet_artworks(persona_id) WHERE kind='base';
CREATE INDEX idx_pet_artworks_persona ON pet_artworks(persona_id,id);

-- 喜好来自可购买食品；基础token仅作为免费通用食物。
CREATE TRIGGER IF NOT EXISTS persona_food_favorite_insert BEFORE INSERT ON persona_food_settings WHEN NEW.favorite_food_id IS NOT NULL AND (SELECT is_unlimited FROM food_catalog WHERE id=NEW.favorite_food_id)=1 BEGIN SELECT RAISE(ABORT,'喜好须为商城食品'); END;
CREATE TRIGGER IF NOT EXISTS persona_food_favorite_update BEFORE UPDATE ON persona_food_settings WHEN NEW.favorite_food_id IS NOT NULL AND (SELECT is_unlimited FROM food_catalog WHERE id=NEW.favorite_food_id)=1 BEGIN SELECT RAISE(ABORT,'喜好须为商城食品'); END;
