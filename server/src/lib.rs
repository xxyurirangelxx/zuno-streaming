//! Servidor Zuno: fundação HTTP, configuração, banco SQLite e biblioteca musical.

pub mod api;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod library;
pub mod repository;

pub use api::{AppState, build_router};
pub use config::Config;
pub use db::Database;
pub use library::{LibraryScanner, ScanOptions};
