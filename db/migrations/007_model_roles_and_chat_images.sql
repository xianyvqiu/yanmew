-- v6→v7：统一用途、卡片排序时间和消息图片。由既有升级事务执行。
ALTER TABLE providers ADD COLUMN created_at TEXT; -- 真实创建UTC时间；旧数据未知为NULL
ALTER TABLE providers ADD COLUMN updated_at TEXT; -- 最近编辑UTC时间；旧数据未知为NULL
ALTER TABLE personas ADD COLUMN created_at TEXT; -- 真实创建UTC时间；旧数据未知为NULL
ALTER TABLE personas ADD COLUMN updated_at TEXT; -- 最近编辑UTC时间；旧数据未知为NULL
CREATE TABLE provider_roles (
    purpose TEXT PRIMARY KEY CHECK (purpose IN ('chat','memory','image','vision')), -- 用途；记忆/识图无行表示跟随聊天
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE RESTRICT, -- 引用统一连接；使用中不能删除
    model TEXT NOT NULL CHECK (length(trim(model))>0), -- 该用途使用的模型名
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)) -- 生图开关；其他用途保持1
);
INSERT INTO provider_roles(purpose,provider_id,model) SELECT 'chat',id,model FROM providers WHERE is_active=1;
INSERT INTO provider_roles(purpose,provider_id,model) SELECT 'memory',id,model FROM providers WHERE memory_is_active=1;
CREATE TABLE message_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT, -- 附件主键
    message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE, -- 所属用户消息；删除消息级联清引用
    filename TEXT NOT NULL, -- 用户原文件名，仅用于展示，不作为文件路径
    image_file TEXT NOT NULL, -- chat_images目录内的受管文件名
    media_type TEXT NOT NULL CHECK (media_type IN ('image/png','image/jpeg','image/webp')), -- 实际识别格式
    caption TEXT, -- 已生成的图片理解事实；NULL表示尚无理解结果
    created_at TEXT NOT NULL -- 保存UTC时间
);
CREATE INDEX idx_message_images_message ON message_images(message_id,id);
