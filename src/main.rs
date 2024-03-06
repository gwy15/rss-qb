use std::str::FromStr;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            "debug,rustls=warn,h2=warn,hyper=warn,reqwest=warn,sqlx=warn,cookie_store=info,html5ever=info,selectors=info",
        );
    }
    pretty_env_logger::init_timed();

    let path = std::path::PathBuf::from_str("config.toml")?;

    let server = rss_qb::server::main();
    let runner = rss_qb::runner::run_watching(path);
    tokio::select! {
        r = server => r,
        r = runner => r
    }
}
