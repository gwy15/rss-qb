#[macro_use]
extern crate log;

pub mod config;
pub mod db;
pub mod qb;
pub mod runner;
pub mod series;

pub use config::Config;
pub use qb::{request, QbClient};

pub mod gpt;
pub mod tmdb;

pub mod server;
