use std::path::PathBuf;

use actix_web::{post, App, HttpResponse, HttpServer, Responder};
use anyhow::{bail, Context, Result};

use crate::{db, Config};

async fn on_hook(body: bytes::Bytes) -> Result<()> {
    info!("running hook");
    let task_title = std::str::from_utf8(&body).context("Invalid body")?;
    debug!("hook: task_title = {task_title}");
    let tid = task_title
        .rsplit(" - tid")
        .next()
        .context("No tid")?
        .parse::<i64>()
        .context("Invalid tid")?;

    // load config
    let path = "config.toml".parse::<std::path::PathBuf>()?;
    let config = tokio::fs::read_to_string(path).await?;
    let mut config = toml::from_str::<Config>(&config).context("Invalid toml config")?;
    config.update_default();
    debug!("config loaded");

    // load from tid
    let db_url = format!("sqlite://{}", config.db_uri.display());
    let pool = db::Pool::connect(&db_url).await?;
    let torrent_info = db::TorrentInfo::from_id(tid, &pool).await?;
    debug!("torrent info loaded from db: {torrent_info:?}");

    // load from qbittorrent
    let qb_client = crate::QbClient::new(
        config.qb.base_url.clone(),
        &config.qb.username,
        &config.qb.password,
    )
    .await?;
    qb_client.login().await?;
    let torrents = qb_client.list_torrent(&torrent_info.name).await?;
    let torrent = torrents
        .into_iter()
        .find(|t| t.name == task_title)
        .context("torrent not found")?;
    debug!("torrent loaded from qb: {torrent:?}");
    drop(qb_client);

    // link against
    let src = torrent.content_path.parse::<PathBuf>()?;
    debug!("link src = {}", src.display());
    if !src.exists() || !src.is_file() {
        warn!("src invalid: {}. exists = {}", src.display(), src.exists());
        bail!("src file not exists or invalid file")
    }
    let ext = src
        .extension()
        .context("no extension")?
        .to_str()
        .context("invalid ext")?;
    let mut target = config.link_to.clone();
    // https://emby.media/support/articles/TV-Naming.html
    let show = if torrent_info.tmdb_id != 0 {
        format!(
            "{} ({}) [tmdbid={}]",
            torrent_info.name, torrent_info.year, torrent_info.tmdb_id
        )
    } else {
        format!("{} ({})", torrent_info.name, torrent_info.year)
    };
    target.push(show);
    target.push(format!("Season {}", torrent_info.season));
    target.push(format!(
        "{} - S{:02}E{:02} - {}-{}.{}",
        torrent_info.name,
        torrent_info.season,
        torrent_info.episode,
        torrent_info.fansub,
        torrent_info.language,
        ext
    ));
    info!("link {} => {}", src.display(), target.display());
    std::fs::create_dir_all(target.parent().unwrap())?;
    std::fs::hard_link(src, target).context("link failed")?;

    Ok(())
}

#[post("/qb_hook")]
async fn hello(body: bytes::Bytes) -> impl Responder {
    match on_hook(body).await {
        Ok(_) => {
            info!("hook run success!");
            HttpResponse::Ok().body("ok")
        }
        Err(e) => {
            error!("hook failed! err = {e:#?}");
            HttpResponse::InternalServerError().body(format!("{e:#?}"))
        }
    }
}

pub async fn main() -> Result<()> {
    HttpServer::new(|| App::new().service(hello))
        .bind(("0.0.0.0", 80))?
        .run()
        .await?;
    Ok(())
}
