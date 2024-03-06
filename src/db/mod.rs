use anyhow::Result;

pub type Pool = sqlx::SqlitePool;

mod item;
pub use item::Item;

// mod ep;
// pub use ep::{SeriesEpisode, Transaction as EpTransaction};

mod tmdb;
pub use tmdb::TmdbShow;
