use serde::Deserialize;

pub struct AddTorrentRequest {
    pub urls: Vec<String>,
    pub torrents: Vec<Vec<u8>>,
    pub savepath: Option<String>,
    pub content_layout: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub rename: Option<String>,
    pub auto_torrent_management: Option<bool>,
    pub ratio_limit: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Torrent {
    pub content_path: String,
    pub name: String,
}
