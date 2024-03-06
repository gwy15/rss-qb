use std::{fmt, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    /// The path to the database file. Example: `/home/user/data.sqlite`
    pub db_uri: PathBuf,

    /// https proxy
    #[serde(default, deserialize_with = "deserialize_proxy")]
    pub https_proxy: Option<reqwest::Proxy>,

    pub tmdb_secret: String,

    pub link_to: PathBuf,

    /// request timeout
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,

    pub email: Option<Email>,

    pub gpt: GptConfig,

    pub qb: QbConfig,
    #[serde(default)]
    pub feed: Vec<Feed>,
}

fn default_timeout() -> u64 {
    10
}

#[derive(Deserialize)]
pub struct QbConfig {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ContentLayout {
    Original,
    Subfolder,
    NoSubfolder,
}
impl fmt::Display for ContentLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentLayout::Original => write!(f, "Original"),
            ContentLayout::Subfolder => write!(f, "Subfolder"),
            ContentLayout::NoSubfolder => write!(f, "NoSubfolder"),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Feed {
    Rss(RssFeed),
}
impl Feed {
    pub fn base(&self) -> &FeedBase {
        match self {
            Self::Rss(rss) => &rss.base,
        }
    }
    pub fn name(&self) -> &str {
        &self.base().name
    }
}

#[derive(Deserialize, Clone)]
pub struct RssFeed {
    pub site: RssSite,
    pub search: String,
    #[serde(flatten)]
    pub base: FeedBase,
}
impl RssFeed {
    pub fn name(&self) -> &str {
        &self.base.name
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RssSite {
    #[serde(alias = "动漫猫")]
    Comicat,
    #[serde(alias = "动漫花园")]
    Dmhy,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeedBase {
    pub name: String,

    #[serde(default = "default_interval")]
    pub interval_s: u64,
    /// Download folder
    pub savepath: Option<String>,
    /// content layout
    #[serde(default)]
    pub content_layout: Option<ContentLayout>,

    /// Category for the torrent
    pub category: Option<String>,

    /// Tags for the torrent
    #[serde(default)]
    pub tags: Vec<String>,

    /// Whether Automatic Torrent Management should be used
    #[serde(default = "bool::default")]
    pub auto_torrent_management: bool,

    /// filter，要包含的正则
    #[serde(default, with = "serde_regex")]
    pub filters: Vec<regex::Regex>,

    /// not_filters，排除的正则
    #[serde(default, with = "serde_regex")]
    pub not_filters: Vec<regex::Regex>,
}
impl FeedBase {
    pub fn filter(&self, item: &crate::db::Item) -> bool {
        for filter in self.filters.iter() {
            if !filter.is_match(&item.title) {
                debug!("item {} filtered out.", item.title);
                return false;
            }
        }
        for filter in self.not_filters.iter() {
            if filter.is_match(&item.title) {
                debug!("item {} filtered out by not filters.", item.title);
                return false;
            }
        }
        true
    }
}

fn default_interval() -> u64 {
    15 * 60
}
fn deserialize_proxy<'de, D>(d: D) -> Result<Option<reqwest::Proxy>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = serde::Deserialize::deserialize(d)?;
    match s {
        Some(s) => {
            let proxy =
                reqwest::Proxy::https(s).map_err(|e| serde::de::Error::custom(format!("{}", e)))?;
            Ok(Some(proxy))
        }
        None => Ok(None),
    }
}

#[derive(Deserialize)]
pub struct Email {
    pub sender: String,
    pub sender_pswd: String,
    pub smtp_host: String,
    pub receiver: String,
}

#[derive(Debug, Deserialize)]
pub struct GptConfig {
    pub url: String,
    pub model: String,
    pub token: String,
    pub retry: u8,
    pub better_model: String,
    pub better_since: u8,
}
impl GptConfig {
    pub fn model(&self, time: u8) -> &str {
        if time >= self.better_since {
            &self.better_model
        } else {
            &self.model
        }
    }
}

#[cfg(test)]
mod tests {
    use reqwest::Proxy;

    use super::*;
    #[test]
    fn test_deserialize_proxy() {
        #[derive(Debug, Deserialize)]
        struct H {
            #[serde(default, deserialize_with = "deserialize_proxy")]
            h: Option<Proxy>,
        }
        let h: H = toml::from_str(r#"h = "socks5h://127.0.0.1:1080" "#).unwrap();
        assert!(h.h.is_some());
        let h: H = toml::from_str(r#""#).unwrap();
        assert!(h.h.is_none());
    }

    // #[test]
    // fn parse_templates_config() {
    //     let s = std::fs::read_to_string("./templates/config.toml").unwrap();
    //     let config: Config = toml::from_str(&s).unwrap();
    //     assert_eq!(config.feed.len(), 2);
    // }
}
