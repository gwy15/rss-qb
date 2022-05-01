use anyhow::{bail, Context, Result};
use reqwest::Client;
use scraper::{Html, Selector};

#[macro_use]
extern crate log;

const CANDIDATE_DOMAINS: &[&str] = &[
    "thepiratebay0.org",
    "thepiratebay10.org",
    "pirateproxy.live",
    "thehiddenbay.com",
];

lazy_static::lazy_static! {
    static ref TD_SELECTOR: Selector = Selector::parse("tr td").unwrap();
    static ref LINK_SELECTOR: Selector = Selector::parse("a.detLink").unwrap();
    static ref MAGNET_SELECTOR: Selector = Selector::parse("a[title]").unwrap();
    static ref INFO_SELECTOR: Selector = Selector::parse("font.detDesc").unwrap();
}

#[derive(Debug, Clone)]
pub struct Item {
    pub title: String,
    pub url: String,
    pub magnet: String,
    pub size: usize,
}

pub async fn search(client: &Client, query: &str) -> Result<Vec<Item>> {
    let mut last_error = None;
    for candidate_domain in CANDIDATE_DOMAINS.iter() {
        match search_with_domain(client, query, candidate_domain).await {
            Ok(items) => return Ok(items),
            Err(err) => {
                warn!("domain {} failed: {:?}", candidate_domain, err);
                last_error = Some(err);
                continue;
            }
        }
    }

    let e = last_error
        .unwrap()
        .context("Tried all candidate domains but none succeeded");
    Err(e)
}

pub async fn search_with_domain(client: &Client, query: &str, domain: &str) -> Result<Vec<Item>> {
    info!("searching query {:?} on domain {}", query, domain);
    let query = query.replace(' ', "%20");
    let url = format!("https://{domain}/search/{query}/1/99/200");
    let response = client.get(&url).send().await?;
    if !response.status().is_success() {
        bail!("search returned status code {}", response.status());
    }
    let text = response.text().await?;

    let items = parse_html(&text)?;

    Ok(items)
}

fn parse_html(s: &str) -> Result<Vec<Item>> {
    let mut items = vec![];
    let document = Html::parse_document(s);
    let rows = document.select(&TD_SELECTOR);
    for row in rows {
        // title & link
        let link_elem = match row.select(&LINK_SELECTOR).next() {
            Some(link) => link,
            None => continue,
        };
        let link = link_elem.value().attr("href").context("Missing link")?;
        let title = link_elem.inner_html();
        debug!("title {} link {}", title, link);

        // magnet
        let magnet_elem = row
            .select(&MAGNET_SELECTOR)
            .next()
            .context("Missing magnet")?;
        let magnet = magnet_elem.value().attr("href").context("Missing magnet")?;

        // size
        let desc_elem = row.select(&INFO_SELECTOR).next().context("Missing info")?;
        let mut desc = desc_elem.text().next().context("Missing info text")?;
        debug!("desc = {:?}", desc);
        desc = desc.split_once(", Size ").context("text format invalid")?.1;
        let size_info = desc
            .trim_end()
            .strip_suffix(", ULed by")
            .context("Failed to strip suffix")?;
        debug!("size_info = {:?}", size_info);
        let (size, unit) = size_info
            .split_once('\u{a0}')
            .context("size format invalid")?;
        let size_float = size.parse::<f64>().context("size parse error")?;
        let size = match unit {
            "GiB" => size_float * 1024.0 * 1024.0 * 1024.0,
            "MiB" => size_float * 1024.0 * 1024.0,
            "KiB" => size_float * 1024.0,
            _ => size_float,
        } as usize;

        items.push(Item {
            title,
            url: link.to_string(),
            magnet: magnet.to_string(),
            size,
        })
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn search_young_sheldon() {
        let client = reqwest::Client::new();
        let result = search(&client, "Young Sheldon 1080p CAKES WEB H264")
            .await
            .unwrap();
        assert!(result.len() > 0);
    }

    #[test]
    fn test_parse() {
        let s = include_str!("../tests/search-result.html");
        let items = parse_html(s).unwrap();
        assert_eq!(items.len(), 28);
    }
}
