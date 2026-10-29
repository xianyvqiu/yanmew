-- 全局聊天表情包，与桌宠动作图片独立。
CREATE TABLE sticker_libraries (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 title TEXT NOT NULL CHECK(length(trim(title))>0),
 description TEXT NOT NULL DEFAULT '',
 revision INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE TABLE chat_stickers (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 library_id INTEGER NOT NULL REFERENCES sticker_libraries(id) ON DELETE CASCADE,
 title TEXT NOT NULL CHECK(length(trim(title))>0),
 description TEXT NOT NULL DEFAULT '',
 filename TEXT NOT NULL,
 image_file TEXT NOT NULL,
 media_type TEXT NOT NULL CHECK(media_type IN ('image/png','image/jpeg','image/webp','image/gif')),
 revision INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE INDEX idx_chat_stickers_library ON chat_stickers(library_id,id);
CREATE TABLE sticker_tags (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 name TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK(length(trim(name))>0),
 revision INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE TABLE sticker_tag_links (
 sticker_id INTEGER NOT NULL REFERENCES chat_stickers(id) ON DELETE CASCADE,
 tag_id INTEGER NOT NULL REFERENCES sticker_tags(id) ON DELETE CASCADE,
 PRIMARY KEY(sticker_id,tag_id)
);
CREATE INDEX idx_sticker_tags_lookup ON sticker_tag_links(tag_id,sticker_id);
CREATE TABLE persona_sticker_libraries (
 persona_id INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
 library_id INTEGER NOT NULL REFERENCES sticker_libraries(id) ON DELETE CASCADE,
 created_at TEXT NOT NULL,
 PRIMARY KEY(persona_id,library_id)
);
CREATE INDEX idx_sticker_library_personas ON persona_sticker_libraries(library_id,persona_id);
CREATE TEMP TABLE sticker_migration_sequence(value INTEGER);
INSERT INTO sticker_migration_sequence SELECT seq FROM sqlite_sequence WHERE name='message_images';
CREATE TABLE message_images_v26 (
    id INTEGER PRIMARY KEY AUTOINCREMENT, -- 附件主键
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE, -- 所属用户消息；删除消息级联清引用
    filename TEXT NOT NULL, -- 用户原文件名，仅用于展示，不作为文件路径
    image_file TEXT NOT NULL, -- chat_images目录内的受管文件名
    media_type TEXT NOT NULL CHECK (media_type IN ('image/png','image/jpeg','image/webp','image/gif')), -- 实际格式：image/png / image/jpeg / image/webp
    source_kind TEXT NOT NULL DEFAULT 'image' CHECK(source_kind IN ('image','sticker')),
    caption TEXT, -- 已生成的图片理解事实；NULL表示尚无理解结果
    created_at TEXT NOT NULL -- 保存UTC时间
);

INSERT INTO message_images_v26(id,message_id,filename,image_file,media_type,caption,created_at)
 SELECT id,message_id,filename,image_file,media_type,caption,created_at FROM message_images;
DROP TABLE message_images;
ALTER TABLE message_images_v26 RENAME TO message_images;
CREATE INDEX idx_message_images_message ON message_images(message_id,id);
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT MAX(value) FROM sticker_migration_sequence),0)) WHERE name='message_images';
DROP TABLE sticker_migration_sequence;
