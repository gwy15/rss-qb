CREATE TABLE `items` (
    `guid`          TEXT    NOT NULL UNIQUE PRIMARY KEY,
    `title`         TEXT    NOT NULL,
    `link`          TEXT    NOT NULL,
    `enclosure`     TEXT    NOT NULL
);