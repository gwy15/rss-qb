use std::collections::{HashMap, HashSet};

use crate::db::{self, Pool};
use anyhow::Result;
use tmdb_api::prelude::Command as _;

pub async fn get_info(
    client: reqwest::Client,
    secret: &str,
    titles: HashSet<String>,
    pool: &Pool,
) -> Result<HashMap<String, db::TmdbShow>> {
    if titles.is_empty() {
        return Ok(Default::default());
    }
    // 去重
    let client = tmdb_api::client::ClientBuilder::default()
        .with_base_url("https://api.themoviedb.org/3")
        .with_reqwest_client(client.clone())
        .with_api_key(secret.to_string())
        .build()?;
    let mut futures = vec![];
    for title in titles {
        let fut = async {
            if let Some(known) = db::TmdbShow::from_name(&title, pool).await? {
                return Ok((title, known));
            }
            if let Some(tmdb) = search_tmdb(&title, &client).await? {
                tmdb.insert_with(&title, pool).await?;
                return Ok((title, tmdb));
            }
            anyhow::bail!("unknown tmdb entry")
        };
        futures.push(fut);
    }
    let ans = futures::future::try_join_all(futures).await?;
    let ans = ans.into_iter().collect();
    Ok(ans)
}

async fn search_tmdb(title: &str, client: &tmdb_api::Client) -> Result<Option<db::TmdbShow>> {
    use chrono::Datelike;
    let cmd = tmdb_api::tvshow::search::TVShowSearch::new(title.to_string())
        .with_language(Some("zh-CN".to_string()))
        .with_include_adult(true);
    let result = cmd.execute(&client).await;
    let mut r = match result {
        Ok(r) => r,
        Err(e) => anyhow::bail!("search tv show failed: {:?}", e),
    };
    if r.results.is_empty() {
        return Ok(None);
    }
    let result = r.results.swap_remove(0);
    Ok(Some(db::TmdbShow {
        tmdb_name: result.inner.name,
        year: result.inner.first_air_date.unwrap_or_default().year() as i64,
    }))
}
