use std::collections::HashSet;

use crate::config::RssFeed;
use crate::db;
use crate::gpt;
use crate::tmdb;
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
        info!("Fetching feed {}", self.name);
        let items = self
            .get_items(request_client)
            .await
            .context("get torrents from url failed")?;

        // 过滤处理过的
        let mut filtered_items = vec![];
        for item in items.into_iter().filter(|i| self.base.filter(i)) {
            if db::Item::exists(&item.guid, pool).await? {
                debug!("item {} already exists in DB, skip", item.title);
                continue;
            }
            filtered_items.push(item);
        }
        let items = filtered_items;
        if items.is_empty() {
            return Ok(vec![]);
        }

        // GPT 提取并过滤剧集信息
        let titles = items.iter().map(|s| s.title.clone()).collect::<Vec<_>>();
        let info = crate::gpt::get_episode_info(&titles, &config.gpt).await?;
        let mut items = items
            .into_iter()
            .zip(info)
            .filter_map(|(item, info)| {
                let gpt::Recognized::Show(info) = info else {
                    return None;
                };
                Some((item, info))
            })
            .collect::<Vec<_>>();

        // 用 tmdb 获取正确的剧名
        let titles = items
            .iter()
            .map(|(_, info)| info.show.clone())
            .collect::<HashSet<_>>();
        debug!("query tmdb for titles: {titles:?}");
        let mapper =
            tmdb::get_info(request_client.clone(), &config.tmdb_secret, titles, pool).await?;
        debug!("tmdb map: {mapper:#?}");
        for item in items.iter_mut() {
            if let Some(tmdb) = mapper.get(&item.1.show) {
                item.1.show = tmdb.tmdb_name.clone();
                item.1.year = tmdb.year;
                item.1.tmdb_id = tmdb.tmdb_id;
            }
        }

        let mut answer = vec![];
        qb_client.login().await?;
        for (item, info) in items {
            info!("series {} new episode {info:?}", self.name);

            // insert into db
            let torrent_id = db::TorrentInfo::gen_id();
            db::TorrentInfo {
                id: torrent_id,
                name: info.show.clone(),
                year: info.year,
                season: info.season,
                episode: info.episode,
                fansub: info.fansub.clone(),
                resolution: info.resolution.clone(),
                language: info.language.clone(),
                tmdb_id: info.tmdb_id,
            }
            .insert(pool)
            .await?;

            let mut tags = self.base.tags.clone().unwrap_or_default();
            tags.push(info.show.clone());
            qb_client
                .add_torrent(crate::request::AddTorrentRequest {
                    urls: vec![item.enclosure.clone()],
                    torrents: vec![],
                    savepath: self.base.savepath.clone(),
                    content_layout: self.base.content_layout.map(|i| i.to_string()),
                    category: self.base.category.clone(),
                    tags,
                    rename: Some(format!(
                        "{anime} - S{season:02}E{ep:02} - {resolution} - {language} - {fansub} - tid{torrent_id}",
                        anime = info.show,
                        season = info.season,
                        ep = info.episode,
                        resolution = info.resolution,
                        language = info.language,
                        fansub = info.fansub
                    )),
                    auto_torrent_management: self.base.auto_torrent_management,
                    ratio_limit: self.base.ratio_limit,
                })
                .await
                .context("add torrent failed")?;
            info!("种子 {} {info:?} 成功添加到 QB", item.title);

            item.insert(pool).await?;
            answer.push(item);
        }

        Ok(answer)
    }

    async fn get_items(&self, client: &reqwest::Client) -> Result<Vec<db::Item>> {
        let url = self.url();
        let r = client
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("request {} failed", self.name))?;
        let status = r.status();
        if !status.is_success() {
            debug!("feed={}, get url {url} failed: {status:?}", self.name);
            debug!(
                "feed={}, url={url}, text: {:?}",
                self.name,
                r.text().await.ok()
            );
            bail!("feed '{}' HTTP status is '{status}'", self.name);
        }
        let s = r.bytes().await.context("failed to fetch body")?;
        let channel = rss::Channel::read_from(&s[..]).context("failed to parse as rss channel")?;
        info!(
            "feed {} (channel name {}) fetched {} items",
            self.name,
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
