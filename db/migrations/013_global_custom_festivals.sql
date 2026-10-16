-- v13：自定义日期改为全局；逐条保留 ID、内容、创建来源与时间，不合并同名记录。
-- 在初始化的 IMMEDIATE 事务中执行；删除会话不再删除这些日期。
CREATE TABLE IF NOT EXISTS custom_festivals (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,   -- 自增主键
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
    FOREIGN KEY (festival_id) REFERENCES festival_catalog(id) ON DELETE SET NULL
);
INSERT INTO custom_festivals(id,festival_id,kind,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,blessing,note,source,created_at,updated_at) SELECT id,festival_id,kind,name,year,calendar_type,solar_month,solar_day,lunar_month,lunar_day,is_leap_month,blessing,note,source,created_at,updated_at FROM session_festivals;
-- 保留已删除记录留下的自增水位，后续新增不会复用旧 ID。
UPDATE sqlite_sequence SET seq=MAX(seq,COALESCE((SELECT seq FROM sqlite_sequence WHERE name='session_festivals'),0)) WHERE name='custom_festivals';
INSERT INTO sqlite_sequence(name,seq) SELECT 'custom_festivals',seq FROM sqlite_sequence WHERE name='session_festivals' AND NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='custom_festivals');
DROP TABLE session_festivals;
CREATE INDEX idx_custom_festivals_created ON custom_festivals(created_at,id);
