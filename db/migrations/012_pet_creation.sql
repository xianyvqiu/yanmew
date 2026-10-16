-- 图片桌宠创作：按人设开关，旧人设保持颜文字模式。
ALTER TABLE personas ADD COLUMN pet_image_enabled INTEGER NOT NULL DEFAULT 0 CHECK(pet_image_enabled IN (0,1)); -- 0颜文字/1图片模式，开启前要求正式初始图
ALTER TABLE personas ADD COLUMN pet_image_revision INTEGER NOT NULL DEFAULT 0 CHECK(pet_image_revision>=0); -- 创作库版本，淘汰旧请求
ALTER TABLE accessories ADD COLUMN position_x INTEGER CHECK(position_x BETWEEN -1500 AND 1500); -- 饰品中心相对桌宠中心X，桌宠宽度的千分比；NULL沿用头顶默认
ALTER TABLE accessories ADD COLUMN position_y INTEGER CHECK(position_y BETWEEN -1500 AND 1500); -- 饰品中心相对桌宠中心Y，桌宠高度的千分比；NULL沿用头顶默认
ALTER TABLE accessories ADD COLUMN scale INTEGER NOT NULL DEFAULT 1000 CHECK(scale BETWEEN 200 AND 3000); -- 饰品大小相对默认尺寸的千分比，1000原大小
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
