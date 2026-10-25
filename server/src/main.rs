use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use zuno_server::{AppState, Config, Database, build_router};

const DEFAULT_LOG_FILTER: &str = "info,zuno_server=debug";

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        tracing::error!(
            error = format!("{error:#}"),
            "falha ao iniciar o servidor Zuno"
        );
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    init_tracing().context("não foi possível inicializar o subsistema de logs")?;

    let config = Config::from_env().context("configuração inválida do servidor")?;
    tracing::debug!(?config, "configuração carregada");

    let database = Database::connect(&config)
        .await
        .context("não foi possível preparar o banco de dados")?;

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
