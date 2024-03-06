use super::*;

#[derive(Debug)]
pub struct TmdbShow {
    pub tmdb_id: i64,
    pub tmdb_name: String,
    pub year: i64,
}

impl TmdbShow {
    pub async fn from_name(name: &str, pool: &Pool) -> Result<Option<Self>> {
        let ans = sqlx::query_as!(
            Self,
            r"SELECT
                `tmdb_name`, `year`, `tmdb_id`
            FROM
                `tmdb_info`
            WHERE
                `name` = ?
            LIMIT 1;",
            name
        )
        .fetch_optional(pool)
        .await?;

        Ok(ans)
    }
    pub async fn insert_with(&self, name: &str, pool: &Pool) -> Result<()> {
        sqlx::query!(
            r"INSERT INTO `tmdb_info`
                (`name`, `tmdb_name`, `year`, `tmdb_id`)
            VALUES
                (?, ?, ?, ?);",
            name,
            self.tmdb_name,
            self.year,
            self.tmdb_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
