use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    /// The path to the database file. Example: `/home/user/data.sqlite`
    pub db_uri: PathBuf,

    pub email: Option<Email>,

    pub qb: QbConfig,
    #[serde(default)]
    pub feed: Vec<Feed>,
}

#[derive(Deserialize)]
pub struct QbConfig {
    pub base_url: String,
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct Feed {
    pub name: String,
    pub url: String,
    #[serde(default = "default_interval")]
    pub interval_s: u64,
    /// Download folder
    pub savepath: Option<String>,
    /// Category for the torrent
    pub category: Option<String>,
    /// Tags for the torrent
    #[serde(default)]
    pub tags: Vec<String>,
    /// Create the root folder. Possible values are true, false, unset (default)
    #[serde(default = "bool::default")]
    pub root_folder: bool,
    /// Whether Automatic Torrent Management should be used
    #[serde(default = "bool::default")]
    pub auto_torrent_management: bool,
    /// filter
    #[serde(default, with = "serde_regex")]
    pub filters: Vec<regex::Regex>,
}

fn default_interval() -> u64 {
    15 * 60
}

#[derive(Deserialize)]
pub struct Email {
    pub sender: String,
    pub sender_pswd: String,
    pub smtp_host: String,
    pub receiver: String,
}
