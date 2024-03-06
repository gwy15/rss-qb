-- Add up migration script here
CREATE TABLE `tmdb_info` (
    `id`            INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    `name`          TEXT    NOT NULL,
    `tmdb_name`     TEXT    NOT NULL,
    `year`          INTEGER NOT NULL
);
CREATE INDEX `idx_tmdb_map` ON `tmdb_info`(`name`)
