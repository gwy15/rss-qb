use crate::db;
use regex::Regex;

impl db::Item {
    pub fn extract_series(&self, extractor: &Regex) -> Option<(&str, &str)> {
        let m = extractor.captures(&self.title)?;

        let ep = m.name("ep").or_else(|| m.name("episode"))?.as_str();
        let season = m.name("season").map(|m| m.as_str()).unwrap_or("");
        Some((season, ep))
    }
}
