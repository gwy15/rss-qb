use anyhow::Result;

pub type Pool = sqlx::SqlitePool;

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

    pub async fn insert(self, pool: &Pool) -> Result<()> {
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
