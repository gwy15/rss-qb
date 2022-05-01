use crate::{
    config::{Email, Feed},
    db, request, Config, QbClient,
};
use anyhow::{bail, Context, Result};
use notify::Watcher;
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};
use tokio::sync::broadcast as a_broadcast;

pub async fn run_watching(path: PathBuf) -> Result<()> {
    let (tx, notify_events_rx) = mpsc::channel();
    let mut watcher = notify::watcher(tx, std::time::Duration::from_secs(10))?;
    watcher.watch(&path, notify::RecursiveMode::NonRecursive)?;

    let (stop_tx, _) = a_broadcast::channel(3);

    // now for the notify event, spawn a dedicated thread to receive the events so that
    // it does not block the event loop
    let stop_tx_clone = stop_tx.clone();
    std::thread::spawn(move || loop {
        match notify_events_rx.recv() {
            Ok(event) => {
                info!("config file changed, reloading");
                debug!("event = {:?}", event);
                stop_tx_clone.send(()).unwrap();
            }
            Err(e) => {
                log::error!("watch config file error: {:?}", e);
                break;
            }
        }
    });

    loop {
        let config = load_config(&path).await?;
        run_config(config, stop_tx.clone()).await?;
        info!("config file changed, reloading");
    }
}

/// 循环跑一个 config
async fn run_config(config: Config, stop: a_broadcast::Sender<()>) -> Result<()> {
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

    let email = Arc::new(config.email);

    let mut fut = vec![];
    for feed in config.feed {
        fut.push(run_feed(
            qb_client.clone(),
            feed,
            email.clone(),
            pool.clone(),
            stop.subscribe(),
        ));
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
async fn run_feed(
    qb_client: Arc<QbClient>,
    feed: Feed,
    email: Arc<Option<Email>>,
    pool: db::Pool,
    mut stop: a_broadcast::Receiver<()>,
) -> Result<()> {
    let secs = feed.interval_s;
    let mut timer = tokio::time::interval(std::time::Duration::from_secs(secs));
    let mut error_counter = 0;
    loop {
        tokio::select! {
            r = stop.recv() => {
                match r {
                    Ok(_) => {
                        info!("received stop sign.");
                        return Ok(());
                    }
                    Err(e) => {
                        log::error!("receive stop sign error: {:?}", e);
                        return Err(anyhow::anyhow!(e));
                    }
                }
            }
            _ = timer.tick() => {
                match run_once(&qb_client, &feed, &email, &pool).await {
                    Ok(_) => {
                        info!("Successfully downloaded {}", feed.name);
                        error_counter = 0;
                    }
                    Err(e) => {
                        eprintln!("{:?}", e);
                        error_counter += 1;
                        if error_counter == 3 {
                            bail!("Too many errors");
                        }
                    }
                }
            }
        }
    }
}

/// 跑一个 feed 并发送结果
async fn run_once(
    qb_client: &QbClient,
    feed: &Feed,
    email: &Option<Email>,
    pool: &db::Pool,
) -> Result<()> {
    match email {
        None => {
            run_once_inner(qb_client, feed, pool).await?;
            debug!("no email configured.");
            Ok(())
        }
        Some(email) => {
            let ret = run_once_inner(qb_client, feed, pool).await;
            let (title, body) = match &ret {
                Ok(added) if !added.is_empty() => {
                    let title = format!("RSS 订阅 {} 新增 {} 个", feed.name, added.len());
                    let body = added
                        .iter()
                        .map(|item| format!("- {}", item.title))
                        .collect::<Vec<_>>()
                        .join("\n");
                    (title, body)
                }
                Ok(_empty) => return Ok(()),
                Err(e) => {
                    let title = format!("刷新 RSS feed {} 发生错误", feed.name);
                    let body = format!("{e:?}");
                    (title, body)
                }
            };

            send(&title, &body, email).await?;

            match ret {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
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
    feed: &Feed,
    pool: &db::Pool,
) -> Result<Vec<db::Item>> {
    info!("Fetching feed {}", feed.name);
    let r = qb_client.inner.get(&feed.url).send().await?;
    if !r.status().is_success() {
        bail!("feed {} HTTP status {}", feed.name, r.status());
    }
    let s = r.bytes().await.context("failed to fetch body")?;
    let channel = rss::Channel::read_from(&s[..]).context("failed to parse as rss channel")?;
    info!("feed {} channel named {} fetched", feed.name, channel.title);
    let mut added = vec![];
    for item in channel.items {
        debug!("judging item {:?}", item.title);
        let item: db::Item = item.try_into()?;
        trace!("item {:?}", item);

        if db::Item::exists(&item.guid, pool).await? {
            debug!("item {} already exists", item.title);
            continue;
        }

        info!("item {} not found, adding", item.title);
        qb_client
            .add_torrent(request::AddTorrentRequest {
                urls: vec![item.enclosure.clone()],
                torrents: vec![],
                savepath: feed.savepath.clone(),
                category: feed.category.clone(),
                tags: feed.tags.clone(),
                rootfolder: Some(feed.root_folder),
                rename: None,
                auto_torrent_management: Some(feed.auto_torrent_management),
            })
            .await
            .context("add torrent failed")?;
        item.insert(pool).await?;
        added.push(item);
    }

    Ok(added)
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
