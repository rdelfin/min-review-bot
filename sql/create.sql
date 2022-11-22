CREATE TABLE IF NOT EXISTS last_checked_change (
    pr_id UNSIGNED INTEGER NOT NULL PRIMARY KEY,
    last_updated_unixus INTEGER NOT NULL
);
