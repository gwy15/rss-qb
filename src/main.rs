use std::str::FromStr;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var(
            "RUST_LOG",
            "debug,rustls=warn,reqwest=warn,sqlx=warn,cookie_store=info",
        );
    }
    pretty_env_logger::init_timed();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());
    let path = std::path::PathBuf::from_str(&config_path)?;

    rss_qb::runner::run_watching(path).await?;

    Ok(())
}
