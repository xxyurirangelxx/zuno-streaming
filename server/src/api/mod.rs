//! Composição da API HTTP.

pub mod artwork;
pub mod auth;
pub mod health;
pub mod invitations;
pub mod library;
pub mod pagination;
pub mod setup;
pub mod stream;
pub mod uploads;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use sqlx::SqlitePool;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::auth::LoginRateLimiter;
use crate::config::Config;
use crate::error::ApiError;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Estado compartilhado entre handlers HTTP.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Pool SQLite usado pelos handlers.
    pub database: Arc<SqlitePool>,
    /// Configuração carregada na inicialização (raízes de biblioteca e cache).
    pub config: Arc<Config>,
    /// Instante em que o processo iniciou.
    pub started_at: Instant,
    /// Versão do servidor exposta no healthcheck.
    pub version: &'static str,
    /// Limitador de tentativas de login (em memória).
    pub login_limiter: Arc<LoginRateLimiter>,
}

impl AppState {
    /// Cria o estado da aplicação a partir do pool já migrado e da configuração.
    pub fn new(pool: SqlitePool, config: Config, version: &'static str) -> Self {
        Self {
            database: Arc::new(pool),
            config: Arc::new(config),
            started_at: Instant::now(),
            version,
            login_limiter: Arc::new(LoginRateLimiter::default()),
        }
    }

    /// Acesso ao pool para testes e composição.
    pub fn pool(&self) -> &SqlitePool {
        &self.database
    }
}

/// Monta o roteador completo com tracing, timeout e fallback JSON.
pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/v1/tracks", get(library::list_tracks))
        .route("/api/v1/tracks/{id}", get(library::get_track))
        .route(
            "/api/v1/tracks/{id}/stream",
            get(stream::stream_track).head(stream::stream_track),
        )
        .route("/api/v1/albums", get(library::list_albums))
        .route("/api/v1/artists", get(library::list_artists))
        .route("/api/v1/search", get(library::search))
        .route("/api/v1/artworks/{id}", get(artwork::get_artwork))
        .route("/api/v1/uploads", post(uploads::create_upload))
        .route(
            "/api/v1/uploads/{id}",
            get(uploads::get_upload).delete(uploads::delete_upload),
        )
        .route(
            "/api/v1/uploads/{id}/complete",
            post(uploads::complete_upload),
        )
        .route("/api/v1/me", get(auth::me))
        .route("/api/v1/auth/refresh", post(auth::refresh))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route(
            "/api/v1/invitations",
            get(invitations::list).post(invitations::create),
        )
        .route("/api/v1/invitations/{id}", delete(invitations::revoke))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::middleware::require_auth,
        ));

    Router::new()
        .route("/health", get(health::health))
        .route("/api/v1/setup/status", get(setup::status))
        .route("/api/v1/setup/claim", post(setup::claim))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/invitations/redeem", post(invitations::redeem))
        .merge(protected)
        .fallback(not_found)
        .layer(middleware::from_fn(
            crate::auth::middleware::same_origin_guard,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(middleware::from_fn(json_timeout_response))
        .with_state(state)
}

async fn not_found() -> ApiError {
    ApiError::NotFound
}

/// Converte o 408 vazio do `TimeoutLayer` no corpo JSON padrão da API.
async fn json_timeout_response(request: axum::extract::Request, next: Next) -> Response {
    let response = next.run(request).await;

    if response.status() == StatusCode::REQUEST_TIMEOUT {
        ApiError::Timeout.into_response()
    } else {
        response
    }
}

/// Resolve um caminho relativo dentro de uma raiz, recusando escapes.
///
/// Usa `canonicalize` (portanto exige que o arquivo exista) e confere se o
/// resultado continua dentro da raiz canônica. Qualquer falha vira 404: nunca
/// revelamos caminhos do servidor ao cliente.
pub fn resolve_within(root: &Path, relative: &str) -> Result<PathBuf, ApiError> {
    let candidate = root.join(relative);

    let canonical = std::fs::canonicalize(&candidate).map_err(|_| ApiError::NotFound)?;
    let canonical_root = std::fs::canonicalize(root).map_err(|error| {
        ApiError::Internal(anyhow::anyhow!(
            "raiz inacessível {}: {error}",
            root.display()
        ))
    })?;

    if !canonical.starts_with(&canonical_root) {
        tracing::warn!(
            relative,
            "caminho resolvido fora da raiz configurada; requisição recusada"
        );
        return Err(ApiError::NotFound);
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_within_rejects_escapes_and_missing_files() {
        let dir = tempfile::tempdir().expect("diretório temporário");
        let file = dir.path().join("track.opus");
        std::fs::write(&file, b"conteudo").expect("escrever arquivo");

        assert!(resolve_within(dir.path(), "track.opus").is_ok());
        assert!(resolve_within(dir.path(), "../escape.opus").is_err());
        assert!(resolve_within(dir.path(), "nao-existe.opus").is_err());
    }
}
