//! Composição da API HTTP.

pub mod health;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use sqlx::SqlitePool;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::error::ApiError;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Estado compartilhado entre handlers HTTP.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Pool SQLite usado pelos handlers.
    pub pool: Arc<SqlitePool>,
    /// Instante em que o processo iniciou.
    pub started_at: Instant,
    /// Versão do servidor exposta no healthcheck.
    pub version: &'static str,
}

impl AppState {
    /// Cria o estado da aplicação a partir do pool já migrado.
    pub fn new(pool: SqlitePool, version: &'static str) -> Self {
        Self {
            pool: Arc::new(pool),
            started_at: Instant::now(),
            version,
        }
    }

    /// Acesso ao pool para testes e composição.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// Monta o roteador completo com tracing, timeout e fallback JSON.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .fallback(not_found)
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
