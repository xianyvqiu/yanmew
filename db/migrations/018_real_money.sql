-- 现实记账独立于喵币；金额为人民币分，不关联聊天、人设或游戏数据。
CREATE TABLE IF NOT EXISTS real_assets (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 name TEXT NOT NULL CHECK(length(trim(name))>0), note TEXT NOT NULL DEFAULT '',
 is_default INTEGER NOT NULL DEFAULT 0 CHECK(is_default IN (0,1)),
 archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
 revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 CHECK(is_default=0 OR archived=0)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_real_default_asset ON real_assets(is_default) WHERE is_default=1;
CREATE TABLE IF NOT EXISTS real_categories (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 name TEXT NOT NULL CHECK(length(trim(name))>0), icon TEXT NOT NULL DEFAULT '🧾',
 direction TEXT NOT NULL DEFAULT 'both' CHECK(direction IN ('income','expense','both')),
 archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)),
 revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_real_category_name ON real_categories(name COLLATE NOCASE) WHERE archived=0;
CREATE TABLE IF NOT EXISTS real_schedules (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 title TEXT NOT NULL CHECK(length(trim(title))>0), kind TEXT NOT NULL CHECK(kind IN ('income','expense')),
 category_id INTEGER NOT NULL REFERENCES real_categories(id) ON DELETE RESTRICT,
 asset_id INTEGER NOT NULL REFERENCES real_assets(id) ON DELETE RESTRICT,
 description TEXT NOT NULL DEFAULT '', amount_cents INTEGER NOT NULL CHECK(typeof(amount_cents)='integer' AND amount_cents>0),
 anchor_local TEXT NOT NULL,
 unit TEXT NOT NULL CHECK(unit IN ('day','week','month','year')),
 interval INTEGER NOT NULL CHECK(interval>0), generation INTEGER NOT NULL DEFAULT 1 CHECK(generation>0), next_sequence INTEGER NOT NULL DEFAULT 0 CHECK(next_sequence>=0),
 status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','paused','ended')),
 review_required INTEGER NOT NULL DEFAULT 0 CHECK(review_required IN (0,1)),
 recorder TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS real_entries (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 title TEXT NOT NULL CHECK(length(trim(title))>0), kind TEXT NOT NULL CHECK(kind IN ('income','expense','adjustment')),
 category_id INTEGER REFERENCES real_categories(id) ON DELETE RESTRICT,
 asset_id INTEGER NOT NULL REFERENCES real_assets(id) ON DELETE RESTRICT,
 description TEXT NOT NULL DEFAULT '', amount_cents INTEGER NOT NULL CHECK(typeof(amount_cents)='integer'),
 occurred_local TEXT NOT NULL,
 source TEXT NOT NULL CHECK(source IN ('manual','ai','cycle','adjustment')),
 recorder TEXT NOT NULL,
 schedule_id INTEGER REFERENCES real_schedules(id) ON DELETE SET NULL,
 revision INTEGER NOT NULL DEFAULT 1 CHECK(revision>0),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 CHECK((kind IN ('income','expense') AND amount_cents>0 AND category_id IS NOT NULL) OR (kind='adjustment' AND category_id IS NULL))
);
CREATE INDEX IF NOT EXISTS idx_real_entry_date ON real_entries(occurred_local,id);
CREATE INDEX IF NOT EXISTS idx_real_entry_asset ON real_entries(asset_id);
CREATE TABLE IF NOT EXISTS real_occurrences (
 id INTEGER PRIMARY KEY AUTOINCREMENT,
 schedule_id INTEGER NOT NULL REFERENCES real_schedules(id) ON DELETE RESTRICT,
 sequence INTEGER NOT NULL CHECK(sequence>=0), generation INTEGER NOT NULL DEFAULT 1 CHECK(generation>0), scheduled_local TEXT NOT NULL,
 snapshot_json TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('pending','recorded','skipped')),
 entry_id INTEGER UNIQUE REFERENCES real_entries(id) ON DELETE SET NULL,
 created_at TEXT NOT NULL, resolved_at TEXT,
 UNIQUE(schedule_id,generation,sequence)
);
CREATE INDEX IF NOT EXISTS idx_real_occurrence_pending ON real_occurrences(status,id);
CREATE TABLE IF NOT EXISTS real_budgets (
 period TEXT PRIMARY KEY CHECK(period IN ('day','month','year')),
 amount_cents INTEGER NOT NULL CHECK(typeof(amount_cents)='integer' AND amount_cents>=0), updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS real_operations (
 request_id TEXT PRIMARY KEY, fingerprint TEXT NOT NULL, result_json TEXT NOT NULL, created_at TEXT NOT NULL
);
