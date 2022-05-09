-- Add up migration script here
CREATE TABLE `series` (
    `id`            INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    `series_name`   TEXT    NOT NULL,
    `series_season` INTEGER NOT NULL,
    `series_episode`INTEGER NOT NULL,
    `item_guid`     TEXT    NOT NULL
);
CREATE INDEX `idx_series_name` ON `series` (`series_name`);
