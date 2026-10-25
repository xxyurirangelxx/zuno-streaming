//! Servidor Zuno: fundação HTTP, configuração e banco SQLite.

pub mod api;
pub mod config;
pub mod db;
pub mod error;

pub use api::{AppState, build_router};
pub use config::Config;
pub use db::Database;
