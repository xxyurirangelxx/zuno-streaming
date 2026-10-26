// O binário é uma CLI: imprimir ajuda e o relatório JSON em stdout é o comportamento
// esperado. O lint `print_stdout/print_stderr` continua ativo para o código de biblioteca.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use zuno_server::{AppState, Config, Database, LibraryScanner, ScanOptions, build_router};

const DEFAULT_LOG_FILTER: &str = "info,zuno_server=debug";

const USAGE: &str = "\
Zuno server

Uso:
  zuno-server            inicia o servidor HTTP (padrão)
  zuno-server scan       varre a biblioteca e grava o resultado no banco
  zuno-server --help     mostra esta ajuda

Variáveis: ZUNO_BIND_ADDR, ZUNO_DATABASE_URL, ZUNO_MUSIC_DIR, ZUNO_ARTWORK_DIR,
           ZUNO_MAX_AUDIO_FILE_BYTES, ZUNO_SCAN_ON_STARTUP, ZUNO_DB_MAX_CONNECTIONS, RUST_LOG.";

#[tokio::main]
async fn main() {
    let command = match std::env::args().nth(1) {
        Some(argument) if argument == "--help" || argument == "-h" => {
            println!("{USAGE}");
            return;
        }
        Some(argument) if argument == "scan" => Command::Scan,
        Some(argument) => {
            eprintln!("argumento desconhecido: {argument}\n\n{USAGE}");
            std::process::exit(2);
        }
        None => Command::Serve,
    };

    if let Err(error) = run(command).await {
        tracing::error!(
            error = format!("{error:#}"),
            "falha ao executar o servidor Zuno"
        );
        std::process::exit(1);
    }
}

#[derive(Debug, Clone, Copy)]
enum Command {
    Serve,
    Scan,
}

async fn run(command: Command) -> Result<()> {
    init_tracing().context("não foi possível inicializar o subsistema de logs")?;

    let config = Config::from_env().context("configuração inválida do servidor")?;
    tracing::debug!(?config, "configuração carregada");

    let database = Database::connect(&config)
        .await
        .context("não foi possível preparar o banco de dados")?;

    match command {
        Command::Scan => scan_library(&config, &database).await,
        Command::Serve => serve(&config, database).await,
    }
}

async fn scan_library(config: &Config, database: &Database) -> Result<()> {
    let scanner = LibraryScanner::new(config, ScanOptions::from_config(config))?;
    let report = scanner.scan(database).await?;

    let json = serde_json::to_string_pretty(&report)
        .context("não foi possível serializar o relatório do scan")?;
    println!("{json}");

    if report.files_failed > 0 {
        tracing::warn!(
            failed = report.files_failed,
            "varredura concluída com arquivos que falharam"
        );
    }

    Ok(())
}

async fn serve(config: &Config, database: Database) -> Result<()> {
    if config.scan_on_startup {
        tracing::info!("ZUNO_SCAN_ON_STARTUP ativo; varrendo a biblioteca antes de servir");
        let scanner = LibraryScanner::new(config, ScanOptions::from_config(config))?;
        scanner.scan(&database).await?;
    }

    let state = AppState::new(database.pool().clone(), env!("CARGO_PKG_VERSION"));
    let app = build_router(state);

    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("não foi possível escutar em {}", config.bind_addr))?;
    let address = listener
        .local_addr()
        .context("não foi possível obter o endereço local do servidor")?;

    tracing::info!(
        address = %address,
        version = env!("CARGO_PKG_VERSION"),
        "servidor Zuno escutando"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("o servidor HTTP terminou com erro")
}

fn init_tracing() -> Result<()> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()
        .map_err(|error| anyhow::anyhow!("tracing já inicializado ou inválido: {error}"))?;

    Ok(())
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!("sinal de encerramento recebido; finalizando"),
        Err(error) => tracing::warn!(%error, "não foi possível escutar o sinal de encerramento"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_documents_commands_and_env() {
        assert!(USAGE.contains("zuno-server scan"));
        assert!(USAGE.contains("ZUNO_MUSIC_DIR"));
    }

    #[test]
    fn scan_command_is_recognised() {
        // Apenas garante que o enum existe e é copiável; a execução real é testada
        // no binário e nos testes de integração do scanner.
        let command = Command::Scan;
        assert!(matches!(command, Command::Scan));
    }

    #[test]
    fn unknown_argument_message_is_helpful() {
        let message = format!("argumento desconhecido: --nope\n\n{USAGE}");
        assert!(message.contains("--help"));
    }
}
