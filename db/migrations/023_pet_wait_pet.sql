-- 等待摸摸动作类别：仅扩充CHECK，保留旧条目、图片、修订及自增高水位。
CREATE TEMP TABLE pet_artworks_sequence_v23(value INTEGER); -- 原表自增高水位，避免重建后复用已删除ID
INSERT INTO pet_artworks_sequence_v23 SELECT seq FROM sqlite_sequence WHERE name='pet_artworks';
CREATE TABLE pet_artworks_v23 (
 id INTEGER PRIMARY KEY AUTOINCREMENT, -- 创作条目主键，AI仅能选择已有正式条目
 persona_id INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE, -- 所属人设，删除人设级联删除条目
 kind TEXT NOT NULL CHECK(kind IN ('base','expression')), -- base唯一初始形象/expression表情
 name TEXT NOT NULL CHECK(length(trim(name))>0), -- 用户可编辑名称
 prompt TEXT NOT NULL DEFAULT '', -- 用户自由填写的图像生成要求
 description TEXT NOT NULL DEFAULT '', -- 给AI参考的使用时机，不作为指令
 category TEXT NOT NULL DEFAULT 'normal' CHECK(category IN ('normal','standby','click','hungry','eating','sleepy','bored','thinking','chat','craving','craving_favorite','eating_favorite','peek_left','peek_right','rest_sit','rest_lie','walk_left','walk_right','lifted','error','toss_up','toss_down','landing','wait_pet')), -- normal=默认，standby=待机，click=点击，hungry=饥饿，eating=进食，sleepy=困倦，bored=无聊，thinking=思考，chat=聊天，craving=等待投喂，craving_favorite=期待最爱，eating_favorite=喜好品尝，peek_left=左侧探头，peek_right=右侧探头，rest_sit=底部坐下，rest_lie=底部趴下，walk_left=向左行走，walk_right=向右行走
 enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0,1)), -- 0不参与表情选择/1允许选择
 image_file TEXT, -- 已接受正式透明PNG受管相对路径，NULL尚无正式图
 preview_file TEXT, -- 尚待用户接受的透明PNG；重新生成失败保留旧预览
 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0), -- 本条内容版本，修改后拒绝迟到生成和旧保存
 created_at TEXT NOT NULL, -- 创建时间RFC3339
 updated_at TEXT NOT NULL -- 最近修改时间RFC3339
);
INSERT INTO pet_artworks_v23(id,persona_id,kind,name,prompt,description,category,enabled,image_file,preview_file,revision,created_at,updated_at) SELECT id,persona_id,kind,name,prompt,description,category,enabled,image_file,preview_file,revision,created_at,updated_at FROM pet_artworks;
DROP TABLE pet_artworks;
ALTER TABLE pet_artworks_v23 RENAME TO pet_artworks;
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT MAX(value) FROM pet_artworks_sequence_v23),0)) WHERE name='pet_artworks';
DROP TABLE pet_artworks_sequence_v23;
CREATE UNIQUE INDEX idx_pet_artworks_base ON pet_artworks(persona_id) WHERE kind='base';
CREATE INDEX idx_pet_artworks_persona ON pet_artworks(persona_id,id);
