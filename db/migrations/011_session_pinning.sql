-- 会话列表置顶状态；旧会话默认不置顶，不改变聊天更新时间或当前选择。
ALTER TABLE sessions ADD COLUMN is_pinned INTEGER NOT NULL DEFAULT 0
    CHECK (is_pinned IN (0, 1)); -- 0=普通会话，1=置顶会话；取消置顶恢复为0。
