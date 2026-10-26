//! Conexão com o SQLite, pool e aplicação de migrations.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

use crate::config::Config;

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Banco de dados da aplicação: pool SQLite já migrado.
#[derive(Clone, Debug)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Abre o pool, garante o diretório do arquivo e aplica as migrations.
    pub async fn connect(config: &Config) -> Result<Self> {
        ensure_parent_directory(&config.database_url)?;

        let options = SqliteConnectOptions::from_str(&config.database_url)
            .with_context(|| {
                format!(
                    "ZUNO_DATABASE_URL inválido: '{}'. Use o formato sqlite://caminho/arquivo.db.",
                    config.database_url
                )
            })?
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT)
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(config.db_max_connections)
            .acquire_timeout(ACQUIRE_TIMEOUT)
            .connect_with(options)
            .await
            .context("não foi possível conectar ao banco SQLite")?;

        let database = Self { pool };
        database.migrate().await?;

        Ok(database)
    }

    /// Aplica as migrations embutidas no binário.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("falha ao aplicar migrations do SQLite")?;

        let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(&self.pool)
            .await
            .context("falha ao consultar o histórico de migrations")?;

        tracing::info!(applied, "migrations do SQLite aplicadas");
        Ok(())
    }

    /// Verifica se o banco responde a uma consulta trivial.
    pub async fn health_check(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Acesso ao pool para composição da aplicação e testes.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Reutiliza um pool já migrado (usado pela API, que guarda apenas o pool).
    ///
    /// Não aplica migrations: quem constrói o pool é responsável por isso.
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn ensure_parent_directory(database_url: &str) -> Result<()> {
    let Some(path) = sqlite_file_path(database_url) else {
        return Ok(());
    };

    let Some(parent) = path.parent() else {
        return Ok(());
    };

    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "não foi possível criar o diretório do banco SQLite: {}",
            parent.display()
        )
    })
}

fn sqlite_file_path(database_url: &str) -> Option<PathBuf> {
    if database_url.contains("mode=memory") {
        return None;
    }

    let raw = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))
        .unwrap_or(database_url);

    let raw = raw.split('?').next().unwrap_or(raw).trim();

    if raw.is_empty() || raw == ":memory:" {
        return None;
    }

    let raw = raw.strip_prefix("//").unwrap_or(raw);

    // URLs absolutas de Windows podem chegar como `sqlite:///C:/...`.
    let raw = match raw.strip_prefix('/') {
        Some(rest) if is_windows_drive_path(rest) => rest,
        _ => raw,
    };

    Some(Path::new(raw).to_path_buf())
}

fn is_windows_drive_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sqlite_file_paths() {
        assert_eq!(
            sqlite_file_path("sqlite://data/zuno.db"),
            Some(PathBuf::from("data/zuno.db"))
        );
        assert_eq!(
            sqlite_file_path("sqlite://data/zuno.db?mode=rwc"),
            Some(PathBuf::from("data/zuno.db"))
        );
        assert_eq!(
            sqlite_file_path("sqlite:///C:/zuno/zuno.db"),
            Some(PathBuf::from("C:/zuno/zuno.db"))
        );
    }

    #[test]
    fn ignores_in_memory_databases() {
        assert_eq!(sqlite_file_path("sqlite::memory:"), None);
        assert_eq!(sqlite_file_path("sqlite://:memory:"), None);
        assert_eq!(sqlite_file_path("sqlite://data/zuno.db?mode=memory"), None);
    }
}
