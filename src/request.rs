pub struct AddTorrentRequest {
    pub urls: Vec<String>,
    pub torrents: Vec<Vec<u8>>,
    pub savepath: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    /// Create the root folder. Possible values are true, false, unset (default)
    pub rootfolder: Option<bool>,
    pub rename: Option<String>,
    pub auto_torrent_management: Option<bool>,
}
