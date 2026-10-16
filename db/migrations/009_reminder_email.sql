-- 邮件队列是本机运行数据，不导出；旧版升级不补登记历史提醒。
CREATE TABLE reminder_email_deliveries (
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
CREATE INDEX idx_reminder_email_queue ON reminder_email_deliveries(state,next_retry_at,id);
CREATE TABLE reminder_email_items (
    delivery_id INTEGER NOT NULL REFERENCES reminder_email_deliveries(id) ON DELETE CASCADE, -- 所属邮件任务。
    occurrence_id INTEGER NOT NULL UNIQUE REFERENCES reminder_occurrences(id) ON DELETE CASCADE, -- 真实提醒触发ID；唯一约束防止重复登记。
    name TEXT NOT NULL, -- 触发时的提醒名称快照。
    content TEXT NOT NULL, -- 触发时的提醒详情快照。
    scheduled_time TEXT NOT NULL, -- 触发时转换为电脑当地时区的可读时间快照。
    PRIMARY KEY(delivery_id,occurrence_id)
);
