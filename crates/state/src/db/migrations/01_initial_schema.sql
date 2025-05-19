-- Initial schema for state storage

-- State entries table
CREATE TABLE IF NOT EXISTS state_entries (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- State roots table for tracking the history of state roots
CREATE TABLE IF NOT EXISTS state_roots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    root_hash TEXT NOT NULL,
    transaction_hash TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    UNIQUE(root_hash)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_state_entries_updated_at ON state_entries(updated_at);
CREATE INDEX IF NOT EXISTS idx_state_roots_created_at ON state_roots(created_at);

-- State diffs table for tracking changes between states
CREATE TABLE IF NOT EXISTS state_diffs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    prev_root_hash TEXT NOT NULL,
    new_root_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY(prev_root_hash) REFERENCES state_roots(root_hash),
    FOREIGN KEY(new_root_hash) REFERENCES state_roots(root_hash)
);

-- State operations table for tracking individual operations in a diff
CREATE TABLE IF NOT EXISTS state_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    diff_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL, -- 'insert' or 'delete'
    key TEXT NOT NULL,
    value TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY(diff_id) REFERENCES state_diffs(id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_state_diffs_roots ON state_diffs(prev_root_hash, new_root_hash);
CREATE INDEX IF NOT EXISTS idx_state_operations_diff_id ON state_operations(diff_id);
