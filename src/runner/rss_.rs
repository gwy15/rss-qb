use crate::config::RssFeed;
use crate::db;
use crate::QbClient;
use anyhow::bail;
use anyhow::{Context, Result};
use url::Url;

impl RssFeed {
    /// 返回成功的 item
    pub async fn run(
        &self,
        qb_client: &QbClient,
        request_client: &reqwest::Client,
        pool: &db::Pool,
        config: &crate::Config,
    ) -> Result<Vec<db::Item>> {
        info!("Fetching feed {}", self.name());
        let items = self
            .get_items(&request_client)
            .await
            .context("get torrents from url failed")?;
        // 过滤已经添加的，从这里开始有 DB 操作

        let mut to_add_items = vec![];
        for item in items.into_iter().filter(|i| self.base.filter(i)) {
            if db::Item::exists(&item.guid, pool).await? {
                debug!("item {} already exists in DB, skip", item.title);
                continue;
            }
            to_add_items.push(item);
        }

        if to_add_items.is_empty() {
            return Ok(vec![]);
        }
        // 提取剧集信息
        let titles = to_add_items
            .iter()
            .map(|s| s.title.clone())
            .collect::<Vec<_>>();
        let info = crate::gpt::get_episode_info(&titles, &config.gpt).await?;
        if info.len() != to_add_items.len() {
            bail!(
                "gpt 提取的 length 不一致，to_add_items.len = {}, gpt_extract.len = {}",
                to_add_items.len(),
                info.len()
            );
        }

        let mut tx = db::EpTransaction::new(pool.clone());
        qb_client.login().await?;
        for (item, info) in to_add_items.iter().zip(info) {
            let ep = db::SeriesEpisode {
                series_name: self.name().to_string(),
                series_season: info.season.clone(),
                series_episode: info.episode.clone(),
                item_guid: item.guid.to_string(),
            };
            if ep.exists(&tx).await? {
                debug!("item {} already exists in series, skip", item.title);
                continue;
            }
            info!("series {} new episode {info:?}", self.name());
            qb_client
                .add_torrent(crate::request::AddTorrentRequest {
                    urls: vec![item.enclosure.clone()],
                    torrents: vec![],
                    savepath: self.base.savepath.clone(),
                    content_layout: self.base.content_layout.map(|i| i.to_string()),
                    category: self.base.category.clone(),
                    tags: self.base.tags.clone(),
                    rename: Some(format!(
                        "{anime} - S{season}E{ep} - {resolution} - {language} - {fansub}",
                        anime = info.anime,
                        season = info.season,
                        ep = info.episode,
                        resolution = info.resolution,
                        language = info.language,
                        fansub = info.fansub
                    )),
                    auto_torrent_management: Some(self.base.auto_torrent_management),
                })
                .await
                .context("add torrent failed")?;
            info!("种子 {} {info:?} 成功添加到 QB", item.title);

            ep.insert(&mut tx).await?;
            item.insert(pool).await?;
        }

        tx.commit();
        Ok(to_add_items)
    }

    async fn get_items(&self, client: &reqwest::Client) -> Result<Vec<db::Item>> {
        let url = self.url();
        let r = client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("request {} failed", self.name()))?;
        let status = r.status();
        if !status.is_success() {
            debug!("feed={}, get url {url} failed: {status:?}", self.name());
            debug!(
                "feed={}, url={url}, text: {:?}",
                self.name(),
                r.text().await.ok()
            );
            bail!("feed '{}' HTTP status is '{status}'", self.name());
        }
        let s = r.bytes().await.context("failed to fetch body")?;
        let channel = rss::Channel::read_from(&s[..]).context("failed to parse as rss channel")?;
        info!(
            "feed {} (channel name {}) fetched {} items",
            self.name(),
            channel.title,
            channel.items.len()
        );

        let mut items = vec![];
        for item in channel.items.into_iter() {
            let item: db::Item = item.try_into()?;
            items.push(item);
        }
        Ok(items)
    }

    fn url(&self) -> Url {
        use crate::config::RssSite;
        match self.site {
            RssSite::Comicat => {
                let mut url = "https://comicat.org/".parse::<Url>().unwrap();
                url.path_segments_mut()
                    .unwrap()
                    .push(&format!("rss-{}.xml", self.search));
                url
            }
            RssSite::Dmhy => {
                let mut url = "https://www.dmhy.org/topics/rss/rss.xml"
                    .parse::<Url>()
                    .unwrap();
                url.query_pairs_mut().append_pair("keyword", &self.search);
                url
            }
        }
    }
}

impl TryFrom<rss::Item> for db::Item {
    type Error = anyhow::Error;
    fn try_from(value: rss::Item) -> Result<Self, Self::Error> {
        let enclosure = value.enclosure.context("missing enclosure")?;
        let enclosure = enclosure.url;
        Ok(db::Item {
            guid: value.guid.context("missing guid")?.value,
            title: value.title.unwrap_or_else(|| "unknown".to_string()),
            link: value.link.unwrap_or_else(|| "unknown".to_string()),
            enclosure,
        })
    }
}
