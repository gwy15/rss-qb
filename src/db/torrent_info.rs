use super::*;

#[derive(Debug)]
pub struct TorrentInfo {
    pub id: i64,
    pub name: String,
    pub year: i64,
    pub season: i64,
    pub episode: i64,
    pub fansub: String,
    pub resolution: String,
    pub language: String,
    pub tmdb_id: i64,
}

impl TorrentInfo {
    pub fn gen_id() -> i64 {
        use rand::prelude::*;
        thread_rng().gen_range(1..=i64::MAX)
    }

    pub async fn from_id(id: i64, pool: &Pool) -> Result<Self> {
        let s = sqlx::query_as!(
            Self,
            r"SELECT
                `id`, `name`, `year`, `season`, `episode`, `fansub`, `resolution`, `language`, `tmdb_id`
            FROM
                `torrent_info`
            WHERE
                `id` = ?
            LIMIT
                1;",
            id
        )
        .fetch_one(pool)
        .await?;
        Ok(s)
    }

    pub async fn insert(self, pool: &Pool) -> Result<()> {
        sqlx::query!(
            r"INSERT INTO `torrent_info`
                (`id`, `name`, `year`, `season`, `episode`, `fansub`, `resolution`, `language`, `tmdb_id`)
            VALUES
                (?, ?, ?, ?, ?, ?, ?, ?, ?);
            ",
            self.id,
            self.name,
            self.year,
            self.season,
            self.episode,
            self.fansub,
            self.resolution,
            self.language,
            self.tmdb_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
