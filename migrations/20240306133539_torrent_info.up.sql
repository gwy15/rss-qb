-- Add up migration script here
CREATE TABLE `torrent_info` (
    `id`            INTEGER NOT NULL PRIMARY KEY,
    `name`          TEXT    NOT NULL,
    `year`          INTEGER NOT NULL,
    `season`        INTEGER NOT NULL,
    `episode`       INTEGER NOT NULL,
    `fansub`        TEXT    NOT NULL,
    `resolution`    TEXT    NOT NULL,
    `language`      TEXT    NOT NULL,
    `tmdb_id`       INTEGER NOT NULL
);
