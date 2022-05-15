use super::*;

pub struct SeriesEpisode {
    pub series_name: String,
    pub series_season: String,
    pub series_episode: String,
    pub item_guid: String,
}

pub struct Transaction {
    pub pool: Pool,
    pub episodes: Vec<SeriesEpisode>,
}

impl SeriesEpisode {
    pub async fn exists(&self, tx: &Transaction) -> Result<bool> {
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
        let count: i32 = stmt.fetch_one(&tx.pool).await?;
        Ok(count > 0)
    }

    pub async fn insert(self, tx: &mut Transaction) -> Result<()> {
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
        .execute(&tx.pool)
        .await?;

        tx.episodes.push(self);

        Ok(())
    }
}

impl Transaction {
    pub fn new(pool: Pool) -> Self {
        Transaction {
            pool,
            episodes: Vec::new(),
        }
    }

    pub fn commit(mut self) {
        self.episodes = Vec::new();
    }

    pub async fn rollback(&mut self) -> Result<()> {
        info!("rolling back {} items", self.episodes.len());
        for episode in self.episodes.iter() {
            sqlx::query!(
                r#"
                DELETE FROM `series`
                WHERE
                    `item_guid` = ?
                ;"#,
                episode.item_guid
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if self.episodes.is_empty() {
            return;
        }
        let fut = async {
            if let Err(e) = self.rollback().await {
                error!("Transaction rollback failed: {:?}", e);
            }
        };
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(fut);
        });
    }
}
