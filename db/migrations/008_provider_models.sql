-- v7→v8：连接只存连接资料；所有旧连接的聊天模型及预算保留为独立配置。
CREATE TABLE provider_models (
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE, -- 所属通用连接
    purpose TEXT NOT NULL CHECK (purpose IN ('chat','memory','image','vision')), -- chat聊天/memory记忆/image生图/vision识图
    model TEXT NOT NULL CHECK (length(trim(model))>0), -- 该连接在此用途的模型名
    max_output_tokens INTEGER NOT NULL DEFAULT 32768 CHECK (max_output_tokens>0), -- 聊天最大输出；保留旧连接的原值
    context_window_override INTEGER CHECK (context_window_override>0), -- NULL自动；保留旧连接的容量覆盖
    PRIMARY KEY(provider_id,purpose)
);
INSERT INTO provider_models(provider_id,purpose,model,max_output_tokens,context_window_override)
SELECT id,'chat',model,max_output_tokens,context_window_override FROM providers;
INSERT INTO provider_models(provider_id,purpose,model)
SELECT provider_id,purpose,model FROM provider_roles WHERE purpose<>'chat';
UPDATE provider_models SET model=(SELECT r.model FROM provider_roles r WHERE r.purpose='chat' AND r.provider_id=provider_models.provider_id)
WHERE purpose='chat' AND EXISTS(SELECT 1 FROM provider_roles r WHERE r.purpose='chat' AND r.provider_id=provider_models.provider_id);
CREATE TABLE provider_roles_v8 (
    purpose TEXT PRIMARY KEY CHECK (purpose IN ('chat','memory','image','vision')), -- chat聊天/memory记忆/image生图/vision识图
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE RESTRICT, -- 当前连接
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0,1)), -- 生图0关闭/1开启；其他用途1
    FOREIGN KEY(provider_id,purpose) REFERENCES provider_models(provider_id,purpose) ON DELETE RESTRICT
);
INSERT INTO provider_roles_v8(purpose,provider_id,enabled) SELECT purpose,provider_id,enabled FROM provider_roles;
DROP TABLE provider_roles;
ALTER TABLE provider_roles_v8 RENAME TO provider_roles;
DROP INDEX idx_providers_one_active;
DROP INDEX idx_providers_one_memory_active;
ALTER TABLE providers DROP COLUMN model;
ALTER TABLE providers DROP COLUMN is_active;
ALTER TABLE providers DROP COLUMN memory_is_active;
ALTER TABLE providers DROP COLUMN max_output_tokens;
ALTER TABLE providers DROP COLUMN context_window_override;
CREATE VIEW provider_effective AS
SELECT p.id,p.name,p.base_url,p.api_key_encrypted,
       COALESCE(m.model,'未配置') AS model,
       EXISTS(SELECT 1 FROM provider_roles r WHERE r.purpose='chat' AND r.provider_id=p.id) AS is_active,
       EXISTS(SELECT 1 FROM provider_roles r WHERE r.purpose='memory' AND r.provider_id=p.id) AS memory_is_active,
       COALESCE(m.max_output_tokens,32768) AS max_output_tokens,
       m.context_window_override,p.created_at,p.updated_at
FROM providers p LEFT JOIN provider_models m ON m.provider_id=p.id AND m.purpose='chat';
