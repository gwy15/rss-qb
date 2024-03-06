use crate::{
    config::{Email, Feed},
    db, Config, QbClient,
};
use anyhow::{bail, Context, Result};
use notify::Watcher;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
#[cfg(unix)]
use tokio::signal::unix as signal;
use tokio::sync::mpsc;

mod rss_;

struct Reloader {
    event: mpsc::Sender<()>,
}
impl Reloader {
    pub fn new(tx: mpsc::Sender<()>) -> Self {
        Self { event: tx }
    }
}
impl notify::EventHandler for Reloader {
    fn handle_event(&mut self, _event: notify::Result<notify::Event>) {
        self.event.blocking_send(()).unwrap();
    }
}

async fn main_loop(mut reload: mpsc::Receiver<()>, path: &Path) -> Result<()> {
    loop {
        tokio::select! {
            _ = reload.recv() => {
                info!("change in config file detected, reloading");
                continue
            },
            _ = signal() => {
                info!("received signal, exiting");
                return Ok(());
            }
            r = run_config(path) => {
                return r;
            }
        }
    }
}

pub async fn run_watching(config_path: PathBuf) -> Result<()> {
    let (tx, rx) = mpsc::channel(1);
    let reloader = Reloader::new(tx);
    let mut watcher = notify::recommended_watcher(reloader)?;
    watcher.watch(&config_path, notify::RecursiveMode::NonRecursive)?;

    main_loop(rx, &config_path).await
}

#[cfg(unix)]
async fn signal() -> Result<()> {
    let mut sig_term = signal::signal(signal::SignalKind::terminate())?;

    tokio::select! {
        _ = sig_term.recv() => {
            info!("received signterm, exiting");
            Ok(())
        }
        _ = tokio::signal::ctrl_c() => {
            info!("ctrl-c received, stopping");
            Ok(())
        }
    }
}

#[cfg(not(unix))]
async fn signal() -> Result<()> {
    tokio::signal::ctrl_c().await?;
    info!("ctrl-c received, stopping");
    Ok(())
}

/// 循环跑一个 config
async fn run_config(config_path: &Path) -> Result<()> {
    info!("running on config {}", config_path.display());
    let config = load_config(config_path).await?;
    let config = Arc::new(config);
    let db_url = format!("sqlite://{}", config.db_uri.display());
    let pool = db::Pool::connect(&db_url).await?;
    sqlx::migrate!().run(&pool).await?;

    let qb_client = QbClient::new(
        config.qb.base_url.clone(),
        &config.qb.username,
        &config.qb.password,
    )
    .await?;
    let qb_client = Arc::new(qb_client);

    let mut request_client_builder =
        reqwest::ClientBuilder::new().timeout(std::time::Duration::from_secs(config.timeout_s));
    if let Some(proxy) = config.https_proxy.clone() {
        debug!("setting request client proxy to {:?}", proxy);
        request_client_builder = request_client_builder.proxy(proxy);
    }
    let request_client = request_client_builder.build()?;
    let request_client = Arc::new(request_client);

    let mut fut = vec![];
    for feed in config.feed.iter() {
        let config = config.clone();
        let feed = feed.clone();
        let qb_client = qb_client.clone();
        let request_client = request_client.clone();
        let pool = pool.clone();
        fut.push(async move { loop_feed(qb_client, request_client, feed, pool, &config).await });
    }

    futures::future::try_join_all(fut).await?;

    Ok(())
}

async fn load_config(path: &Path) -> Result<Config> {
    let config_str = tokio::fs::read_to_string(path)
        .await
        .with_context(|| format!("Cannot read config path {}", path.display()))?;
    let config = toml::from_str::<Config>(&config_str).context("Config file corrupted")?;
    Ok(config)
}

/// 循环跑一个 feed
async fn loop_feed(
    qb_client: Arc<QbClient>,
    request_client: Arc<reqwest::Client>,
    feed: Feed,
    pool: db::Pool,
    config: &Config,
) -> Result<()> {
    let secs = feed.base().interval_s;
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(secs));
    let mut error_counter = 0;
    loop {
        timer.tick().await;
        match run_feed_once(&qb_client, &request_client, &feed, &pool, config).await {
            Ok(_) => {
                info!("RSS {} 刷新完成", feed.name());
                error_counter = 0;
            }
            Err(e) => {
                error!(
                    "feed {} failed for the {} times: {:?}",
                    feed.name(),
                    error_counter,
                    e
                );
                error_counter += 1;
                if error_counter == 3 {
                    error!("Too many errors! error_counter = {}", error_counter);
                    if let Some(email) = &config.email {
                        let title = format!("RSS {} 刷新失败: {}", feed.name(), e);
                        let body = format!("错误信息：\n{:?}", e);
                        send(&title, &body, email).await.ok();
                    }
                    bail!("Too many errors");
                }
            }
        }
    }
}

/// 跑一个 feed 并发送结果
async fn run_feed_once(
    qb_client: &QbClient,
    request_client: &reqwest::Client,
    feed: &Feed,
    pool: &db::Pool,
    config: &Config,
) -> Result<()> {
    match &config.email {
        None => {
            run_once_inner(qb_client, request_client, feed, pool, config).await?;
            debug!("no email configured.");
            Ok(())
        }
        Some(email) => {
            let ret = run_once_inner(qb_client, request_client, feed, pool, config).await;
            match ret {
                Ok(added) if !added.is_empty() => {
                    let title = format!("RSS 订阅 {} 新增 {} 个", feed.name(), added.len());
                    let body = added
                        .iter()
                        .map(|item| format!("- {}", item.title))
                        .collect::<Vec<_>>()
                        .join("\n");
                    send(&title, &body, email).await?;
                    Ok(())
                }
                r => r.map(|_| ()),
            }
        }
    }
}

async fn send(title: &str, body: &str, email: &Email) -> Result<()> {
    use lettre::{
        message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
        AsyncTransport, Message, Tokio1Executor,
    };
    debug!("sending email to {}", email.receiver);
    let message = Message::builder()
        .from(Mailbox::new(None, email.sender.parse()?))
        .to(Mailbox::new(None, email.receiver.parse()?))
        .subject(title)
        .body(body.to_string())?;
    let credentials = Credentials::new(email.sender.clone(), email.sender_pswd.clone());

    let mailer = AsyncSmtpTransport::<Tokio1Executor>::relay(&email.smtp_host)?
        .credentials(credentials)
        .build();

    let r = mailer.send(message).await?;
    debug!("send email result: {:?}", r);
    Ok(())
}

/// 跑一个 feed
async fn run_once_inner(
    qb_client: &QbClient,
    request_client: &reqwest::Client,
    feed: &Feed,
    pool: &db::Pool,
    config: &Config,
) -> Result<Vec<db::Item>> {
    match feed {
        Feed::Rss(rss) => rss.run(qb_client, request_client, pool, config).await,
    }
}
