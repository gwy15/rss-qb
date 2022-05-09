use anyhow::Result;

pub type Pool = sqlx::SqlitePool;
pub type Tx<'a> = sqlx::Transaction<'a, sqlx::Sqlite>;

#[derive(Debug)]
pub struct Item {
    pub guid: String,
    pub title: String,
    pub link: String,
    pub enclosure: String,
}

impl Item {
    pub async fn exists(guid: &str, pool: &Pool) -> Result<bool> {
        let stmt = sqlx::query_scalar!("SELECT COUNT(*) FROM items WHERE guid = ?;", guid);
        let count: i32 = stmt.fetch_one(pool).await?;
        Ok(count > 0)
    }

    pub async fn insert(&self, pool: &Pool) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO items
                (guid, title, link, enclosure)
            VALUES
                (?, ?, ?, ?);
            "#,
            self.guid,
            self.title,
            self.link,
            self.enclosure
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

pub struct SeriesEpisode<'a> {
    pub series_name: &'a str,
    pub series_season: &'a str,
    pub series_episode: &'a str,
    pub item_guid: &'a str,
}

impl<'a> SeriesEpisode<'a> {
    pub async fn exists<'x, 't>(&self, tx: &'t mut Tx<'x>) -> Result<bool> {
        let stmt = sqlx::query_scalar!(
            r#"
                SELECT COUNT(*)
                FROM `series`
                WHERE
                    `series_name` = ?
                    AND series_season = ? 
                    AND series_episode = ?
                ;"#,
            self.series_name,
            self.series_season,
            self.series_episode
        );
        let count: i32 = stmt.fetch_one(tx).await?;
        Ok(count > 0)
    }

    pub async fn insert<'x, 't>(&self, tx: &'t mut Tx<'x>) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO `series`
                (`series_name`, `series_season`, `series_episode`, `item_guid`)
            VALUES
                (?, ?, ?, ?);
            "#,
            self.series_name,
            self.series_season,
            self.series_episode,
            self.item_guid
        )
        .execute(tx)
        .await?;
        Ok(())
    }
}
