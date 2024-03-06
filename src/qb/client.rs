use super::request;
use anyhow::{bail, Result};
use reqwest::{multipart, Client};
use std::{future::Future, sync::Arc};

/// 不实现 Clone，因为需要在 Drop 的时候登出
pub struct QbClient {
    inner: Client,
    base_url: Arc<str>,
    username: String,
    password: String,
}

impl QbClient {
    fn url(&self, api_name: &str, method_name: &str) -> String {
        format!("{}/api/v2/{}/{}", self.base_url, api_name, method_name)
    }

    pub async fn new(
        base_url: impl Into<Arc<str>>,
        username: &str,
        password: &str,
    ) -> Result<Self> {
        let mut client_builder = reqwest::ClientBuilder::new()
            .timeout(std::time::Duration::from_secs(10))
            .cookie_store(true);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Cache-Control",
            reqwest::header::HeaderValue::from_static("no-cache"),
        );
        client_builder = client_builder.default_headers(headers);
        let client = client_builder.build()?;
        let this = Self {
            inner: client,
            base_url: base_url.into(),
            username: username.to_string(),
            password: password.to_string(),
        };
        this.login().await?;
        info!("client logged in.");
        Ok(this)
    }

    pub async fn login(&self) -> Result<()> {
        let url = self.url("auth", "login");
        let resp = self
            .inner
            .post(&url)
            .form(&[("username", &self.username), ("password", &self.password)])
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            bail!("Login failed, status: {}", resp.status())
        }
    }

    pub fn logout(&self) -> impl Future<Output = Result<()>> + 'static {
        let url = self.url("auth", "logout");
        let fut = self.inner.post(url).send();
        async move {
            let resp = fut.await?;
            if resp.status().is_success() {
                info!("client log out success");
                Ok(())
            } else {
                bail!("Logout failed, status: {}", resp.status())
            }
        }
    }

    pub async fn add_torrent(&self, req: request::AddTorrentRequest) -> Result<()> {
        use multipart::Part;
        let mut form = multipart::Form::new();
        if !req.urls.is_empty() {
            form = form.part("urls", Part::text(req.urls.join("\n")));
        }
        for torrent in req.torrents {
            form = form.part("torrents", Part::bytes(torrent));
        }
        if let Some(savepath) = req.savepath {
            form = form.part("savepath", Part::text(savepath));
        }
        if let Some(content_layout) = req.content_layout {
            form = form.part("contentLayout", Part::text(content_layout));
        }
        if let Some(category) = req.category {
            form = form.part("category", Part::text(category));
        }
        if !req.tags.is_empty() {
            form = form.part("tags", Part::text(req.tags.join(",")));
        }
        if let Some(rename) = req.rename {
            form = form.part("rename", Part::text(rename));
        }
        if let Some(auto_torrent_management) = req.auto_torrent_management {
            form = form.part("autoTMM", Part::text(auto_torrent_management.to_string()));
        }

        let url = self.url("torrents", "add");
        let response = self.inner.post(&url).multipart(form).send().await?;
        if response.status().is_success() {
            info!("add torrent success");
            if let Ok(body) = response.text().await {
                debug!("body = {body}");
            }
            Ok(())
        } else {
            bail!("Add torrent failed, status: {}", response.status())
        }
    }

    pub async fn list_torrent(&self, tag: &str) -> Result<Vec<request::Torrent>> {
        let url = self.url("torrents", "info");
        let response = self.inner.post(&url).query(&[("tag", tag)]).send().await?;
        if !response.status().is_success() {
            bail!("Add torrent failed, status: {}", response.status())
        }
        let torrents = response.json::<Vec<request::Torrent>>().await?;
        Ok(torrents)
    }
}
